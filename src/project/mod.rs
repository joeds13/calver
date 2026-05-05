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

/// Detect all known project files under `dir` and return updaters for each.
///
/// Walks up to 3 levels deep, respects `.gitignore` files at each level, skips
/// hidden directories, and hard-blocks known dependency trees (node_modules,
/// target, vendor) as a safety net for projects that don't gitignore them.
pub fn detect_all(dir: &Path) -> Vec<Box<dyn ProjectFile>> {
    let mut files: Vec<Box<dyn ProjectFile>> = Vec::new();

    let walker = ignore::WalkBuilder::new(dir)
        .max_depth(Some(3))
        .hidden(true)
        .filter_entry(|e| {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = e.file_name().to_str().unwrap_or("");
                !matches!(name, "node_modules" | "target" | "vendor")
            } else {
                true
            }
        })
        .build();

    for result in walker {
        let entry = match result {
            Ok(e) if e.file_type().map(|t| t.is_file()).unwrap_or(false) => e,
            _ => continue,
        };
        let path = entry.into_path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        match name {
            "Cargo.toml" => files.push(Box::new(cargo::CargoFile::new(path))),
            "package.json" => files.push(Box::new(npm::NpmFile::new(path))),
            "pyproject.toml" => files.push(Box::new(python::PythonFile::new(path))),
            "Chart.yaml" => files.push(Box::new(helm::HelmFile::new(path))),
            "VERSION" => files.push(Box::new(plain::PlainFile::new(path))),
            "kustomization.yaml" => {
                if std::fs::read_to_string(&path)
                    .map(|c| c.contains("newTag:"))
                    .unwrap_or(false)
                {
                    files.push(Box::new(kustomization::KustomizationFile::new(path)));
                }
            }
            _ => {
                if path.extension().and_then(|e| e.to_str()) == Some("go")
                    && go::is_version_file(&path)
                {
                    files.push(Box::new(go::GoFile::new(path)));
                }
            }
        }
    }

    files
}
