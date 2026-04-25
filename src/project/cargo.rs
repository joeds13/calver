use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::version::CalVer;

use super::ProjectFile;

#[derive(Debug)]
pub struct CargoFile {
    path: PathBuf,
}

impl CargoFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ProjectFile for CargoFile {
    fn path(&self) -> &Path {
        &self.path
    }

    fn current_version(&self) -> Result<Option<CalVer>> {
        let raw = std::fs::read_to_string(&self.path)?;
        let doc: toml_edit::DocumentMut = raw.parse().context("invalid Cargo.toml")?;
        let ver = doc
            .get("package")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .and_then(CalVer::parse);
        Ok(ver)
    }

    fn update_version(&self, version: &CalVer) -> Result<()> {
        let raw = std::fs::read_to_string(&self.path)?;
        let mut doc: toml_edit::DocumentMut = raw.parse().context("invalid Cargo.toml")?;
        doc["package"]["version"] = toml_edit::value(version.to_string());
        std::fs::write(&self.path, doc.to_string())?;
        Ok(())
    }
}
