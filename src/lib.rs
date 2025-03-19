mod constants;
mod codec;

use anyhow::Result;
use futures::{FutureExt, Stream, StreamExt};
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
            match this.swarm.poll_next_unpin(cx)  {
                Poll::Ready(Some(_e)) =>  {
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
        Self { id, number_of_nodes }
    }
}


// TODO: Implements stream
pub struct Swarm {
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
        let handles = Self::init_connections(&config).await?;
        let handler = Handler { handles };

        Ok(Self {
            config,
            listener,
            handler,
            state: NodeState::new(),
        })
    }

    async fn init_connections(config: &Config) -> Result<Vec<Handle>> {
        let mut handles = Vec::new();
        for i in config.id..=config.number_of_nodes {
            handles.push(Self::init_connection(i).await?);
        }
        Ok(handles)
    }

    async fn init_connection(id: u32) -> Result<Handle> {
        let stream = TcpStream::connect(
            SocketAddr::from_str(format!("127.0.0.1:{}", 8000 + id).as_str()).unwrap(),
        )
        .await?;
        let (command_tx, command_rx) = mpsc::channel(10);
        let (event_tx, event_rx) = mpsc::channel(10);
        // Create the session and spawn it
        let handle = Handle {
            peer_id: id,
            command_tx,
            event_rx,
        };

        tokio::spawn(Session {
            peer_id: id,
            stream,
            event_tx,
            command_rx,
        });

        Ok(handle)
    }
}

enum SwarmEvent {}
impl Stream for Swarm {
    type Item = SwarmEvent;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        // Poll Listener
        // Poll Handler
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


        // TODO: Poll Connection

        Poll::Pending
    }
}
enum ListenerEvent {}


struct Handler {
    handles: Vec<Handle>,
}

impl Handler {}

struct Handle {
    peer_id: u32,
    command_tx: mpsc::Sender<SessionCommand>, // Send to Session
    event_rx: mpsc::Receiver<SessionEvent>,   // Receive from Session
}

struct Session {
    stream: TcpStream,
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
}

struct LogEntry {
    index: usize,     // Position in the log
    term: u64,        // Term when entry was created
    command: Command, // The operation to apply
}

struct AppendEntries {
    term: u64,              // Leader’s term
    leader_id: u32,         // Leader’s ID
    prev_log_index: usize,  // Index of log entry before new ones
    prev_log_term: u64,     // Term of prev_log_index
    entries: Vec<LogEntry>, // New entries to append (empty for heartbeat)
    leader_commit: usize,   // Leader’s commit index
}

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
enum SessionEvent {}
