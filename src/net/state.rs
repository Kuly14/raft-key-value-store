use futures::FutureExt;
use tokio::sync::mpsc;

use crate::net::{primitives::{AppendEntries, LogEntry, Role}, timeout::Timeout};
use std::{collections::HashMap, task::{Context, Poll}};

pub(crate) struct NodeState {
    /// Leader, Follower, Candidate
    role: Role,
    /// Current term
    current_term: u64,
    /// Node ID voted for in this term
    voted_for: Option<u32>,
    /// Replicated log
    log: Vec<LogEntry>,
    /// Highest committed entry
    commit_index: usize,
    /// Last entry applied to KV store
    last_applied: usize,
    /// Key-value store
    kv_store: HashMap<String, String>,
    /// Timeout tracker
    timeout: Timeout
}

impl NodeState {
    pub(crate) fn new() -> Self {
        Self {
            role: Role::Follower,
            current_term: 0,
            voted_for: None,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            kv_store: HashMap::new(),
            timeout: Timeout::new(),
        }
    }

    pub(crate) fn handle_entry(&mut self, mut entry: AppendEntries) {
        self.current_term = if self.current_term < entry.term {
            entry.term
        } else {
            self.current_term
        };
        self.log.append(&mut entry.entries);
    }
    pub(crate) fn poll(&mut self, cx: &mut Context<'_>) -> Poll<StateEvent> {
        // Poll timeout

        if let Poll::Ready(()) = self.timeout.poll_unpin(cx) {
            return Poll::Ready(StateEvent::TimerElapsed);
            // Timeout expired send an event that we should start election to swarm
        }

        Poll::Pending
    }
}

pub(crate) enum StateEvent {
    TimerElapsed,
}
