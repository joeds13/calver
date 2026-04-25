use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = ".calver.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct CalVerConfig {
    pub deploy: Vec<DeployTarget>,
}

/// A gitops deploy target — maps an app name to the file and key to update.
#[derive(Debug, Deserialize)]
pub struct DeployTarget {
    /// Logical app name, matched by `calver deploy --app <name>`.
    pub app: String,
    /// Path (relative to repo root) of the file containing the version.
    pub file: PathBuf,
    /// The key whose value will be replaced (simple string match on the key name).
    pub key: String,
    /// Optional prefix prepended to the version string (e.g. `"v"` → `v2026.4`).
    #[serde(default)]
    pub prefix: String,
}

impl CalVerConfig {
    /// Load config from `.calver.toml` in `dir`, returning a default config if absent.
    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join(CONFIG_FILE);
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)?;
        let cfg: Self = toml_edit::de::from_str(&raw)?;
        Ok(cfg)
    }

    pub fn deploy_target(&self, app: &str) -> Option<&DeployTarget> {
        self.deploy.iter().find(|t| t.app == app)
    }
}
