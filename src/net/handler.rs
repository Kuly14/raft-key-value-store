use crate::net::{
    primitives::{AppendEntries, Message},
    session::{SessionCommand, SessionEvent},
};
use anyhow::Result;
use futures::Stream;
use std::{
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::mpsc;
use tracing::info;

use super::primitives::{AppendResponse, VoteRequest, VoteResponse};

pub(crate) struct Handler {
    // PeerId -> Handle
    pub(crate) handles: HashMap<u32, Handle>,
    pub(crate) sessions_tx: mpsc::Sender<SessionEvent>,
    pub(crate) sessions_rx: mpsc::Receiver<SessionEvent>,
}

impl Handler {
    pub(crate) fn send_vote_request(&mut self, message: Message) -> Result<()> {
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
                    return Poll::Ready(Some(HandlerEvent::AppendResponse(append_response)))
                }
            }
        }

        Poll::Pending
    }
}

pub(crate) enum HandlerEvent {
    ReceivedEntries(AppendEntries),
    AppendResponse(AppendResponse),
    VoteResponse(VoteResponse),
    VoteRequest(VoteRequest),
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
