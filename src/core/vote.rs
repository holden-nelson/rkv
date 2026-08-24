use anyhow::{Ok, Result};

use crate::{
    core::{
        managers::{config::ConfigurationManager, lifecycle::NodeLifecycleManager, log::{LogManager}}, rpc::{RequestVote, RequestVoteResponse}, state::NodeState
    },
    tasks::rpc_server::server::{RpcServer, RpcServerCommand},
};

pub async fn become_candidate(
    cfg: &ConfigurationManager,
    node_mgr: &mut NodeLifecycleManager,
    log_mgr: &mut LogManager,
    rpc_server: &RpcServer,
) -> Result<()> {
    node_mgr.to_candidate()?;

    let current_term = node_mgr.get_current_term();
    let last_term = log_mgr.last_term()?;
    let last_index = log_mgr.last_index();

    let vote_request = RequestVote {
        candidate_id: cfg.id.to_string(),
        term: current_term,
        last_index,
        last_term,
    };

    let server_commands = cfg.peers.iter().map(|p| RpcServerCommand::RequestVote {
        peer: p.raft_addr,
        params: vote_request.clone(),
    });

    for cmd in server_commands {
        rpc_server.cmd_tx.send(cmd).await?;
    }

    Ok(())
}

pub fn handle_incoming_vote_request(
    node_mgr: &mut NodeLifecycleManager,
    v: RequestVote,
) -> Result<RequestVoteResponse> {
    let current_term = node_mgr.get_current_term();
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

    let (last_term, last_index) = state.get_last_logged_term_and_index()?;
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
