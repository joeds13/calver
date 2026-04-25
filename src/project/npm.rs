use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::version::CalVer;

use super::ProjectFile;

#[derive(Debug)]
pub struct NpmFile {
    path: PathBuf,
}

impl NpmFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ProjectFile for NpmFile {
    fn path(&self) -> &Path {
        &self.path
    }

    fn current_version(&self) -> Result<Option<CalVer>> {
        let raw = std::fs::read_to_string(&self.path)?;
        let json: serde_json::Value = serde_json::from_str(&raw).context("invalid package.json")?;
        let ver = json["version"].as_str().and_then(CalVer::parse);
        Ok(ver)
    }

    fn update_version(&self, version: &CalVer) -> Result<()> {
        let raw = std::fs::read_to_string(&self.path)?;
        let mut json: serde_json::Value =
            serde_json::from_str(&raw).context("invalid package.json")?;
        json["version"] = serde_json::Value::String(version.to_string());
        // Preserve trailing newline; serde_json pretty-print uses 2-space indent.
        let mut out = serde_json::to_string_pretty(&json)?;
        out.push('\n');
        std::fs::write(&self.path, out)?;
        Ok(())
    }
}
