use crate::net::{codec::MessageCodec, primitives::Message};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::codec::Framed;

pub(crate) struct Session {
    pub(crate) stream: Framed<TcpStream, MessageCodec>,
    pub(crate) peer_id: u32,
    pub(crate) event_tx: mpsc::Sender<SessionEvent>, // Send to hanlder
    pub(crate) command_rx: mpsc::Receiver<SessionCommand>, // receive from Handler
}

impl Future for Session {
    type Output = SessionEvent;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

pub(crate) enum SessionCommand {}
pub(crate) enum SessionEvent {
    ReceivedData(Message),
    Vote,
}
