use crate::context::NodeContext;
use crate::core::state::NodeState;
use crate::core::vote::{become_candidate, handle_incoming_vote_request, handle_vote_received};
use crate::tasks::rpc_server::server::RpcServer;
use crate::{core::events::Event, tasks::timer::ElectionTimer};

use anyhow::Result;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

pub async fn run(ctx: NodeContext) -> Result<()> {
    let mut state = NodeState::new(&ctx)?;

    let (event_tx, mut event_rx) = mpsc::channel::<Event>(128);

    let election_timeout = get_next_timeout_deadline(&state);
    let timer = ElectionTimer::spawn(event_tx.clone(), election_timeout);

    let bind_addr = ctx.raft_addr;
    let rpc_server = RpcServer::spawn(event_tx.clone(), bind_addr);

    let majority = ctx.cluster_size / 2 + 1;

    while let Some(event) = event_rx.recv().await {
        match event {
            Event::ElectionTimeoutFired => {
                println!(
                    "[{}] election timed out (term={})",
                    ctx.id,
                    state.get_current_term()
                );

                become_candidate(&ctx, &mut state, &rpc_server).await?;

                state.randomize_election_timeout();
                let _ = timer
                    .reset_deadline(get_next_timeout_deadline(&state))
                    .await;
            }
            Event::VoteReceived(v) => {
                println!(
                    "[{}] vote received: term={}, granted={}",
                    ctx.id, v.term, v.vote_granted
                );

                let num_votes = handle_vote_received(v, &mut state);

                if num_votes >= majority {
                    event_tx.send(Event::ElectionVictory).await?;
                }
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

                let response = handle_incoming_vote_request(request, &mut state)?;

                let _ = respond.send(response);
            }
            Event::ElectionVictory => {
                println!(
                    "[{}] election victorious with {} votes",
                    ctx.id,
                    state.get_vote_count()
                );

                state.to_leader()?;
            }
        }
    }

    timer.stop().await;

    Ok(())
}

fn get_next_timeout_deadline(state: &NodeState) -> Instant {
    Instant::now() + Duration::from_millis(state.get_election_timeout().into())
}
