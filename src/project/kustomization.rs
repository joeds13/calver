use anyhow::Result;
use regex::Regex;
use std::path::{Path, PathBuf};

use crate::version::AnnoVer;

use super::ProjectFile;

#[derive(Debug)]
pub struct KustomizationFile {
    path: PathBuf,
}

impl KustomizationFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ProjectFile for KustomizationFile {
    fn path(&self) -> &Path {
        &self.path
    }

    fn current_version(&self) -> Result<Option<AnnoVer>> {
        let raw = std::fs::read_to_string(&self.path)?;
        let ver = raw.lines().find_map(parse_new_tag_line);
        Ok(ver)
    }

    fn update_version(&self, version: &AnnoVer) -> Result<()> {
        let raw = std::fs::read_to_string(&self.path)?;
        // Captures: prefix, optional open-quote, version value, optional close-quote
        let re = Regex::new(r#"^(\s*newTag:\s*)(["']?)([^\s"']+)(["']?)\s*$"#)?;
        let new_ver = version.to_string();
        let updated: String = raw
            .lines()
            .map(|line| {
                if let Some(caps) = re.captures(line) {
                    format!("{}{}{}{}", &caps[1], &caps[2], new_ver, &caps[4])
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let updated = if raw.ends_with('\n') {
            format!("{updated}\n")
        } else {
            updated
        };

        std::fs::write(&self.path, updated)?;
        Ok(())
    }
}

fn parse_new_tag_line(line: &str) -> Option<AnnoVer> {
    let value = line.trim().strip_prefix("newTag:")?.trim();
    let value = value.trim_matches('"').trim_matches('\'');
    AnnoVer::parse(value)
}
