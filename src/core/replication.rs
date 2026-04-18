use std::collections::HashMap;

use anyhow::{Context, Result};

use crate::{
    context::NodeContext,
    core::{rpc::AppendEntries, state::NodeState},
    tasks::rpc_server::server::{RpcServer, RpcServerCommand},
};

pub struct FollowerReplicationState {
    pub next_index: u64,
    pub match_index: u64,
    pub inflight: bool,
    pub needs_replicated_to: bool,
}

pub async fn try_replicate(
    ctx: &NodeContext,
    state: &mut NodeState,
    rpc_server: &RpcServer,
) -> Result<()> {
    for follower in &ctx.peers {
        let follower_state = state
            .get_replication_state()
            .context(format!("no replication state - called on follower?"))?
            .get(&follower.id)
            .context(format!("missing follower state for {}", follower.id))?;

        if follower_state.inflight || !follower_state.needs_replicated_to {
            continue;
        }

        let last_index = state.log_store.last_index();

        let entries = {
            (follower_state.next_index..=last_index)
                .map(|idx| {
                    state
                        .log_store
                        .get_entry_at_index(idx)?
                        .context(format!("missing log entry at index {idx}"))
                })
                .collect::<Result<Vec<_>>>()?
        };

        let (prev_log_term, prev_log_index) = state.get_last_logged_term_and_index()?;

        let append_entries = AppendEntries {
            leader_id: ctx.id.clone(),
            term: state.get_current_term(),
            prev_log_term,
            prev_log_index,
            entries,
            leader_commit_index: state.get_commit_index(),
        };

        let cmd = RpcServerCommand::AppendEntries {
            peer: follower.raft_addr,
            params: append_entries,
        };

        rpc_server.cmd_tx.send(cmd).await?;
    }

    Ok(())
}
