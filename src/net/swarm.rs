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
    primitives::{AppendEntries, Message, Role, VoteRequest, VoteResponse},
    session::{SessionCommand, pending_session},
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

        let state = NodeState::new(&config);

        // init the connections
        let (sessions_tx, sessions_rx) = mpsc::channel(100);
        let (pending_tx, pending_rx) = mpsc::channel(10);
        let handler = Handler {
            handles: HashMap::new(),
            sessions_rx,
            sessions_tx,
            pending_tx,
            pending_rx,
        };
        let swarm = Self {
            config,
            listener,
            handler,
            state,
        };

        swarm.init_connections().await?;

        Ok(swarm)
    }
    pub(crate) fn state(&self) -> &NodeState {
        &self.state
    }

    async fn init_connections(&self) -> Result<()> {
        for id in self.config.id + 1..=self.config.number_of_nodes - 1 {
            self.init_connection(id).await?;
        }
        Ok(())
    }

    async fn init_connection(&self, id: u32) -> anyhow::Result<()> {
        let addr = format!("127.0.0.1:{}", 8000 + id)
            .parse::<SocketAddr>()
            .unwrap();

        info!(address=?addr, "Initializing connection");
        let stream = TcpStream::connect(addr).await?;
        tokio::spawn(pending_session(
            stream,
            self.config.local_addr,
            self.handler.pending_tx.clone(),
        ));
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

    pub(crate) fn handle_vote_response(&mut self, vote_response: VoteResponse) -> bool {
        self.state.handle_vote_response(vote_response)
    }

    pub(crate) fn handle_entries(&mut self, entries: AppendEntries) -> bool {
        self.state.handle_entries(entries)
    }

    pub(crate) fn handle_timer_elapsed_as_leader(&mut self) {
        self.state.increment_term();
        // TODO: Drain the entries from client and build the AppendEntries struct here

        let entries = AppendEntries {
            term: self.state.current_term(),
            entries: Vec::new(),
            leader_commit: 0,
            // TODO: Make this dynamic as it should be
            leader_id: self.state.id(),
            prev_log_term: 0,
            prev_log_index: 0,
        };

        // TODO: Handle error
        let _ = self
            .handler
            .send_message_to_all(Message::AppendEntries(entries));
        self.state.reset_timeout();
    }

    /// The [crate::net::timeout::Timeout] future resolved as Poll::Ready, thus we need to spawn a new one
    pub(crate) fn handle_timer_elapsed_as_follower(&mut self) {
        self.state.increment_term();

        let message = self.state.create_vote_request(self.config.id);
        // TODO: When Receive message reset timer

        // TODO: Handle this
        self.state.vote_for_self(self.config.id);
        let _ = self.handler.send_message_to_all(message);

        info!("{:?}", self.state().role());
        info!("{:?}", self.state().vote_count());

        // No leader we need to start election
        // Timer elapsed we need to start an election and spawn the future again
        self.state.reset_timeout();
        // Have to poll the future, to register it with the waker
    }

    /// Sends message over the reset_tx channel to the timeout to reset the timer
    pub(crate) fn restart_timeout(&mut self) {
        // TODO: Handle Error
        let _ = self.state.timeout().reset_tx.try_send(());
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
                    tokio::spawn(pending_session(
                        stream,
                        this.config.local_addr,
                        this.handler.pending_tx.clone(),
                    ));
                    return Poll::Ready(Some(SwarmEvent::NewConnection { addr }));
                }
                ListenerEvent::ConnectionError(_e) => {
                    todo!("handle error");
                }
            }
        }

        // Poll Handler
        while let Poll::Ready(Some(event)) = this.handler.poll_next_unpin(cx) {
            // TODO: PRINT THE MESSAGES TO SEE IOF THE LEADER IS ALWAYS THE ONE 
            match event {
                HandlerEvent::NewSession { stream, addr } => this.spawn_session(stream, addr),
                HandlerEvent::ReceivedEntries(entries) => {
                    if this.handle_entries(entries) {
                        this.restart_timeout();
                    }
                }
                HandlerEvent::AppendResponse(append_response) => (),
                HandlerEvent::VoteRequest(vote_request) => this.handle_vote_request(vote_request),
                HandlerEvent::VoteResponse(vote_response) => {
                    if this.handle_vote_response(vote_response) {
                        this.restart_timeout();
                    }
                }
            }
        }

        // Poll State if async
        if let Poll::Ready(state_event) = this.state.poll(cx) {
            match state_event {
                StateEvent::TimerElapsed => {
                    match this.state().role() {
                        Role::Leader => {
                            this.handle_timer_elapsed_as_leader();
                        }
                        Role::Follower | Role::Candidate => this.handle_timer_elapsed_as_follower(),
                    }
                    let _ = this.state.poll(cx);
                }
            }
        }

        Poll::Pending
    }
}
