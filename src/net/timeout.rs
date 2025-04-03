use futures::FutureExt;
use rand::Rng;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
use tokio::time::{Duration, Sleep, sleep};

#[derive(Debug)]
pub(crate) struct Timeout {
    sleep: Pin<Box<Sleep>>, // Timer for the timeout duration
    pub(crate) is_leader: bool,
    reset_rx: mpsc::Receiver<()>, // Channel to receive reset signals
    pub(crate) reset_tx: mpsc::Sender<()>, // Channel to send reset signals (for external use)
}

impl Timeout {
    pub(crate) fn new() -> Self {
        let (reset_tx, reset_rx) = mpsc::channel(1); // Capacity 1 for latest reset
        let timeout_ms = rand::rng().random_range(150..=250);
        Timeout {
            sleep: Box::pin(sleep(Duration::from_millis(timeout_ms as u64))),
            is_leader: false,
            reset_rx,
            reset_tx,
        }
    }

    pub(crate) fn reset(&mut self) {
        // TODO: Change back to 150..=300
        let timeout_ms = if self.is_leader {
            100
        } else {
            rand::rng().random_range(150..=250)
        };
        self.sleep = Box::pin(sleep(Duration::from_millis(timeout_ms as u64)));
    }
}

impl Future for Timeout {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        // Poll the sleep timer to check if timeout has elapsed
        if this.sleep.as_mut().poll(cx).is_ready() {
            return Poll::Ready(());
        }

        // Poll the reset channel for heartbeat signals
        match this.reset_rx.poll_recv(cx) {
            Poll::Ready(Some(())) => {
                this.reset();
                // TODO: Figure out if this is necessary
                let _ = this.poll_unpin(cx);
                Poll::Pending
            }
            Poll::Ready(None) => Poll::Ready(()),
            Poll::Pending => Poll::Pending,
        }
    }
}
