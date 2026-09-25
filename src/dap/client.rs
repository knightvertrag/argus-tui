use std::collections::VecDeque;
use std::process::ExitStatus;

use tokio::io::{AsyncBufRead, AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;

use tracing::Instrument;

use super::framing::{self, FramingError};
use super::types::{Message, ProtocolError, Request, RequestMessage, Seq};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("failed to spawn lldb-dap. Is Xcode / lldb-dap available via xcrun?")]
    Spawn(#[source] std::io::Error),
    #[error("lldb-dap stdio pipe is missing")]
    MissingPipe,
    #[error("lldb-dap closed the DAP stream")]
    Closed,
    #[error(transparent)]
    Framing(#[from] FramingError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error("unexpected response for seq {got} (waiting for {expected}, command {command})")]
    UnexpectedResponse {
        expected: Seq,
        got: Seq,
        command: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Event(super::types::Event),
    ReverseRequest(RequestMessage),
}

pub struct Client {
    child: Option<Child>,
    stdin: ChildStdin,
    /// Complete frames only. `recv` is cancellation-safe because a dropped
    /// wait leaves the frame in this channel. `framing::read` is not, so the
    /// reader task is the only caller and is never cancelled mid-frame.
    frames: mpsc::UnboundedReceiver<Result<Message, ClientError>>,
    next_seq: Seq,
    inbox: VecDeque<Incoming>,
}

impl Client {
    /// Spawn `xcrun lldb-dap`. Must run inside a tokio runtime: a task owns stdout.
    pub fn spawn() -> Result<Self, ClientError> {
        let mut child = Command::new("xcrun")
            .arg("lldb-dap")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(ClientError::Spawn)?;

        let stdin = child.stdin.take().ok_or(ClientError::MissingPipe)?;
        let stdout = child.stdout.take().ok_or(ClientError::MissingPipe)?;
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(log_stderr(stderr));
        }

        tracing::info!("spawned xcrun lldb-dap");

        Ok(Self {
            child: Some(child),
            stdin,
            frames: spawn_reader(BufReader::new(stdout)),
            next_seq: 1,
            inbox: VecDeque::new(),
        })
    }

    pub async fn request<R: Request>(
        &mut self,
        args: R::Arguments,
    ) -> Result<R::Response, ClientError> {
        let seq = self.next_seq;
        self.next_seq += 1;
        let span = tracing::debug_span!("dap.request", command = R::COMMAND, seq);
        self.send_and_wait::<R>(seq, args).instrument(span).await
    }

    async fn send_and_wait<R: Request>(
        &mut self,
        seq: Seq,
        args: R::Arguments,
    ) -> Result<R::Response, ClientError> {
        tracing::debug!(seq, command = R::COMMAND, "dap request");
        let message = Message::Request(RequestMessage::new::<R>(seq, args)?);
        let body = serde_json::to_vec(&message)?;
        tracing::trace!(json = %String::from_utf8_lossy(&body), "dap request body");
        framing::write(&mut self.stdin, &body).await?;

        loop {
            match self.next_frame().await? {
                Message::Response(resp) if resp.request_seq == seq => {
                    return Ok(resp.decode_success::<R>()?);
                }
                Message::Response(resp) => {
                    tracing::error!(
                        expected = seq,
                        got = resp.request_seq,
                        command = %resp.command,
                        "unexpected dap response"
                    );
                    return Err(ClientError::UnexpectedResponse {
                        expected: seq,
                        got: resp.request_seq,
                        command: resp.command,
                    });
                }
                Message::Event(event) => {
                    self.inbox.push_back(Incoming::Event(event.parse()?));
                }
                Message::Request(request) => {
                    self.inbox.push_back(Incoming::ReverseRequest(request));
                }
            }
        }
    }

    /// Events and reverse requests received while waiting for a response.
    pub fn drain_inbox(&mut self) -> Vec<Incoming> {
        self.inbox.drain(..).collect()
    }

    pub async fn recv(&mut self) -> Result<Incoming, ClientError> {
        if let Some(incoming) = self.inbox.pop_front() {
            return Ok(incoming);
        }

        match self.next_frame().await? {
            Message::Event(event) => Ok(Incoming::Event(event.parse()?)),
            Message::Request(request) => Ok(Incoming::ReverseRequest(request)),
            Message::Response(resp) => {
                tracing::error!(
                    got = resp.request_seq,
                    command = %resp.command,
                    "unexpected dap response with no pending request"
                );
                Err(ClientError::UnexpectedResponse {
                    expected: 0,
                    got: resp.request_seq,
                    command: resp.command,
                })
            }
        }
    }

    pub async fn shutdown(mut self) -> Result<ExitStatus, ClientError> {
        let mut child = self.child.take().ok_or(ClientError::MissingPipe)?;
        let status = child.wait().await?;
        tracing::info!(%status, "lldb-dap exited");
        Ok(status)
    }

    async fn next_frame(&mut self) -> Result<Message, ClientError> {
        match self.frames.recv().await {
            Some(result) => result,
            None => Err(ClientError::Closed),
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.start_kill();
        }
    }
}

async fn log_stderr(stderr: tokio::process::ChildStderr) {
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        tracing::debug!(target: "lldb_dap", "{line}");
    }
}

