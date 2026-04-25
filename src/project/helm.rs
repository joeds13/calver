use anyhow::Result;
use regex::Regex;
use std::path::{Path, PathBuf};

use crate::version::CalVer;

use super::ProjectFile;

#[derive(Debug)]
pub struct HelmFile {
    path: PathBuf,
}

impl HelmFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ProjectFile for HelmFile {
    fn path(&self) -> &Path {
        &self.path
    }

    fn current_version(&self) -> Result<Option<CalVer>> {
        let raw = std::fs::read_to_string(&self.path)?;
        let ver = raw.lines().find_map(parse_app_version_line);
        Ok(ver)
    }

    fn update_version(&self, version: &CalVer) -> Result<()> {
        let raw = std::fs::read_to_string(&self.path)?;
        let re = Regex::new(r"^(appVersion:\s*)(.+)$")?;
        let new_ver = version.to_string();
        let updated: String = raw
            .lines()
            .map(|line| {
                if let Some(caps) = re.captures(line) {
                    format!("{}{}", &caps[1], new_ver)
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Preserve trailing newline
        let updated = if raw.ends_with('\n') {
            format!("{updated}\n")
        } else {
            updated
        };

        std::fs::write(&self.path, updated)?;
        Ok(())
    }
}

fn parse_app_version_line(line: &str) -> Option<CalVer> {
    let value = line.strip_prefix("appVersion:")?.trim();
    // Strip optional surrounding quotes
    let value = value.trim_matches('"').trim_matches('\'');
    CalVer::parse(value)
}
