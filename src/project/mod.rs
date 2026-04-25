pub mod cargo;
pub mod go;
pub mod helm;
pub mod kustomization;
pub mod npm;
pub mod plain;
pub mod python;

use anyhow::Result;
use std::path::Path;

use crate::version::AnnoVer;

pub trait ProjectFile: std::fmt::Debug {
    fn path(&self) -> &Path;
    fn current_version(&self) -> Result<Option<AnnoVer>>;
    fn update_version(&self, version: &AnnoVer) -> Result<()>;
}

/// Detect all known project files in `dir` and return updaters for each.
pub fn detect_all(dir: &Path) -> Vec<Box<dyn ProjectFile>> {
    let mut files: Vec<Box<dyn ProjectFile>> = Vec::new();

    let candidates: Vec<Box<dyn ProjectFile>> = vec![
        Box::new(cargo::CargoFile::new(dir.join("Cargo.toml"))),
        Box::new(npm::NpmFile::new(dir.join("package.json"))),
        Box::new(python::PythonFile::new(dir.join("pyproject.toml"))),
        Box::new(helm::HelmFile::new(dir.join("Chart.yaml"))),
        Box::new(kustomization::KustomizationFile::new(
            dir.join("kustomization.yaml"),
        )),
        Box::new(go::GoFile::new(dir.join("main.go"))),
        Box::new(go::GoFile::new(dir.join("cmd/main.go"))),
        Box::new(plain::PlainFile::new(dir.join(".annover"))),
        Box::new(plain::PlainFile::new(dir.join("VERSION"))),
    ];

    for f in candidates {
        if f.path().exists() {
            files.push(f);
        }
    }

    files
}
