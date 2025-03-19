use anyhow::Result;
use futures::Stream;
use futures::ready;
use std::{
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::net::{TcpListener, TcpStream};

pub(crate) struct Listener {
    conn: TcpListener,
    local_addr: SocketAddr,
}

impl Listener {
    pub(crate) fn new(conn: TcpListener, local_addr: SocketAddr) -> Self {
        Self { conn, local_addr }
    }

    pub(crate) async fn bind(addr: SocketAddr) -> Result<Self> {
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
pub(crate) enum ListenerEvent {
    NewConnection { stream: TcpStream, addr: SocketAddr },
    ConnectionError(anyhow::Error),
}
