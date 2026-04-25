use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::version::CalVer;

use super::ProjectFile;

#[derive(Debug)]
pub struct PythonFile {
    path: PathBuf,
}

impl PythonFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ProjectFile for PythonFile {
    fn path(&self) -> &Path {
        &self.path
    }

    fn current_version(&self) -> Result<Option<CalVer>> {
        let raw = std::fs::read_to_string(&self.path)?;
        let doc: toml_edit::DocumentMut = raw.parse().context("invalid pyproject.toml")?;
        // PEP 621 [project.version] takes priority, then [tool.poetry.version]
        let ver = doc
            .get("project")
            .and_then(|p| p.get("version"))
            .or_else(|| {
                doc.get("tool")
                    .and_then(|t| t.get("poetry"))
                    .and_then(|p| p.get("version"))
            })
            .and_then(|v| v.as_str())
            .and_then(CalVer::parse);
        Ok(ver)
    }

    fn update_version(&self, version: &CalVer) -> Result<()> {
        let raw = std::fs::read_to_string(&self.path)?;
        let mut doc: toml_edit::DocumentMut = raw.parse().context("invalid pyproject.toml")?;
        let ver_str = version.to_string();

        if doc
            .get("project")
            .and_then(|p| p.get("version"))
            .is_some()
        {
            doc["project"]["version"] = toml_edit::value(ver_str);
        } else if doc
            .get("tool")
            .and_then(|t| t.get("poetry"))
            .and_then(|p| p.get("version"))
            .is_some()
        {
            doc["tool"]["poetry"]["version"] = toml_edit::value(version.to_string());
        }

        std::fs::write(&self.path, doc.to_string())?;
        Ok(())
    }
}
