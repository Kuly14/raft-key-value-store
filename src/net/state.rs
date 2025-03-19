use crate::net::primitives::{AppendEntries, LogEntry, Role};
use std::collections::HashMap;

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
}
