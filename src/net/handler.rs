use crate::net::{
    primitives::{AppendEntries, Message},
    session::{SessionCommand, SessionEvent},
};
use futures::Stream;
use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::mpsc;

pub(crate) struct Handler {
    // PeerId -> Handle
    pub(crate) handles: HashMap<u32, Handle>,
    pub(crate) sessions_tx: mpsc::Sender<SessionEvent>,
    pub(crate) sessions_rx: mpsc::Receiver<SessionEvent>,
}

impl Handler {}

impl Stream for Handler {
    type Item = HandlerEvent;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        while let Poll::Ready(Some(event)) = this.sessions_rx.poll_recv(cx) {
            match event {
                SessionEvent::ReceivedData(Message::AppendEntries(entries)) => {
                    return Poll::Ready(Some(HandlerEvent::ReceivedEntries(entries)));
                }
                SessionEvent::Vote => (),
                _ => (),
            }
        }

        Poll::Pending
    }
}

pub(crate) enum HandlerEvent {
    ReceivedEntries(AppendEntries),
}

pub(crate) struct Handle {
    pub(crate) peer_id: u32,
    pub(crate) command_tx: mpsc::Sender<SessionCommand>, // Send to Session
}

impl Handle {
    pub(crate) fn new(peer_id: u32, command_tx: mpsc::Sender<SessionCommand>) -> Self {
        Self {
            peer_id,
            command_tx,
        }
    }
}
