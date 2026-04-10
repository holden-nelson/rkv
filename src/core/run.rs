use crate::context::NodeContext;
use crate::core::entries::send_heartbeats;
use crate::core::rpc::AppendEntriesResponse;
use crate::core::state::NodeState;
use crate::core::vote::{become_candidate, handle_incoming_vote_request, handle_vote_received};
use crate::tasks::heartbeat_timer::HeartbeatTimer;
use crate::tasks::rpc_server::server::RpcServer;
use crate::{core::events::Event, tasks::election_timer::ElectionTimer};

use anyhow::Result;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

pub async fn run(ctx: NodeContext) -> Result<()> {
    let mut state = NodeState::new(&ctx)?;

    let (event_tx, mut event_rx) = mpsc::channel::<Event>(128);

    let election_timeout = get_next_timeout_deadline(&state);
    let election_timer = ElectionTimer::spawn(event_tx.clone(), election_timeout);

    let bind_addr = ctx.raft_addr;
    let rpc_server = RpcServer::spawn(event_tx.clone(), bind_addr);

    let heartbeat_timer = HeartbeatTimer::spawn(event_tx.clone(), ctx.heartbeat_interval);

    let majority = ctx.cluster_size / 2 + 1;

    while let Some(event) = event_rx.recv().await {
        match event {
            Event::ElectionTimeoutFired => {
                println!(
                    "[{}] election timed out (term={})",
                    ctx.id.to_string(),
                    state.get_current_term()
                );

                become_candidate(&ctx, &mut state, &rpc_server).await?;

                state.randomize_election_timeout();
                let _ = election_timer
                    .reset_deadline(get_next_timeout_deadline(&state))
                    .await;
            }
            Event::VoteReceived(v) => {
                println!(
                    "[{}] vote received: term={}, granted={}",
                    ctx.id, v.term, v.vote_granted
                );

                if let Some(num_votes) = handle_vote_received(v, &mut state) {
                    if num_votes >= majority {
                        event_tx.send(Event::ElectionVictory).await?;
                    }
                };
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

                let _ = election_timer
                    .reset_deadline(get_next_timeout_deadline(&state))
                    .await;

                let response = handle_incoming_vote_request(request, &mut state)?;

                let _ = respond.send(response);
            }
            Event::ElectionVictory => {
                println!(
                    "[{}] election victorious with {} votes",
                    ctx.id,
                    state.get_vote_count().unwrap()
                );

                state.to_leader()?;
                election_timer.stop().await;
                send_heartbeats(&ctx, &state, &rpc_server).await?;
                heartbeat_timer.start().await?;
            }
            Event::HeartbeatTimerFired => {
                println!("[{}] heartbeat timer fired", ctx.id);

                send_heartbeats(&ctx, &state, &rpc_server).await?;
            }
            Event::AppendEntriesReceived { request, respond } => {
                println!(
                    "[{}] append entry received from {}",
                    ctx.id, request.leader_id
                );

                if request.entries.is_empty() {
                    println!("[{}] entry was a heartbeat", ctx.id);
                }

                election_timer
                    .reset_deadline(get_next_timeout_deadline(&state))
                    .await?;

                let response = AppendEntriesResponse {
                    term: state.get_current_term(),
                    success: true,
                };
                let _ = respond.send(response);
            }
            Event::AppendEntriesResponse(r) => {
                println!(
                    "[{}] append entries response received: success {}",
                    ctx.id, r.success
                );
            }
        }
    }

    election_timer.shutdown().await;

    Ok(())
}

fn get_next_timeout_deadline(state: &NodeState) -> Instant {
    Instant::now() + Duration::from_millis(state.get_election_timeout().into())
}
