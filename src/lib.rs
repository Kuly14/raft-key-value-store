mod codec;
mod constants;

use anyhow::Result;
use codec::{Message, MessageCodec};
use futures::ready;
use futures::{FutureExt, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::SocketAddr,
    pin::Pin,
    str::FromStr,
    task::{Context, Poll},
};
use tokio::sync::mpsc;
use tokio::{
    net::{TcpListener, TcpStream},
    signal::ctrl_c,
};
use tokio_util::codec::Framed;

pub async fn run(config: Config) -> Result<()> {
    let swarm = Swarm::new(config).await?;
    let manager = NetworkManager::new(swarm);
    tokio::spawn(manager);

    let _ = ctrl_c().await;
    Ok(())
}

// TODO: Implements Future so it can poll Swarm
struct NetworkManager {
    swarm: Swarm,
}
impl NetworkManager {
    fn new(swarm: Swarm) -> Self {
        NetworkManager { swarm }
    }
}

impl Future for NetworkManager {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        loop {
            match this.swarm.poll_next_unpin(cx) {
                Poll::Ready(Some(_e)) => {
                    todo!("handle the event");
                }
                Poll::Ready(None) => todo!("Not yet sure when it would be none"),
                Poll::Pending => break,
            }
        }

        Poll::Pending
    }
}

pub struct Config {
    id: u32,
    number_of_nodes: u32,
}
impl Config {
    pub fn new(id: u32, number_of_nodes: u32) -> Self {
        Self {
            id,
            number_of_nodes,
        }
    }
}

// TODO: Implements stream
struct Swarm {
    config: Config,
    listener: Listener,
    handler: Handler,
    state: NodeState,
}

impl Swarm {
    async fn new(config: Config) -> Result<Self> {
        // Init the connections
        let listener = Listener::bind(
            SocketAddr::from_str(format!("127.0.0.1:{}", 8000 + config.id).as_str()).unwrap(),
        )
        .await?;

        // init the connections
        let (sessions_tx, sessions_rx) = mpsc::channel(100);
        let mut handler = Handler {
            handles: HashMap::new(),
            sessions_rx,
            sessions_tx,
        };
        let handles = Self::init_connections(&config, &handler.sessions_tx).await?;
        handler.handles = handles;

        Ok(Self {
            config,
            listener,
            handler,
            state: NodeState::new(),
        })
    }

    async fn init_connections(
        config: &Config,
        sessions_tx: &mpsc::Sender<SessionEvent>,
    ) -> Result<HashMap<u32, Handle>> {
        let mut handles = HashMap::new();
        for i in config.id..=config.number_of_nodes {
            let id = 8000 + i;
            handles.insert(id, Self::init_connection(id, sessions_tx).await?);
        }
        Ok(handles)
    }

    async fn init_connection(id: u32, sessions_tx: &mpsc::Sender<SessionEvent>) -> Result<Handle> {
        let addr = format!("127.0.0.1:{}", 8000 + id)
            .parse::<SocketAddr>()
            .unwrap();
        let stream = TcpStream::connect(addr).await?;
        let (handle, session) = Swarm::get_handle_and_session(stream, addr, sessions_tx);
        tokio::spawn(session);
        Ok(handle)
    }

    fn get_handle_and_session(
        stream: TcpStream,
        addr: SocketAddr,
        sessions_tx: &mpsc::Sender<SessionEvent>,
    ) -> (Handle, Session) {
        let peer_id = addr.port() as u32 - 8000;
        let (command_tx, command_rx) = mpsc::channel(100);

        let handle = Handle {
            peer_id,
            command_tx,
        };

        let session = Session {
            stream: Framed::new(stream, MessageCodec::new()),
            peer_id,
            event_tx: sessions_tx.clone(),
            command_rx,
        };

        (handle, session)
    }
}

