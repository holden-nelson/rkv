use serde::{Deserialize, Serialize};

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
