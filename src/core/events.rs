use tokio::sync::oneshot::Sender;

use crate::core::rpc::{RequestVote, RequestVoteResponse};

pub enum Event {
    ElectionTimeoutFired,
    VoteRequestReceived {
        request: RequestVote,
        respond: Sender<RequestVoteResponse>,
    },
    VoteReceived(RequestVoteResponse),
}
