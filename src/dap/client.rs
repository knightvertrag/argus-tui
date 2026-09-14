use std::collections::VecDeque;
use std::process::ExitStatus;

use tokio::io::BufReader;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use tracing::Instrument;

use super::framing::{self, FramingError};
use super::types::{Event, Message, ProtocolError, Request, RequestMessage, Seq};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("failed to spawn lldb-dap. Is Xcode / lldb-dap available via xcrun?")]
    Spawn(#[source] std::io::Error),
    #[error("lldb-dap stdio pipe is missing")]
    MissingPipe,
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
    Event(Event),
    ReverseRequest(RequestMessage),
}

pub struct Client {
    child: Option<Child>,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_seq: Seq,
    inbox: VecDeque<Incoming>,
}

impl Client {
    pub fn spawn() -> Result<Self, ClientError> {
        let mut child = Command::new("xcrun")
            .arg("lldb-dap")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(ClientError::Spawn)?;

        let stdin = child.stdin.take().ok_or(ClientError::MissingPipe)?;
        let stdout = child.stdout.take().ok_or(ClientError::MissingPipe)?;

        tracing::info!("spawned xcrun lldb-dap");

        Ok(Self {
            child: Some(child),
            stdin,
            stdout: BufReader::new(stdout),
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
            match self.read_message().await? {
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

        match self.read_message().await? {
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

    async fn read_message(&mut self) -> Result<Message, ClientError> {
        let body = framing::read(&mut self.stdout).await?;
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
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.start_kill();
        }
    }
}
