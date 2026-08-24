use anyhow::Result;

use crate::core::managers::{lifecycle::NodeLifecycleManager, log::{CommandKind, EntryKind, LogEntry, LogManager}};

pub fn handle_put(node_mgr: &NodeLifecycleManager, log_mgr: &mut LogManager, key: String, value: Vec<u8>) -> Result<LogEntry> {
    let entry = LogEntry {
        term: node_mgr.get_current_term(),
        index: log_mgr.last_index() + 1,
        kind: EntryKind::Command(CommandKind::Put { key, value }),
    };

    log_mgr.append_entry(&entry, true)?;

    Ok(entry)
}

pub fn handle_delete(node_mgr: &NodeLifecycleManager, log_mgr: &mut LogManager, key: String) -> Result<LogEntry> {
    let entry = LogEntry {
        term: node_mgr.get_current_term(),
        index: log_mgr.last_index() + 1,
        kind: EntryKind::Command(CommandKind::Delete { key }),
    };

    log_mgr.append_entry(&entry, true)?;

    Ok(entry)
}
