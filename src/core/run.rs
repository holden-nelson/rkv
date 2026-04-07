use crate::context::NodeContext;
use crate::core::rpc::{RequestVote, RequestVoteResponse};
use crate::core::state::NodeState;
use crate::tasks::rpc_server::server::{RpcServer, RpcServerCommand};
use crate::{core::events::Event, tasks::timer::ElectionTimer};

use anyhow::Result;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

pub async fn run(ctx: NodeContext, state: NodeState) -> Result<()> {
    let (event_tx, mut event_rx) = mpsc::channel::<Event>(128);

    let election_timeout = get_next_timeout_deadline(&ctx);
    let timer = ElectionTimer::spawn(event_tx.clone(), election_timeout);

    let bind_addr = ctx.raft_addr;
    let rpc_server = RpcServer::spawn(event_tx.clone(), bind_addr);

    while let Some(event) = event_rx.recv().await {
        match event {
            Event::ElectionTimeoutFired => {
                println!(
                    "[{}] election timed out (term={})",
                    ctx.id, state.persistent_state.current_term
                );

                let vote_request = RequestVote {
                    candidate_id: ctx.id.to_string(),
                    term: state.persistent_state.current_term,
                    last_index: state.volatile_state.last_logged_index,
                    last_term: state.volatile_state.last_logged_term,
                };

                let server_commands = ctx.peers.iter().map(|p| RpcServerCommand::RequestVote {
                    peer: p.raft_addr,
                    params: vote_request.clone(),
                });

                for cmd in server_commands {
                    rpc_server.cmd_tx.send(cmd).await?;
                }

                let _ = timer.reset_deadline(get_next_timeout_deadline(&ctx)).await;
            }
            Event::VoteReceived(v) => {
                println!(
                    "[{}] vote received: term={}, granted={}",
                    ctx.id, v.term, v.vote_granted
                );
            }
            Event::VoteRequestReceived { request, respond } => {
                println!(
                    "[{}] vote request received from {} (term={}, last_index={}, last_term={})",
                    ctx.id,
                    request.candidate_id,
                    request.term,
                    request.last_index,
                    request.last_term
                );

                let response = RequestVoteResponse {
                    term: 0,
                    vote_granted: true,
                };

                let _ = respond.send(response);
            }
        }
    }

    timer.stop().await;

    Ok(())
}

fn get_next_timeout_deadline(ctx: &NodeContext) -> Instant {
    Instant::now() + Duration::from_millis(ctx.election_timeout_ms.into())
}
