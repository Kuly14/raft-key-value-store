use crate::net::{codec::MessageCodec, primitives::Message};
use futures::{SinkExt, StreamExt};
use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
    net::SocketAddr,
};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::codec::Framed;
use tracing::info;

pub(crate) struct Session {
    pub(crate) stream: Framed<TcpStream, MessageCodec>,
    pub(crate) peer_id: u32,
    pub(crate) event_tx: mpsc::Sender<SessionEvent>, // Send to hanlder
    pub(crate) command_rx: mpsc::Receiver<SessionCommand>, // receive from Handler
    pub(crate) queued_messages: VecDeque<Message>,
}

// TODO: Error handling
impl Future for Session {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        // Drain the channel

        while let Poll::Ready(Some(command)) = this.command_rx.poll_recv(cx) {
            match command {
                SessionCommand::Send(msg) => {
                    this.queued_messages.push_back(msg);
                }
            }
        }

        // Poll the stream
        // TODO: Handle errors
        while let Poll::Ready(Some(Ok(msg))) = this.stream.poll_next_unpin(cx) {
            info!("Received Message From Peer: {:#?}", msg);
            // TODO: Handle errors
            let _ = this.event_tx.try_send(SessionEvent::ReceivedData(msg));
        }
        while let Poll::Ready(inner) = this.stream.poll_ready_unpin(cx) {
            // TODO: Handle errors
            if let Err(e) = inner {
                // error!(err=%e);
                // pin.disconnect();
                return Poll::Ready(());
            }
            match this.queued_messages.pop_front() {
                Some(msg) => {
                    if let Err(e) = this.stream.start_send_unpin(msg) {
                        // error!(err=%e);
                        // pin.disconnect();
                        return Poll::Ready(());
                    }
                }
                None => break,
            }
        }

        if let Poll::Ready(Err(_e)) = this.stream.poll_flush_unpin(cx) {
            // error!(err=%e);
            // pin.disconnect();
            return Poll::Ready(());
        }

        Poll::Pending
    }
}

pub(crate) enum SessionCommand {
    Send(Message),
}
#[derive(Debug)]
// TODO: Change this so I don't have to propagate message
pub(crate) enum SessionEvent {
    ReceivedData(Message),
}

pub(crate) async fn pending_session(
    stream: TcpStream,
    local_addr: SocketAddr,
    sender: mpsc::Sender<PendingSessionEvent>,
) -> Option<()> {
    let mut stream = Framed::new(stream, MessageCodec::new());
    info!("IN PENDING SESSION");

    stream
        .send(Message::Hello { local_addr })
        .await
        .ok()?;

    if let x @ Message::Hello { local_addr } = stream.next().await?.ok()? {
        info!("RECEIVED IN PENDING SESSION: {:?}", x);
        sender
            .send(PendingSessionEvent::Established { stream, addr: local_addr })
            .await
            .ok()?;
    }
    None
}

pub enum PendingSessionEvent {
    Established {
        stream: Framed<TcpStream, MessageCodec>,
        addr: SocketAddr,
    },
}
