pub mod cargo;
pub mod helm;
pub mod npm;
pub mod plain;
pub mod python;

use anyhow::Result;
use std::path::Path;

use crate::version::CalVer;

pub trait ProjectFile: std::fmt::Debug {
    fn path(&self) -> &Path;
    fn current_version(&self) -> Result<Option<CalVer>>;
    fn update_version(&self, version: &CalVer) -> Result<()>;
}

/// Detect all known project files in `dir` and return updaters for each.
pub fn detect_all(dir: &Path) -> Vec<Box<dyn ProjectFile>> {
    let mut files: Vec<Box<dyn ProjectFile>> = Vec::new();

    let candidates: Vec<Box<dyn ProjectFile>> = vec![
        Box::new(cargo::CargoFile::new(dir.join("Cargo.toml"))),
        Box::new(npm::NpmFile::new(dir.join("package.json"))),
        Box::new(python::PythonFile::new(dir.join("pyproject.toml"))),
        Box::new(helm::HelmFile::new(dir.join("Chart.yaml"))),
        Box::new(plain::PlainFile::new(dir.join(".calver"))),
        Box::new(plain::PlainFile::new(dir.join("VERSION"))),
    ];

    for f in candidates {
        if f.path().exists() {
            files.push(f);
        }
    }

    files
}
