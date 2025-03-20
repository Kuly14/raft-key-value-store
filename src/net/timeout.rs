use rand::Rng;
use std::time::Duration;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{sync::mpsc, time::Instant};

pub(crate) struct Timeout {
    deadline: Instant,
    reset_rx: mpsc::Receiver<()>,
}

impl Timeout {
    pub(crate) fn new(reset_rx: mpsc::Receiver<()>) -> Self {
        Timeout {
            deadline: Self::get_deadline(),
            reset_rx,
        }
    }

    fn get_deadline() -> Instant {
        let timeout_ms = rand::rng().random_range(150..=300);
        Instant::now() + Duration::from_millis(timeout_ms as u64)
    }
}

impl Future for Timeout {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if Instant::now() > this.deadline {
            return Poll::Ready(());
        }

        match this.reset_rx.poll_recv(cx) {
            Poll::Ready(Some(())) => {
                this.deadline = Self::get_deadline();
                Poll::Pending
            }
            Poll::Ready(None) => Poll::Ready(()),
            Poll::Pending => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}
