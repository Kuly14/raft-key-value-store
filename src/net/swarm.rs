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
    collections::HashMap,
    net::SocketAddr,
    pin::Pin,
    str::FromStr,
    task::{Context, Poll},
};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::codec::Framed;

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

        let handle = Handle::new(peer_id, command_tx);

        let session = Session {
            stream: Framed::new(stream, MessageCodec::new()),
            peer_id,
            event_tx: sessions_tx.clone(),
            command_rx,
        };

        (handle, session)
    }
}

pub enum SwarmEvent {}
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
