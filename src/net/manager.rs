use crate::net::swarm::Swarm;
use futures::StreamExt;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pub(crate) struct NetworkManager {
    swarm: Swarm,
}
impl NetworkManager {
    pub(crate) fn new(swarm: Swarm) -> Self {
        NetworkManager { swarm }
    }
}

impl Future for NetworkManager {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // TODO: Handle Swarm Event and add logging
        loop {
            match this.swarm.poll_next_unpin(cx) {
                Poll::Ready(Some(_e)) => {}
                Poll::Ready(None) => todo!("Not yet sure when it would be none"),
                Poll::Pending => break,
            }
        }

        Poll::Pending
    }
}