enum SwarmEvent {}
impl Stream for Swarm {
    type Item = SwarmEvent;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        // Poll Listener
        // TODO: Handle edge cases and errors
        while let Poll::Ready(Some(ListenerEvent::NewConnection { stream, addr })) =
            this.listener.poll_next_unpin(cx)
        {
            let (handle, session) =
                Self::get_handle_and_session(stream, addr, &this.handler.sessions_tx);
            tokio::spawn(session);
            this.handler.handles.insert(handle.peer_id, handle);
        }

        // Poll Handler
        while let Poll::Ready(Some(event)) = this.handler.poll_next_unpin(cx) {
            match event {
                HandlerEvent::ReceivedEntries(entries) => this.state.handle_entry(entries),
            }
        }
        // Poll State if async

        Poll::Pending
    }
}

struct Listener {
    // TODO: Implement Framed
    conn: TcpListener,
    local_addr: SocketAddr,
}

impl Listener {
    fn new(conn: TcpListener, local_addr: SocketAddr) -> Self {
        Self { conn, local_addr }
    }

    async fn bind(addr: SocketAddr) -> Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        Ok(Self::new(listener, addr))
    }
}

impl Stream for Listener {
    type Item = ListenerEvent;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        match ready!(this.conn.poll_accept(cx)) {
            Ok((stream, addr)) => Poll::Ready(Some(ListenerEvent::NewConnection { stream, addr })),
            Err(e) => Poll::Ready(Some(ListenerEvent::ConnectionError(e.into()))),
        }
    }
}
enum ListenerEvent {
    NewConnection { stream: TcpStream, addr: SocketAddr },
    ConnectionError(anyhow::Error),
}

struct Handler {
    // PeerId -> Handle
    handles: HashMap<u32, Handle>,
    sessions_tx: mpsc::Sender<SessionEvent>,
    sessions_rx: mpsc::Receiver<SessionEvent>,
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
            }
        }

        Poll::Pending
    }
}

enum HandlerEvent {
    ReceivedEntries(AppendEntries),
}

struct Handle {
    peer_id: u32,
    command_tx: mpsc::Sender<SessionCommand>, // Send to Session
}

struct Session {
    stream: Framed<TcpStream, MessageCodec>,
    peer_id: u32,
    event_tx: mpsc::Sender<SessionEvent>, // Send to hanlder
    command_rx: mpsc::Receiver<SessionCommand>, // receive from Handler
}

impl Future for Session {
    type Output = SessionEvent;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

struct NodeState {
    role: Role,                        // Leader, Follower, Candidate
    current_term: u64,                 // Current term
    voted_for: Option<u32>,            // Node ID voted for in this term
    log: Vec<LogEntry>,                // Replicated log
    commit_index: usize,               // Highest committed entry
    last_applied: usize,               // Last entry applied to KV store
    kv_store: HashMap<String, String>, // Key-value store
}

impl NodeState {
    fn new() -> Self {
        Self {
            role: Role::Follower,
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            kv_store: HashMap::new(),
        }
    }

    fn handle_entry(&mut self, mut entry: AppendEntries) {
        self.current_term = entry.term;
        self.log.append(&mut entry.entries);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct LogEntry {
    /// Position in the log
    index: usize,
    /// Term when entry was created
    term: u64,
    /// The operation to apply
    command: Command,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AppendEntries {
    /// Leader’s term
    term: u64,
    /// Leader’s ID
    leader_id: u32,
    /// Index of log entry before new ones
    prev_log_index: usize,
    /// Term of prev_log_index
    prev_log_term: u64,
    /// New entries to append (empty for heartbeat)
    entries: Vec<LogEntry>,
    /// Leader’s commit index
    leader_commit: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
enum Command {
    Put { key: String, value: String },
    Get { key: String },
}

enum Role {
    Leader,
    Follower,
    Candidate,
}

enum SessionCommand {}
enum SessionEvent {
    ReceivedData(Message),
    Vote,
}
