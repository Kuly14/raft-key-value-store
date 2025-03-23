use crate::net::{
    primitives::{AppendEntries, Message},
    session::{SessionCommand, SessionEvent},
};
use anyhow::Result;
use futures::Stream;
use std::{
    collections::HashMap,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{net::TcpSocket, net::TcpStream, sync::mpsc};
use tokio_util::codec::Framed;
use tracing::info;

use super::{
    codec::MessageCodec,
    primitives::{AppendResponse, VoteRequest, VoteResponse},
    session::PendingSessionEvent,
};

#[derive(Debug)]
pub(crate) struct Handler {
    // PeerId -> Handle
    pub(crate) handles: HashMap<u32, Handle>,
    pub(crate) sessions_tx: mpsc::Sender<SessionEvent>,
    pub(crate) sessions_rx: mpsc::Receiver<SessionEvent>,
    pub(crate) pending_tx: mpsc::Sender<PendingSessionEvent>,
    pub(crate) pending_rx: mpsc::Receiver<PendingSessionEvent>,
}

impl Handler {
    pub(crate) fn send_message_to_all(&mut self, message: Message) -> Result<()> {
        for handle in self.handles.values() {
            let command = SessionCommand::Send(message.clone());
            handle.command_tx.try_send(command)?;
        }
        Ok(())
    }
}

impl Stream for Handler {
    type Item = HandlerEvent;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // TODO: Handle errors here
        if let Poll::Ready(Some(event)) = this.sessions_rx.poll_recv(cx) {
            match event {
                SessionEvent::ReceivedData(Message::AppendEntries(entries)) => {
                    return Poll::Ready(Some(HandlerEvent::ReceivedEntries(entries)));
                }
                // TODO: Handle received data
                // TODO: Maybe change Session Events To Other types so this isn't so weird
                SessionEvent::ReceivedData(Message::VoteRequeset(vote_request)) => {
                    return Poll::Ready(Some(HandlerEvent::VoteRequest(vote_request)));
                }

                SessionEvent::ReceivedData(Message::VoteResponse(vote_response)) => {
                    return Poll::Ready(Some(HandlerEvent::VoteResponse(vote_response)));
                }

                SessionEvent::ReceivedData(Message::AppendResponse(append_response)) => {
                    return Poll::Ready(Some(HandlerEvent::AppendResponse(append_response)));
                }
                SessionEvent::ReceivedData(Message::Hello { .. }) => unreachable!(),
            }
        }

        if let Poll::Ready(Some(PendingSessionEvent::Established { stream, addr })) =
            this.pending_rx.poll_recv(cx)
        {
            return Poll::Ready(Some(HandlerEvent::NewSession { stream, addr }));
        }

        Poll::Pending
    }
}

pub(crate) enum HandlerEvent {
    ReceivedEntries(AppendEntries),
    AppendResponse(AppendResponse),
    VoteResponse(VoteResponse),
    VoteRequest(VoteRequest),
    NewSession {
        stream: Framed<TcpStream, MessageCodec>,
        addr: SocketAddr,
    },
}

#[derive(Debug)]
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
