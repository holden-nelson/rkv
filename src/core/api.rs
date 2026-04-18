use anyhow::Result;

use crate::core::{log::LogEntry, state::NodeState};

use super::log::{CommandKind, EntryKind};

pub fn handle_put(state: &mut NodeState, key: String, value: Vec<u8>) -> Result<LogEntry> {
    let entry = LogEntry {
        term: state.get_current_term(),
        index: state.log_store.last_index() + 1,
        kind: EntryKind::Command(CommandKind::Put { key, value }),
    };

    state.log_store.append_entry(&entry, true)?;

    Ok(entry)
}

pub fn handle_delete(state: &mut NodeState, key: String) -> Result<LogEntry> {
    let entry = LogEntry {
        term: state.get_current_term(),
        index: state.log_store.last_index() + 1,
        kind: EntryKind::Command(CommandKind::Delete { key }),
    };

    state.log_store.append_entry(&entry, true)?;

    Ok(entry)
}
