use std::collections::HashMap;
use crate::net::primitives::{Role, LogEntry, AppendEntries};

pub(crate) struct NodeState {
    role: Role,                        // Leader, Follower, Candidate
    current_term: u64,                 // Current term
    voted_for: Option<u32>,            // Node ID voted for in this term
    log: Vec<LogEntry>,                // Replicated log
    commit_index: usize,               // Highest committed entry
    last_applied: usize,               // Last entry applied to KV store
    kv_store: HashMap<String, String>, // Key-value store
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
        self.current_term = entry.term;
        self.log.append(&mut entry.entries);
    }
}
