use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct LogEntry {
    pub term: u64,
    pub index: u64,
    pub kind: EntryKind,
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

    pub fn last_term(&mut self) -> io::Result<u64> {
        let last_index = self.last_index();
        if last_index == 0 {
            return Ok(0);
        }

        match self.get_entry_at_index(last_index)? {
            Some(entry) => Ok(entry.term),
            None => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "log offsets indicated a last entry, but it could not be read/validated",
            )),
        }
    }

    pub fn get_entry_at_index(&mut self, index: u64) -> io::Result<Option<LogEntry>> {
        if index == 0 || index > self.last_index() {
            return Ok(None);
        }

        let vec_idx = (index - 1) as usize;
        let offset = self.offsets[vec_idx];
        self.file.seek(SeekFrom::Start(offset))?;

        let mut len_buf = [0u8; 4];
        if let Err(e) = self.file.read_exact(&mut len_buf) {
            return match e.kind() {
                io::ErrorKind::UnexpectedEof => Ok(None),
                _ => Err(e),
            };
        }

        let len = u32::from_be_bytes(len_buf);
        if len > MAX_RECORD_LEN {
            return Ok(None);
        }

        let mut entry_bytes = vec![0u8; len as usize];
        if let Err(e) = self.file.read_exact(&mut entry_bytes) {
            return match e.kind() {
                io::ErrorKind::UnexpectedEof => Ok(None),
                _ => Err(e),
            };
        }

        let mut checksum_buf = [0u8; 4];
        if let Err(e) = self.file.read_exact(&mut checksum_buf) {
            return match e.kind() {
                io::ErrorKind::UnexpectedEof => Ok(None),
                _ => Err(e),
            };
        }

        let expected = u32::from_be_bytes(checksum_buf);
        let actual = crc32fast::hash(&entry_bytes);
        if expected != actual {
            return Ok(None);
        }

        let entry = serde_json::from_slice::<LogEntry>(&entry_bytes);

        match entry {
            Err(_) => Ok(None),
            Ok(e) => Ok(Some(e)),
        }
    }

    pub fn append_entry(&mut self, entry: &LogEntry, durable: bool) -> io::Result<u64> {
        let payload =
            serde_json::to_vec(entry).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        self.append_payload(&payload, durable)?;
        Ok(self.last_index())
    }

    pub fn append_entry_from(
        &mut self,
        index: u64,
        entry: &LogEntry,
        durable: bool,
    ) -> io::Result<u64> {
        if index == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "log index is 1-based; 0 is not a valid entry index",
            ));
        }

        let last = self.last_index();

        if index <= last {
            self.truncate_from(index)?;
        } else if index != last + 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot append beyond last_index + 1 (would create a gap)",
            ));
        }

        self.append_entry(entry, durable)
    }

    pub fn append_entries(&mut self, entries: &[LogEntry], durable: bool) -> io::Result<u64> {
        for entry in entries {
            self.append_entry(entry, durable)?;
        }

        Ok(self.last_index())
    }

    pub fn append_entries_from(
        &mut self,
        index: u64,
        entries: &[LogEntry],
        durable: bool,
    ) -> io::Result<u64> {
        if index == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "log index is 1-based; 0 is not a valid entry index",
            ));
        }

        let last = self.last_index();

        if index <= last {
            self.truncate_from(index)?;
        } else if index != last + 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot append beyond last_index + 1 (would create a gap)",
            ));
        }

        self.append_entries(entries, durable)?;
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

        self.file.seek(SeekFrom::End(0))?;
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
