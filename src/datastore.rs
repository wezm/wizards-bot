use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::bushfire::EntryId;

pub struct Datastore {
    path: PathBuf,
    records: Records,
}

pub type Records = HashSet<EntryId>;

impl Datastore {
    pub fn new<P: Into<PathBuf>>(path: P) -> Result<Self, io::Error> {
        let path = path.into();
        match Self::load(&path) {
            Ok(records) => Ok(Datastore { path, records }),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Datastore {
                path,
                records: HashSet::new(),
            }),
            Err(err) => Err(err),
        }
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Records, io::Error> {
        let path = path.as_ref();
        // Read the existing records
        let file = BufReader::new(File::open(path)?);
        let mut records = HashSet::new();
        for line in file.lines() {
            let line = line?;
            if !line.is_empty() {
                records.insert(EntryId(line));
            }
        }
        Ok(records)
    }

    pub fn append(&mut self, record: EntryId) -> Result<(), io::Error> {
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)?;
        writeln!(file, "{}", record.0)?;
        self.records.insert(record);
        Ok(())
    }

    pub fn contains(&self, entry: &EntryId) -> bool {
        self.records.contains(entry)
    }
}
