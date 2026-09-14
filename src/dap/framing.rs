//! DAP Content-Length framing (header + UTF-8 JSON body).

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const MAX_HEADER_BYTES: usize = 1024 * 1024;
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum FramingError {
    #[error("unexpected EOF while reading DAP frame")]
    UnexpectedEof,
    #[error("missing Content-Length header")]
    MissingContentLength,
    #[error("invalid Content-Length header")]
    InvalidLength,
    #[error("DAP header is not valid ASCII/UTF-8")]
    InvalidHeader,
    #[error("DAP headers exceed {MAX_HEADER_BYTES} bytes")]
    HeadersTooLarge,
    #[error("DAP body of {len} bytes exceeds {MAX_BODY_BYTES} byte limit")]
    BodyTooLarge { len: usize },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub fn encode(body: &[u8]) -> Vec<u8> {
    let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    frame.extend_from_slice(body);
    frame
}

pub async fn write<W: AsyncWrite + Unpin>(writer: &mut W, body: &[u8]) -> Result<(), FramingError> {
    tracing::trace!(content_length = body.len(), "dap write");
    writer.write_all(&encode(body)).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Vec<u8>, FramingError> {
    let mut content_length = None;
    let mut header_bytes = 0usize;

    loop {
        let mut line = Vec::new();
        let n = reader.read_until(b'\n', &mut line).await?;
        if n == 0 {
            return Err(FramingError::UnexpectedEof);
        }
        header_bytes += n;
        if header_bytes > MAX_HEADER_BYTES {
            tracing::warn!(header_bytes, "DAP headers too large");
            return Err(FramingError::HeadersTooLarge);
        }

        if line.ends_with(b"\n") {
            line.pop();
        }
        if line.ends_with(b"\r") {
            line.pop();
        }
        if line.is_empty() {
            break;
        }

        let line = std::str::from_utf8(&line).map_err(|_| FramingError::InvalidHeader)?;
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("Content-Length")
        {
            content_length = Some(
                value
                    .trim()
                    .parse()
                    .map_err(|_| FramingError::InvalidLength)?,
            );
        }
    }

    let len = content_length.ok_or(FramingError::MissingContentLength)?;
    if len > MAX_BODY_BYTES {
        tracing::warn!(content_length = len, "DAP body too large");
        return Err(FramingError::BodyTooLarge { len });
    }

    tracing::trace!(content_length = len, "dap read");

    let mut body = vec![0u8; len];
    match reader.read_exact(&mut body).await {
        Ok(_) => Ok(body),
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
            Err(FramingError::UnexpectedEof)
        }
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tokio::io::BufReader;

    async fn read_from(bytes: &[u8]) -> Result<Vec<u8>, FramingError> {
        let mut reader = BufReader::new(Cursor::new(bytes.to_vec()));
        read(&mut reader).await
    }

    #[tokio::test]
    async fn roundtrip_one_message() {
        let body = br#"{"seq":1,"type":"request","command":"threads"}"#;
        let encoded = encode(body);
        let mut reader = BufReader::new(Cursor::new(encoded.clone()));
        let got = read(&mut reader).await.unwrap();
        assert_eq!(got, body);

        let mut out = Vec::new();
        write(&mut out, body).await.unwrap();
        assert_eq!(out, encoded);
    }

    #[tokio::test]
    async fn ignores_extra_headers() {
        let body = b"hello";
        let frame = format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            std::str::from_utf8(body).unwrap()
        );
        assert_eq!(read_from(frame.as_bytes()).await.unwrap(), body);
    }

    #[tokio::test]
    async fn content_length_allows_spaces() {
        let body = b"abcdefghijkl";
        let frame = format!("Content-Length:  {}\r\n\r\n{}", body.len(), "abcdefghijkl");
        assert_eq!(read_from(frame.as_bytes()).await.unwrap(), body);
    }

    #[tokio::test]
    async fn two_frames_back_to_back() {
        let first = b"{\"a\":1}";
        let second = b"{\"b\":2}";
        let mut bytes = encode(first);
        bytes.extend_from_slice(&encode(second));
        let mut reader = BufReader::new(Cursor::new(bytes));
        assert_eq!(read(&mut reader).await.unwrap(), first);
        assert_eq!(read(&mut reader).await.unwrap(), second);
    }

    #[tokio::test]
    async fn missing_content_length() {
        let err = read_from(b"Content-Type: application/json\r\n\r\n{}")
            .await
            .unwrap_err();
        assert!(matches!(err, FramingError::MissingContentLength));
    }

    #[tokio::test]
    async fn eof_after_partial_header() {
        let err = read_from(b"Content-Length: 4\r\n").await.unwrap_err();
        assert!(matches!(err, FramingError::UnexpectedEof));
    }

    #[tokio::test]
    async fn eof_after_partial_body() {
        let err = read_from(b"Content-Length: 8\r\n\r\nabc")
            .await
            .unwrap_err();
        assert!(matches!(err, FramingError::UnexpectedEof));
    }

    #[tokio::test]
    async fn invalid_content_length() {
        let err = read_from(b"Content-Length: nope\r\n\r\n")
            .await
            .unwrap_err();
        assert!(matches!(err, FramingError::InvalidLength));
    }
}
