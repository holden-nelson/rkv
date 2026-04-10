use serde::{Deserialize, Serialize};

use crate::core::log::LogEntry;

#[derive(Serialize, Deserialize, Clone)]
pub struct RequestVote {
    pub candidate_id: String,
    pub term: u64,
    pub last_index: u64,
    pub last_term: u64,
}

#[derive(Serialize, Deserialize)]
pub struct RequestVoteResponse {
    pub term: u64,
    pub vote_granted: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppendEntries {
    pub leader_id: String,
    pub term: u64,
    pub prev_log_index: u64,
    pub prev_log_term: u64,
    pub entries: Vec<LogEntry>,
    pub leader_commit_index: u64,
}

#[derive(Serialize, Deserialize)]
pub struct AppendEntriesResponse {
    pub term: u64,
    pub success: bool,
}
