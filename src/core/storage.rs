use std::{
    io::{self, Write},
    path::Path,
};

use tempfile::NamedTempFile;

pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // writes / overwrites an entire file using a
    // tmp --> fsync --> rename --> fsync(dir) pattern
    // since renames are atomic we can't crash halfway through

    let dir = path.parent().unwrap();

    let mut tmp = NamedTempFile::new_in(dir)?;

    tmp.write_all(bytes)?;

    tmp.persist(path)?;

    Ok(())
}
