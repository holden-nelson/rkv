use anyhow::Result;

use crate::{
    context::NodeContext,
    core::{rpc::AppendEntries, state::NodeState},
    tasks::rpc_server::server::{RpcServer, RpcServerCommand},
};

pub async fn send_heartbeats(
    ctx: &NodeContext,
    state: &NodeState,
    rpc_server: &RpcServer,
) -> Result<()> {
    let (last_logged_term, last_logged_index) = state.get_last_logged_term_and_index();

    let heartbeat_payload = AppendEntries {
        leader_id: ctx.id.to_string(),
        term: state.get_current_term(),
        prev_log_index: last_logged_index,
        prev_log_term: last_logged_term,
        entries: vec![],
        leader_commit_index: state.get_commit_index(),
    };

    let server_commands = ctx.peers.iter().map(|p| RpcServerCommand::AppendEntries {
        peer: p.raft_addr,
        params: heartbeat_payload.clone(),
    });

    for cmd in server_commands {
        rpc_server.cmd_tx.send(cmd).await?;
    }

    Ok(())
}
