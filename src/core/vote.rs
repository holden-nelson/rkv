use anyhow::{Ok, Result};

use crate::{
    context::NodeContext,
    core::{
        rpc::{RequestVote, RequestVoteResponse},
        state::NodeState,
    },
    tasks::rpc_server::server::{RpcServer, RpcServerCommand},
};

pub async fn become_candidate(
    ctx: &NodeContext,
    state: &mut NodeState,
    rpc_server: &RpcServer,
) -> Result<()> {
    state.to_candidate();
    state.increment_term()?;
    state.vote_for(&ctx.id)?;
    state.record_vote();

    let current_term = state.get_current_term();
    let (last_term, last_index) = state.get_last_logged_term_and_index();

    let vote_request = RequestVote {
        candidate_id: ctx.id.to_string(),
        term: current_term,
        last_index,
        last_term,
    };

    let server_commands = ctx.peers.iter().map(|p| RpcServerCommand::RequestVote {
        peer: p.raft_addr,
        params: vote_request.clone(),
    });

    for cmd in server_commands {
        rpc_server.cmd_tx.send(cmd).await?;
    }

    Ok(())
}

pub fn handle_incoming_vote_request(
    v: RequestVote,
    state: &mut NodeState,
) -> Result<RequestVoteResponse> {
    let current_term = state.get_current_term();
    let deny_response = RequestVoteResponse {
        term: current_term,
        vote_granted: false,
    };

    if v.term < current_term {
        return Ok(deny_response);
    }

    if state.has_voted() {
        return Ok(deny_response);
    }

    let (last_term, last_index) = state.get_last_logged_term_and_index();
    if v.last_term < last_term {
        return Ok(deny_response);
    }

    if v.last_index < last_index {
        return Ok(deny_response);
    }

    state.vote_for(&v.candidate_id)?;

    Ok(RequestVoteResponse {
        term: state.get_current_term(),
        vote_granted: true,
    })
}

pub fn handle_vote_received(r: RequestVoteResponse, state: &mut NodeState) -> Option<u32> {
    let mut votes_received = state.get_vote_count();

    if r.vote_granted {
        votes_received = state.record_vote();
    };

    votes_received
}
