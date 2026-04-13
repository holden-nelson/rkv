use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use crc32fast;
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

const MAX_RECORD_LEN: u32 = 4 * 1024 * 1024;

pub struct LogStore {
    path: PathBuf,
    file: File,
    offsets: Vec<u64>,
}

impl LogStore {
    pub fn open(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();

        let (offsets, valid_len) = scan_log_file(&path)?;

        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)?;

        let actual_len = file.metadata()?.len();
        if actual_len != valid_len {
            file.set_len(valid_len)?;
        }

        file.seek(SeekFrom::End(0))?;

        Ok(Self {
            path,
            file,
            offsets,
        })
    }

    pub fn last_index(&self) -> u64 {
        self.offsets.len() as u64
    }

    pub fn append_entry(&mut self, entry: &LogEntry, durable: bool) -> io::Result<u64> {
        let payload =
            serde_json::to_vec(entry).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        self.append_payload(&payload, durable)?;
        Ok(self.last_index())
    }

    fn append_payload(&mut self, payload: &[u8], durable: bool) -> io::Result<()> {
        let len: u32 = payload.len().try_into().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "record length overflows u32")
        })?;

        if len > MAX_RECORD_LEN {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "record too large",
            ));
        }

        let start_offset = self.file.stream_position()?;

        let checksum = crc32fast::hash(payload);

        self.file.write_all(&len.to_be_bytes())?;
        self.file.write_all(payload)?;
        self.file.write_all(&checksum.to_be_bytes())?;

        if durable {
            self.file.sync_data()?;
        }

        self.offsets.push(start_offset);
        Ok(())
    }

    pub fn truncate_from(&mut self, index: u64) -> io::Result<()> {
        if index == 0 {
            self.file.set_len(0)?;
            self.offsets.clear();
            self.file.seek(SeekFrom::Start(0))?;
            return Ok(());
        }

        let idx0 = (index - 1) as usize;
        if idx0 >= self.offsets.len() {
            return Ok(());
        }

        let new_len = self.offsets[idx0];
        self.file.set_len(new_len)?;
        self.offsets.truncate(idx0);
        self.file.seek(SeekFrom::End(0))?;
        Ok(())
    }
}

fn scan_log_file(path: &Path) -> io::Result<(Vec<u64>, u64)> {
    let mut offsets = Vec::new();

    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((offsets, 0)),
        Err(e) => return Err(e),
    };

    let mut offset: u64 = 0;

    loop {
        let record_start = offset;

        let mut len_buf = [0u8; 4];
        if let Err(e) = file.read_exact(&mut len_buf) {
            return match e.kind() {
                io::ErrorKind::UnexpectedEof => Ok((offsets, record_start)),
                _ => Err(e),
            };
        }
        offset += 4;

        let len = u32::from_be_bytes(len_buf);
        if len > MAX_RECORD_LEN {
            return Ok((offsets, record_start));
        }

        let mut payload = vec![0u8; len as usize];
        if let Err(e) = file.read_exact(&mut payload) {
            return match e.kind() {
                io::ErrorKind::UnexpectedEof => Ok((offsets, record_start)),
                _ => Err(e),
            };
        }

        offset += len as u64;

        let mut checksum_buf = [0u8; 4];
        if let Err(e) = file.read_exact(&mut checksum_buf) {
            return match e.kind() {
                io::ErrorKind::UnexpectedEof => Ok((offsets, record_start)),
                _ => Err(e),
            };
        }
        offset += 4;

        let expected = u32::from_be_bytes(checksum_buf);
        let actual = crc32fast::hash(&payload);
        if expected != actual {
            return Ok((offsets, record_start));
        }

        if serde_json::from_slice::<LogEntry>(&payload).is_err() {
            return Ok((offsets, record_start));
        }

        offsets.push(record_start);
    }
}
