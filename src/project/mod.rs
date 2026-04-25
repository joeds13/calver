pub mod cargo;
pub mod go;
pub mod helm;
pub mod kustomization;
pub mod npm;
pub mod plain;
pub mod python;

use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::version::AnnoVer;

pub trait ProjectFile: std::fmt::Debug {
    fn path(&self) -> &Path;
    fn current_version(&self) -> Result<Option<AnnoVer>>;
    fn update_version(&self, version: &AnnoVer) -> Result<()>;
}

/// Detect all known project files in `dir` and return updaters for each.
pub fn detect_all(dir: &Path) -> Vec<Box<dyn ProjectFile>> {
    let mut files: Vec<Box<dyn ProjectFile>> = Vec::new();

    // Files always at project root by convention
    let fixed: Vec<Box<dyn ProjectFile>> = vec![
        Box::new(cargo::CargoFile::new(dir.join("Cargo.toml"))),
        Box::new(npm::NpmFile::new(dir.join("package.json"))),
        Box::new(python::PythonFile::new(dir.join("pyproject.toml"))),
        Box::new(helm::HelmFile::new(dir.join("Chart.yaml"))),
        Box::new(plain::PlainFile::new(dir.join("VERSION"))),
    ];
    for f in fixed {
        if f.path().exists() {
            files.push(f);
        }
    }

    // Files that may appear at any depth in the tree
    for path in walk_files(dir) {
        match path.file_name().and_then(|n| n.to_str()) {
            Some("kustomization.yaml")
                if std::fs::read_to_string(&path)
                    .map(|c| c.contains("newTag:"))
                    .unwrap_or(false) =>
            {
                files.push(Box::new(kustomization::KustomizationFile::new(path)));
            }
            _ if path.extension().and_then(|e| e.to_str()) == Some("go")
                && go::is_version_file(&path) =>
            {
                files.push(Box::new(go::GoFile::new(path)));
            }
            _ => {}
        }
    }

    files
}

fn walk_files(dir: &Path) -> impl Iterator<Item = PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    std::iter::from_fn(move || {
        while let Some(path) = stack.pop() {
            if path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.starts_with('.'))
                            .unwrap_or(false)
                        {
                            continue;
                        }
                        stack.push(p);
                    }
                }
            } else if path.is_file() {
                return Some(path);
            }
        }
        None
    })
}
