use std::net::SocketAddr;

use futures::{FutureExt, StreamExt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct LogEntry {
    /// Position in the log
    index: usize,
    /// Term when entry was created
    term: u64,
    /// The operation to apply
    command: Command,
}

impl LogEntry {
    pub(crate) fn new(index: usize, term: u64, command: Command) -> Self {
        Self {
            index,
            term,
            command,
        }
    }
    pub(crate) fn term(&self) -> u64 {
        self.term
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct AppendEntries {
    /// Leader’s term
    pub(crate) term: u64,
    /// Leader’s ID
    pub(crate) leader_id: u32,
    /// Index of log entry before new ones
    pub(crate) prev_log_index: usize,
    /// Term of prev_log_index
    pub(crate) prev_log_term: u64,
    /// New entries to append (empty for heartbeat)
    pub(crate) entries: Vec<LogEntry>,
    /// Leader’s commit index
    pub(crate) leader_commit: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) enum Command {
    Put { key: String, value: String },
    Get { key: String },
}

#[derive(Debug)]
pub(crate) enum Role {
    Leader,
    Follower,
    Candidate,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) enum Message {
    AppendEntries(AppendEntries),
    AppendResponse(AppendResponse),
    VoteRequeset(VoteRequest),
    VoteResponse(VoteResponse),
    Hello { local_addr: SocketAddr },
}

impl Message {
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(self).unwrap();
        bytes.push(b'\r');
        bytes.push(b'\n');
        bytes
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct VoteRequest {
    /// Candidate’s term
    pub(crate) term: u64,
    /// Candidate’s ID
    pub(crate) candidate_id: u32,
    /// Index of candidate’s last log entry
    pub(crate) last_log_index: usize,
    /// Term of candidate’s last log entry
    pub(crate) last_log_term: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct VoteResponse {
    /// Receiver’s current term
    pub(crate) term: u64,
    /// Whether the vote was granted
    pub(crate) vote_granted: bool,
}

impl VoteResponse {
    pub(crate) fn new(term: u64, vote_granted: bool) -> Self {
        Self { term, vote_granted }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct AppendResponse {
    term: u64,
    success: bool,
}

impl AppendResponse {
    pub(crate) fn new(term: u64, success: bool) -> Self {
        Self { term, success }
    }
}
