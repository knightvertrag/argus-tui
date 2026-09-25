//! Crossterm key and resize stream. The draw loop wakes on these; it does not poll at a fixed rate.

use crossterm::event::{Event as CrosstermEvent, EventStream};
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct EventHandler {
    receiver: mpsc::UnboundedReceiver<CrosstermEvent>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut reader = EventStream::new();
            loop {
                tokio::select! {
                    _ = sender.closed() => break,
                    Some(Ok(event)) = reader.next().fuse() => {
                        if sender.send(event).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Self { receiver }
    }

    pub async fn next(&mut self) -> Option<CrosstermEvent> {
        self.receiver.recv().await
    }
}
