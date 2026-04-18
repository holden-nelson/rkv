use std::fs::create_dir_all;

use crate::context::NodeContext;
use crate::core::api::{handle_delete, handle_put};
use crate::core::entries::send_heartbeats;
use crate::core::log::LogStore;
use crate::core::replication::try_replicate;
use crate::core::rpc::{AppendEntriesResponse, handle_append_entries};
use crate::core::state::NodeState;
use crate::core::vote::{become_candidate, handle_incoming_vote_request, handle_vote_received};
use crate::tasks::api_server::server::{ApiEvent, ApiServer};
use crate::tasks::heartbeat_timer::HeartbeatTimer;
use crate::tasks::rpc_server::server::RpcServer;
use crate::{core::events::Event, tasks::election_timer::ElectionTimer};

use anyhow::Result;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};
use tracing::{debug, info};

pub async fn run(ctx: NodeContext) -> Result<()> {
    let log_dir = ctx.persistence.base_dir.join("log");
    create_dir_all(&log_dir)?;

    let log_store_path = log_dir.join("entries.log");
    let log_store = LogStore::open(log_store_path)?;

    let mut state = NodeState::new(&ctx, log_store)?;

    let (event_tx, mut event_rx) = mpsc::channel::<Event>(128);

    let election_timeout = get_next_timeout_deadline(&state);
    let election_timer = ElectionTimer::spawn(event_tx.clone(), election_timeout);

    let bind_addr = ctx.raft_addr;
    let rpc_server = RpcServer::spawn(event_tx.clone(), bind_addr);

    let _api_server = ApiServer::spawn(event_tx.clone(), ctx.client_addr);

    let heartbeat_timer = HeartbeatTimer::spawn(event_tx.clone(), ctx.heartbeat_interval);

    let majority = ctx.cluster_size / 2 + 1;

    while let Some(event) = event_rx.recv().await {
        match event {
            Event::ElectionTimeoutFired => {
                info!(
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
                debug!(
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
                debug!(
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
                info!(
                    "[{}] election victorious with {} votes",
                    ctx.id,
                    state.get_vote_count().unwrap()
                );

                state.to_leader(&ctx)?;
                election_timer.stop().await;
                try_replicate(&ctx, &mut state, &rpc_server).await?;
                heartbeat_timer.start().await?;
            }
            Event::HeartbeatTimerFired => {
                debug!("[{}] heartbeat timer fired", ctx.id);

                try_replicate(&ctx, &mut state, &rpc_server).await?;
            }
            Event::AppendEntriesReceived { request, respond } => {
                let success = handle_append_entries(&ctx, &mut state, request)?;

                election_timer
                    .reset_deadline(get_next_timeout_deadline(&state))
                    .await?;

                let (_, last_index) = state.get_last_logged_term_and_index()?;

                let response = AppendEntriesResponse {
                    node_id: ctx.id.clone(),
                    term: state.get_current_term(),
                    last_index,
                    success,
                };
                let _ = respond.send(response);
            }
            Event::AppendEntriesResponse(r) => {
                debug!(
                    "[{}] append entries response received: success {}",
                    ctx.id, r.success
                );
            }
            Event::ClientRequestReceived(e) => {
                info!("[{}] api request received {:?}", ctx.id, e);

                match e {
                    ApiEvent::Put {
                        key,
                        value,
                        respond,
                    } => {
                        let _ = handle_put(&mut state, key, value)?;
                        try_replicate(&ctx, &mut state, &rpc_server).await?;
                        let _ = respond.send(Ok(())); // makes PUT return 204
                    }
                    ApiEvent::Delete { key, respond } => {
                        handle_delete(&mut state, key)?;
                        try_replicate(&ctx, &mut state, &rpc_server).await?;
                        let _ = respond.send(Ok(())); // makes DELETE return 204
                    }
                    ApiEvent::Get { respond, .. } => {
                        let _ = respond.send(Ok(None)); // makes GET return 404 in your handler
                    }
                }
            }
        }
    }

    election_timer.shutdown().await;

    Ok(())
}

fn get_next_timeout_deadline(state: &NodeState) -> Instant {
    Instant::now() + Duration::from_millis(state.get_election_timeout().into())
}
