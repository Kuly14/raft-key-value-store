use crate::net::{
    codec::MessageCodec,
    handler::{Handle, HandlerEvent},
    session::Session,
};
use crate::net::{
    config::Config,
    handler::Handler,
    listener::{Listener, ListenerEvent},
    session::SessionEvent,
    state::NodeState,
};
use anyhow::Result;
use futures::{Stream, StreamExt};
use std::{
    collections::{HashMap, VecDeque},
    net::SocketAddr,
    pin::Pin,
    str::FromStr,
    task::{Context, Poll},
};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::codec::Framed;
use tracing::info;

use super::{
    primitives::{AppendEntries, VoteRequest},
    session::{pending_session, SessionCommand},
    state::StateEvent,
};

#[derive(Debug)]
pub struct Swarm {
    config: Config,
    listener: Listener,
    handler: Handler,
    state: NodeState,
}

impl Swarm {
    pub(crate) async fn new(config: Config) -> Result<Self> {
        // Init the connections
        let listener = Listener::bind(
            SocketAddr::from_str(format!("127.0.0.1:{}", 8000 + config.id).as_str()).unwrap(),
        )
        .await?;

        let state = NodeState::new();

        // init the connections
        let (sessions_tx, sessions_rx) = mpsc::channel(100);
        let (pending_tx, pending_rx) = mpsc::channel(10);
        let handler = Handler {
            handles: HashMap::new(),
            sessions_rx,
            sessions_tx,
            pending_tx,
            pending_rx
        };
        let swarm = Self {
            config,
            listener,
            handler,
            state,
        };

        swarm.init_connections().await?;
        info!("HANDLER: {:?}", swarm.handler);

        Ok(swarm)
    }

    async fn init_connections(
        &self,
    ) -> Result<()> {
        for id in self.config.id + 1..=self.config.number_of_nodes - 1 {
            self.init_connection(id).await?;
        }
        Ok(())
    }

    async fn init_connection(&self, id: u32) -> anyhow::Result<()>  {
        let addr = format!("127.0.0.1:{}", 8000 + id)
            .parse::<SocketAddr>()
            .unwrap();

        info!(address=?addr, "Initializing connection");
        let stream = TcpStream::connect(addr).await?;
        tokio::spawn(pending_session(stream, self.config.local_addr, self.handler.pending_tx.clone()));
        Ok(())
    }

    fn get_handle_and_session(
        stream: Framed<TcpStream, MessageCodec>,
        peer_id: u32,
        sessions_tx: &mpsc::Sender<SessionEvent>,
    ) -> (Handle, Session) {
        let (command_tx, command_rx) = mpsc::channel(100);

        let handle = Handle::new(peer_id, command_tx);

        let session = Session {
            stream,
            peer_id,
            event_tx: sessions_tx.clone(),
            command_rx,
            queued_messages: VecDeque::new(),
        };

        (handle, session)
    }

    pub fn spawn_session(&mut self, stream: Framed<TcpStream, MessageCodec>, addr: SocketAddr) {
        //TODO: HANDLE THIS UNWRAP
        let id = addr.port() as u32 - 8000;
        let (handle, session) = Self::get_handle_and_session(stream, id, &self.handler.sessions_tx);
        tokio::spawn(session);
        println!("HANDLE: {:?}", handle);
        self.handler.handles.insert(handle.peer_id, handle);
    }

    pub(crate) fn handle_vote_request(&mut self, vote_request: VoteRequest) {
        let candidate_id = vote_request.candidate_id;
        let response = self.state.handle_vote_request(vote_request);
        if let Some(handle) = self.handler.handles.get(&candidate_id) {
            // TODO: Handle error
            let _ = handle.command_tx.try_send(SessionCommand::Send(response));
        } else {
            unreachable!("Should be unreachable: {:#?}", self.handler.handles.keys())
        }
    }
    pub(crate) fn handle_entry(&mut self, entry: AppendEntries) {
        self.state.handle_entry(entry);
    }
    pub(crate) fn handle_timer_elapsed(&mut self) {
        self.state.increment_term();
        let message = self.state.create_vote_request(self.config.id);
        // TODO: When Receive message reset timer

        // TODO: Handle this
        let _ = self.handler.send_vote_request(message);

        // No leader we need to start election
        // Timer elapsed we need to start an election and spawn the future again
        self.state.reset_timeout();
        // Have to poll the future, to register it with the waker
    }

}

#[derive(Debug)]
pub enum SwarmEvent {
    NewConnection { addr: SocketAddr },
}

impl Stream for Swarm {
    type Item = SwarmEvent;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        // Poll Listener
        // TODO: Handle edge cases and errors
        while let Poll::Ready(Some(event)) = this.listener.poll_next_unpin(cx) {
            match event {
                ListenerEvent::NewConnection { stream, addr } => {
                    tokio::spawn(pending_session(stream, this.config.local_addr, this.handler.pending_tx.clone()));
                    return Poll::Ready(Some(SwarmEvent::NewConnection { addr }));
                }
                ListenerEvent::ConnectionError(_e) => {
                    todo!("handle error");
                }
            }
        }

        // Poll Handler
        while let Poll::Ready(Some(event)) = this.handler.poll_next_unpin(cx) {
            this.state.reset_timeout();
            let _ = this.state.poll(cx);
            match event {
                HandlerEvent::ReceivedEntries(entries) => this.handle_entry(entries),
                HandlerEvent::AppendResponse(append_response) => (),
                HandlerEvent::VoteRequest(vote_request) => this.handle_vote_request(vote_request),
                HandlerEvent::VoteResponse(vote_response) => (),
                HandlerEvent::NewSession { stream, addr } => this.spawn_session(stream, addr),
            }
        }
        // Poll State if async
        if let Poll::Ready(state_event) = this.state.poll(cx) {
            match state_event {
                StateEvent::TimerElapsed => {
                    this.handle_timer_elapsed();
                    let _ = this.state.poll(cx);
                }
            }
        }

        Poll::Pending
    }
}
