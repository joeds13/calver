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
            // Cargo requires semver, so strip the mandatory `.0` patch suffix before parsing.
            .map(|s| s.strip_suffix(".0").unwrap_or(s))
            .and_then(CalVer::parse);
        Ok(ver)
    }

    fn update_version(&self, version: &CalVer) -> Result<()> {
        let raw = std::fs::read_to_string(&self.path)?;
        let mut doc: toml_edit::DocumentMut = raw.parse().context("invalid Cargo.toml")?;
        // Cargo requires three-component semver; append `.0` as the patch version.
        doc["package"]["version"] = toml_edit::value(format!("{version}.0"));
        std::fs::write(&self.path, doc.to_string())?;
        Ok(())
    }
}
