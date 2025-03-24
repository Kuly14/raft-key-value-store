use crate::{
    Config,
    net::{
        primitives::{AppendEntries, LogEntry, Role},
        timeout::Timeout,
    },
};
use futures::FutureExt;
use std::{
    collections::HashMap,
    task::{Context, Poll},
};
use tracing::info;

use super::primitives::{Message, VoteRequest, VoteResponse};

#[derive(Debug)]
pub(crate) struct NodeState {
    /// Id of this Node
    id: u32,
    /// Cluster Size
    cluster_size: u32,
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
    /// Vote count for this node in the current term
    vote_count: u32,
}

impl NodeState {
    pub(crate) fn new(config: &Config) -> Self {
        Self {
            id: config.id,
            cluster_size: config.number_of_nodes,
            role: Role::Follower,
            current_term: 0,
            voted_for: None,
            current_leader: None,
            log: Vec::new(),
            commit_index: 0,
            last_applied: 0,
            kv_store: HashMap::new(),
            timeout: Timeout::new(),
            vote_count: 0,
        }
    }

    pub(crate) fn current_term(&self) -> u64 {
        self.current_term
    }

    pub(crate) fn increment_term(&mut self) {
        self.current_term += 1;
        self.vote_count = 0;
        self.voted_for = None;
    }

    pub(crate) fn timeout(&self) -> &Timeout {
        &self.timeout
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

    pub(crate) fn vote_for_self(&mut self, id: u32) {
        self.role = Role::Candidate;
        self.voted_for = Some(id);
    }

    pub(crate) fn create_vote_request(&self, id: u32) -> Message {
        Message::VoteRequeset(VoteRequest {
            term: self.current_term,
            candidate_id: id,
            last_log_index: self.log.len(),
            last_log_term: self.last_log_term(),
        })
    }

    // TODO: Handle the rest of the entry

    pub(crate) fn handle_entries(&mut self, entries: AppendEntries) -> bool {
        // Reject if the term is lower (stale leader)
        if entries.term < self.current_term {
            return false; // Don’t reset timer
        }

        // If term is higher, update our term and step down if leader
        if entries.term > self.current_term {
            self.current_term = entries.term;
            self.role = Role::Follower; 
            self.current_leader = Some(entries.leader_id);
            self.timeout.is_leader = false;
            return true;
        }

        // Term is equal: accept if no leader or matching leader
        if self.current_leader.is_none() {
            self.current_leader = Some(entries.leader_id);
            info!("SET NEW LEADER AS: {}", entries.leader_id);
            return true; // Reset timer, new leader accepted
        } else if self.current_leader == Some(entries.leader_id) {
            return true; // Reset timer, existing leader confirmed
        }

        // If term matches but leader differs, don’t reset timer
        false
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

    pub(crate) fn role(&self) -> &Role {
        &self.role
    }

    pub(crate) fn handle_vote_response(&mut self, vote_response: VoteResponse) -> bool {
        let VoteResponse { term, vote_granted } = vote_response;

        if !matches!(self.role, Role::Candidate) {
            return false;
        }

        if term > self.current_term {
            self.current_term = term;
            self.role = Role::Follower;
            self.voted_for = None;
            self.current_leader = None;
            self.vote_count = 0; // Reset since election is invalid
            return true;
        }

        if term < self.current_term {
            return false;
        }

        if vote_granted {
            self.vote_count += 1;

            if self.vote_count > self.cluster_size / 2 {
                self.initialize_leader_state();
                return true;
            }

            return false;
        }

        unreachable!()
    }

    pub(crate) fn initialize_leader_state(&mut self) {
        self.role = Role::Leader;
        self.timeout.is_leader = true;
        self.current_leader = Some(self.id());
        self.vote_count = 0;
        info!("BECOMING LEADER IN TERM: {}", self.current_term());
    }

    pub(crate) fn id(&self) -> u32 {
        self.id
    }

    pub(crate) fn set_new_leader(&mut self, id: Option<u32>) {
        self.current_leader = id;
    }

    /// When [Timeout] future resolves, we need to create a new one
    pub(crate) fn reset_timeout(&mut self) {
        self.timeout.reset();
    }

    pub(crate) fn poll(&mut self, cx: &mut Context<'_>) -> Poll<StateEvent> {
        if let Poll::Ready(()) = self.timeout.poll_unpin(cx) {
            // Timeout expired send an event that we should start election to swarm
            return Poll::Ready(StateEvent::TimerElapsed);
        }

        Poll::Pending
    }
}

pub(crate) enum StateEvent {
    TimerElapsed,
}
