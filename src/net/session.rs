use tracing::info;
use crate::net::{codec::MessageCodec, primitives::Message};
use futures::StreamExt;
use std::{
    collections::VecDeque,
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
    pub(crate) queed_message: VecDeque<Message>,
}

impl Future for Session {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        // Drain the channel

        while let Poll::Ready(Some(command)) = this.command_rx.poll_recv(cx) {
            match command {
                SessionCommand::Send(msg) => {
                    this.queed_message.push_back(msg);
                }
            }
        }

        // Poll the stream
        // TODO: Handle errors
        while let Poll::Ready(Some(Ok(msg))) = this.stream.poll_next_unpin(cx) {
            match msg {
                Message::AppendEntries(entries) => {}
                Message::AppendResponse { term, success } => {}
                Message::RequestVote {
                    term,
                    candidate_id,
                    last_log_index,
                    last_log_term,
                } => {}
                Message::VoteResponse { term, vote_granted } => {}
            }
        }
        Poll::Pending
    }
}

pub(crate) enum SessionCommand {
    Send(Message),
}
pub(crate) enum SessionEvent {
    ReceivedData(Message),
    Vote,
}
