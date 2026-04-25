use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::version::CalVer;

use super::ProjectFile;

/// Plain text file containing only a version string (e.g. `.calver` or `VERSION`).
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

    fn current_version(&self) -> Result<Option<CalVer>> {
        let raw = std::fs::read_to_string(&self.path)?;
        Ok(CalVer::parse(raw.trim()))
    }

    fn update_version(&self, version: &CalVer) -> Result<()> {
        std::fs::write(&self.path, format!("{version}\n"))?;
        Ok(())
    }
}