fn spawn_reader<R>(mut reader: R) -> mpsc::UnboundedReceiver<Result<Message, ClientError>>
where
    R: AsyncBufRead + Unpin + Send + 'static,
{
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            match read_frame(&mut reader).await {
                Ok(message) => {
                    if tx.send(Ok(message)).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let _ = tx.send(Err(err));
                    break;
                }
            }
        }
    });
    rx
}

async fn read_frame<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Message, ClientError> {
    let body = framing::read(reader).await?;
    tracing::trace!(json = %String::from_utf8_lossy(&body), "dap frame");
    let message: Message = serde_json::from_slice(&body)?;
    match &message {
        Message::Response(resp) => {
            tracing::debug!(
                request_seq = resp.request_seq,
                command = %resp.command,
                success = resp.success,
                "dap response"
            );
        }
        Message::Event(event) => {
            tracing::debug!(seq = event.seq, event = %event.event, "dap event");
        }
        Message::Request(request) => {
            tracing::warn!(command = %request.command, "dap reverse request ignored");
        }
    }
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::time::Duration;

    fn two_event_frames() -> Vec<u8> {
        let first = br#"{"seq":1,"type":"event","event":"initialized"}"#;
        let second = br#"{"seq":2,"type":"event","event":"terminated"}"#;
        let mut bytes = framing::encode(first);
        bytes.extend_from_slice(&framing::encode(second));
        bytes
    }

    #[tokio::test]
    async fn reader_delivers_two_frames() {
        let reader = BufReader::new(Cursor::new(two_event_frames()));
        let mut frames = spawn_reader(reader);

        let first = frames.recv().await.unwrap().unwrap();
        let second = frames.recv().await.unwrap().unwrap();
        match first {
            Message::Event(event) => assert_eq!(event.event, "initialized"),
            other => panic!("expected event, got {other:?}"),
        }
        match second {
            Message::Event(event) => assert_eq!(event.event, "terminated"),
            other => panic!("expected event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn cancelled_recv_keeps_the_frame() {
        let reader = BufReader::new(Cursor::new(two_event_frames()));
        let mut frames = spawn_reader(reader);

        let cancelled = tokio::time::timeout(Duration::from_secs(1), async {
            tokio::select! {
                biased;
                _ = std::future::ready(()) => None,
                message = frames.recv() => message,
            }
        })
        .await
        .expect("select");
        assert!(cancelled.is_none());

        let message = tokio::time::timeout(Duration::from_secs(1), frames.recv())
            .await
            .expect("frame still queued")
            .expect("channel open")
            .expect("frame decodes");
        match message {
            Message::Event(event) => assert_eq!(event.event, "initialized"),
            other => panic!("expected event, got {other:?}"),
        }
    }
}
