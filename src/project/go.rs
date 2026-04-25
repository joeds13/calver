use anyhow::Result;
use regex::Regex;
use std::path::{Path, PathBuf};

use crate::version::AnnoVer;

use super::ProjectFile;

#[derive(Debug)]
pub struct GoFile {
    path: PathBuf,
}

impl GoFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ProjectFile for GoFile {
    fn path(&self) -> &Path {
        &self.path
    }

    fn current_version(&self) -> Result<Option<AnnoVer>> {
        let raw = std::fs::read_to_string(&self.path)?;
        let ver = raw.lines().find_map(parse_version_line);
        Ok(ver)
    }

    fn update_version(&self, version: &AnnoVer) -> Result<()> {
        let raw = std::fs::read_to_string(&self.path)?;
        // Matches: Version   = "2026.19"
        let re = Regex::new(r#"^(\s*Version\s*=\s*")([^"]+)(")"#)?;
        let new_ver = version.to_string();
        let updated: String = raw
            .lines()
            .map(|line| {
                if let Some(caps) = re.captures(line) {
                    format!("{}{}{}", &caps[1], new_ver, &caps[3])
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

pub fn is_version_file(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|c| c.lines().any(|l| parse_version_line(l).is_some()))
        .unwrap_or(false)
}

fn parse_version_line(line: &str) -> Option<AnnoVer> {
    let rest = line.trim().strip_prefix("Version")?;
    // Must be followed by optional whitespace then '='
    let rest = rest.trim_start().strip_prefix('=')?;
    let value = rest.trim().trim_matches('"');
    AnnoVer::parse(value)
}
