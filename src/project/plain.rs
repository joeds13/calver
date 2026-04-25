use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::version::AnnoVer;

use super::ProjectFile;

/// Plain text file containing only a version string (e.g. `.annover` or `VERSION`).
#[derive(Debug)]
pub struct PlainFile {
    path: PathBuf,
}

impl PlainFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ProjectFile for PlainFile {
    fn path(&self) -> &Path {
        &self.path
    }

    fn current_version(&self) -> Result<Option<AnnoVer>> {
        let raw = std::fs::read_to_string(&self.path)?;
        Ok(AnnoVer::parse(raw.trim()))
    }

    fn update_version(&self, version: &AnnoVer) -> Result<()> {
        std::fs::write(&self.path, format!("{version}\n"))?;
        Ok(())
    }
}
