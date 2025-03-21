use futures::FutureExt;

use crate::net::{
    primitives::{AppendEntries, LogEntry, Role},
    timeout::Timeout,
};
use std::{
    collections::HashMap,
    task::{Context, Poll},
};

use super::primitives::{Message, VoteRequest, VoteResponse};

#[derive(Debug)]
pub(crate) struct NodeState {
    /// Current leader
    current_leader: Option<u32>,
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
    timeout: Timeout,
}

impl NodeState {
    pub(crate) fn new() -> Self {
        Self {
            role: Role::Follower,
            current_term: 0,
            voted_for: None,
            current_leader: None,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            kv_store: HashMap::new(),
            timeout: Timeout::new(),
        }
    }

    pub(crate) fn increment_term(&mut self) {
        self.current_term += 1;
        self.voted_for = None;
    }

    pub(crate) fn last_log_term(&self) -> u64 {
        match self.log.last() {
            Some(entry) => entry.term(),
            None => 0,
        }
    }

    pub(crate) fn last_log_index(&self) -> usize {
        self.log.len()
    }

    pub(crate) fn create_vote_request(&self, id: u32) -> Message {
        Message::VoteRequeset(VoteRequest {
            term: self.current_term,
            candidate_id: id,
            last_log_index: self.log.len(),
            last_log_term: self.last_log_term(),
        })
    }

    pub(crate) fn leader(&self) -> &Option<u32> {
        &self.current_leader
    }

    // TODO: Handle the rest of the entry
    pub(crate) fn handle_entry(&mut self, mut entry: AppendEntries) {
        self.current_term = if self.current_term < entry.term {
            entry.term
        } else {
            self.current_term
        };
        self.log.append(&mut entry.entries);
    }

    // TODO: Unit tests to see if the logic is correct
    pub(crate) fn handle_vote_request(&mut self, vote_request: VoteRequest) -> Message {
        let VoteRequest {
            term,
            candidate_id,
            last_log_term,
            last_log_index,
        } = vote_request;

        // Reject if candidate’s term is outdated
        if term < self.current_term {
            return Message::VoteResponse(VoteResponse::new(self.current_term, false));
        }

        // Update term and reset vote if candidate’s term is newer
        if term > self.current_term {
            self.current_term = term;
            self.voted_for = None;
        }

        // Check if we can vote and if log is up-to-date
        let vote_granted = (self.voted_for.is_none() || self.voted_for == Some(candidate_id))
            && ((last_log_term > self.last_log_term())
                || (last_log_term == self.last_log_term()
                    && last_log_index >= self.last_log_index()));

        if vote_granted {
            self.voted_for = Some(candidate_id);
        }

        Message::VoteResponse(VoteResponse::new(self.current_term, vote_granted))
    }

    // pub(crate) fn handle_vote_request(&mut self, vote_request: VoteRequest) -> Message {
    //     let VoteRequest {
    //         term,
    //         candidate_id,
    //         last_log_term,
    //         last_log_index,
    //     } = vote_request;
    //     // Uses Raft Consensus Rules to either accept or reject the request
    //     let vote_granted = self.voted_for.is_none()
    //         || self.voted_for == Some(candidate_id)
    //             && term >= self.current_term
    //             && ((last_log_term > self.last_log_term())
    //                 || (last_log_term == self.last_log_term())
    //                     && last_log_index >= self.last_log_index())
    //             && matches!(self.role, Role::Follower);
    //
    //     if vote_granted {
    //         self.voted_for = Some(candidate_id);
    //     }
    //
    //     Message::VoteResponse(VoteResponse::new(self.current_term, vote_granted))
    // }

    pub(crate) fn set_new_leader(&mut self, id: Option<u32>) {
        self.current_leader = id;
    }

    /// When [Timeout] future resolves, we need to create a new one
    pub(crate) fn reset_timeout(&mut self) {
        self.timeout.reset();
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
