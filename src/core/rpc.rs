use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::core::managers::{config::ConfigurationManager, lifecycle::NodeLifecycleManager, log::{LogEntry, LogManager}};

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
    cfg: &ConfigurationManager,
    node_mgr: &mut NodeLifecycleManager,
    log_mgr: &mut LogManager,
    request: AppendEntries,
) -> Result<bool> {
    if request.entries.is_empty() {
        debug!("[{}] heartbeat from {}", cfg.id, request.leader_id);
        return Ok(true);
    } else {
        debug!(
            "[{}] append entry received from {}",
            cfg.id, request.leader_id
        );

        if request.term < node_mgr.get_current_term() {
            return Ok(false);
        }

        if request.prev_log_index != 0 {
            let Some(prev_entry) = log_mgr.get_entry_at_index(request.prev_log_index)?
            else {
                return Ok(false);
            };

            if prev_entry.term != request.prev_log_term {
                return Ok(false);
            }
        }

        for (i, entry) in request.entries.iter().enumerate() {
            match log_mgr.get_entry_at_index(entry.index)? {
                Some(logged_entry) => {
                    if logged_entry.term == entry.term {
                        continue;
                    }

                    log_mgr.append_entries_from(
                        entry.index,
                        &request.entries[i..],
                        true,
                    )?;
                    break;
                }
                None => {
                    log_mgr
                        .append_entries(&request.entries[i..], true)?;
                    break;
                }
            }
        }
    }

    if request.leader_commit_index > node_mgr.commit_index {
        node_mgr.commit_index = request
                .leader_commit_index
                .min(log_mgr.last_index(),
        );
    }

    Ok(true)
}
