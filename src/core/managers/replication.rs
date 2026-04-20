use std::{collections::HashMap, net::SocketAddr};

use anyhow::{Context, Result};

use crate::{
    core::{
        managers::{
            config::ConfigurationManager, lifecycle::NodeLifecycleManager, log::LogManager,
        },
        rpc::AppendEntries,
    },
    tasks::rpc_server::server::{RpcServer, RpcServerCommand},
};

struct ReplicationManager {
    node_id: String,
    replication_state: HashMap<String, FollowerReplicationState>,
}

struct FollowerReplicationState {
    raft_addr: SocketAddr,
    next_index: u64,
    match_index: u64,
    inflight: bool,
    needs_replicated_to: bool,
}

impl ReplicationManager {
    pub fn new(cfg: &ConfigurationManager, log_mgr: &LogManager) -> Self {
        let last_logged_index = log_mgr.last_index();

        let replication_state = cfg
            .peers
            .iter()
            .map(|peer| {
                (
                    peer.id.clone(),
                    FollowerReplicationState {
                        raft_addr: peer.raft_addr,
                        next_index: last_logged_index + 1,
                        match_index: 0,
                        inflight: false,
                        needs_replicated_to: true,
                    },
                )
            })
            .collect();

        ReplicationManager {
            node_id: cfg.id.clone(),
            replication_state,
        }
    }

    pub async fn try_replicate(
        &self,
        log_mgr: &LogManager,
        node_mgr: &NodeLifecycleManager,
        rpc_server: &RpcServer,
    ) -> Result<()> {
        for (node_id, follower_replication_state) in self.replication_state.iter_mut() {
            if follower_replication_state.inflight
                || !follower_replication_state.needs_replicated_to
            {
                continue;
            }

            follower_replication_state.needs_replicated_to = false;

            let last_index = log_mgr.last_index();

            let entries = {
                (follower_replication_state.next_index..=last_index)
                    .map(|idx| {
                        log_mgr
                            .get_entry_at_index(idx)?
                            .context(format!("missing log entry at index {idx}"))
                    })
                    .collect::<Result<Vec<_>>>()?
            };

            let last_term = log_mgr.last_term()?;

            let append_entries = AppendEntries {
                leader_id: self.node_id,
                term: node_mgr.get_current_term(),
                prev_log_term: last_term,
                prev_log_index: last_index,
                entries,
                leader_commit_index: node_mgr.commit_index,
            };

            let cmd = RpcServerCommand::AppendEntries {
                peer: follower_replication_state.raft_addr,
                params: append_entries,
            };

            rpc_server.cmd_tx.send(cmd).await?;

            follower_replication_state.inflight = true;
        }

        Ok(())
    }
}
