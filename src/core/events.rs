use crate::core::rpc::{RequestVote, RequestVoteResponse};

pub enum Event {
    ElectionTimeoutFired,
    VoteRequestReceived(RequestVote),
    VoteReceived(RequestVoteResponse),
}
