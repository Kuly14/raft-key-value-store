use rand::Rng;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;
use tokio::time::{Duration, Sleep, sleep};

pub(crate) struct Timeout {
    sleep: Pin<Box<Sleep>>,       // Timer for the timeout duration
    reset_rx: mpsc::Receiver<()>, // Channel to receive reset signals
    reset_tx: mpsc::Sender<()>,   // Channel to send reset signals (for external use)
}

impl Timeout {
    pub(crate) fn new() -> Self {
        let (reset_tx, reset_rx) = mpsc::channel(1); // Capacity 1 for latest reset
        // TODO: CHange back to 150..=300
        let timeout_ms = rand::rng().random_range(1..=2);
        Timeout {
            // TODO: Change back to from_millis()
            sleep: Box::pin(sleep(Duration::from_secs(timeout_ms as u64))),
            reset_rx,
            reset_tx,
        }
    }

    pub(crate) fn reset(&mut self) {
        // TODO: Change back to 150..=300
        let timeout_ms = rand::rng().random_range(1..=2);
        // TODO: CHange back to from_millis
        self.sleep = Box::pin(sleep(Duration::from_secs(timeout_ms as u64)));
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
                Poll::Pending
            }
            Poll::Ready(None) => Poll::Ready(()),
            Poll::Pending => Poll::Pending,
        }
    }
}
