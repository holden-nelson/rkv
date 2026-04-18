use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{
    context::NodeContext,
    core::{log::LogEntry, state::NodeState},
};

#[derive(Serialize, Deserialize, Clone)]
pub struct RequestVote {
    pub candidate_id: String,
    pub term: u64,
    pub last_index: u64,
    pub last_term: u64,
}

#[derive(Serialize, Deserialize)]
pub struct RequestVoteResponse {
    pub term: u64,
    pub vote_granted: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AppendEntries {
    pub leader_id: String,
    pub term: u64,
    pub prev_log_index: u64,
    pub prev_log_term: u64,
    pub entries: Vec<LogEntry>,
    pub leader_commit_index: u64,
}

#[derive(Serialize, Deserialize)]
pub struct AppendEntriesResponse {
    pub node_id: String,
    pub term: u64,
    pub last_index: u64,
    pub success: bool,
}

pub fn handle_append_entries(
    ctx: &NodeContext,
    state: &mut NodeState,
    request: AppendEntries,
) -> Result<bool> {
    if request.entries.is_empty() {
        debug!("[{}] heartbeat from {}", ctx.id, request.leader_id);
        return Ok(true);
    } else {
        debug!(
            "[{}] append entry received from {}",
            ctx.id, request.leader_id
        );

        if request.term < state.get_current_term() {
            return Ok(false);
        }

        if request.prev_log_index != 0 {
            let Some(prev_entry) = state.log_store.get_entry_at_index(request.prev_log_index)?
            else {
                return Ok(false);
            };

            if prev_entry.term != request.prev_log_term {
                return Ok(false);
            }
        }

        for (i, entry) in request.entries.iter().enumerate() {
            match state.log_store.get_entry_at_index(entry.index)? {
                Some(logged_entry) => {
                    if logged_entry.term == entry.term {
                        continue;
                    }

                    state.log_store.append_entries_from(
                        entry.index,
                        &request.entries[i..],
                        true,
                    )?;
                    break;
                }
                None => {
                    state
                        .log_store
                        .append_entries(&request.entries[i..], true)?;
                    break;
                }
            }
        }
    }

    if request.leader_commit_index > state.get_commit_index() {
        state.set_commit_index(
            request
                .leader_commit_index
                .min(state.log_store.last_index()),
        );
    }

    Ok(true)
}
