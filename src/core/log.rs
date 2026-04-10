use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct LogEntry {
    term: u64,
    kind: EntryKind,
}

#[derive(Serialize, Deserialize, Clone)]
pub enum EntryKind {
    Command(CommandKind),
}

#[derive(Serialize, Deserialize, Clone)]
pub enum CommandKind {
    Put { key: String, value: Vec<u8> },
    Delete { key: String },
}
