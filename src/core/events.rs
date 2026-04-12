use tokio::sync::oneshot::Sender;

use crate::{
    core::rpc::{AppendEntries, AppendEntriesResponse, RequestVote, RequestVoteResponse},
    tasks::api_server::server::ApiEvent,
};

pub enum Event {
    ElectionTimeoutFired,
    VoteRequestReceived {
        request: RequestVote,
        respond: Sender<RequestVoteResponse>,
    },
    VoteReceived(RequestVoteResponse),
    ElectionVictory,
    HeartbeatTimerFired,
    AppendEntriesReceived {
        request: AppendEntries,
        respond: Sender<AppendEntriesResponse>,
    },
    AppendEntriesResponse(AppendEntriesResponse),
    ClientRequestReceived(ApiEvent),
}
