use anyhow::{Context, Result};
use std::process::Command;

use crate::version::CalVer;

fn git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .context("failed to execute git")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {} failed: {}", args[0], stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Returns the current branch name, or `HEAD` when detached.
pub fn current_branch() -> Result<String> {
    // Handles detached HEAD (e.g. in CI with a checked-out SHA)
    let symbolic = git(&["symbolic-ref", "--short", "HEAD"]);
    if let Ok(b) = symbolic {
        return Ok(b);
    }
    // Fallback: look at GITHUB_HEAD_REF (PRs) or GITHUB_REF_NAME (pushes)
    if let Ok(r) = std::env::var("GITHUB_HEAD_REF") {
        if !r.is_empty() {
            return Ok(r);
        }
    }
    if let Ok(r) = std::env::var("GITHUB_REF_NAME") {
        if !r.is_empty() {
            return Ok(r);
        }
    }
    Ok("HEAD".to_string())
}

pub fn is_main_branch(branch: &str) -> bool {
    matches!(branch, "main" | "master")
}

/// All calver-formatted tags in the repo, sorted ascending.
pub fn version_tags() -> Result<Vec<CalVer>> {
    let raw = git(&["tag", "--list"])?;
    let mut versions: Vec<CalVer> = raw
        .lines()
        .filter_map(|t| CalVer::parse(t.trim()))
        .collect();
    versions.sort();
    Ok(versions)
}

/// Highest release (non-dev) tag.
pub fn latest_release_tag(tags: &[CalVer]) -> Option<&CalVer> {
    tags.iter().filter(|v| !v.is_dev()).max()
}

/// Highest dev tag matching the given base version.
pub fn latest_dev_tag<'a>(tags: &'a [CalVer], base: &CalVer) -> Option<&'a CalVer> {
    tags.iter()
        .filter(|v| v.is_dev() && v.year == base.year && v.increment == base.increment)
        .max()
}

/// Stage all tracked modified files and create a commit.
pub fn create_commit(message: &str) -> Result<()> {
    git(&["add", "-u"])?;
    git(&["commit", "--message", message])?;
    Ok(())
}

/// Create an annotated tag at HEAD.
pub fn create_tag(version: &CalVer) -> Result<()> {
    let tag = version.to_string();
    let msg = format!("Release {tag}");
    git(&["tag", "--annotate", &tag, "--message", &msg])?;
    Ok(())
}

/// Push a tag to origin.
pub fn push_tag(version: &CalVer) -> Result<()> {
    git(&["push", "origin", &version.to_string()])?;
    Ok(())
}

/// Push the current branch to origin.
pub fn push_branch() -> Result<()> {
    git(&["push"])?;
    Ok(())
}

/// Parse `owner` and `repo` from the origin remote URL.
///
/// Handles both SSH (`git@github.com:owner/repo.git`) and HTTPS forms.
pub fn repo_info() -> Result<(String, String)> {
    let url = git(&["remote", "get-url", "origin"]).context("no origin remote found")?;

    // Strip trailing .git
    let url = url.strip_suffix(".git").unwrap_or(&url);

    // HTTPS: https://github.com/owner/repo
    if let Some(path) = url.strip_prefix("https://github.com/") {
        let (owner, repo) = split_owner_repo(path)?;
        return Ok((owner, repo));
    }

    // SSH: git@github.com:owner/repo
    if let Some(path) = url.strip_prefix("git@github.com:") {
        let (owner, repo) = split_owner_repo(path)?;
        return Ok((owner, repo));
    }

    anyhow::bail!("cannot parse GitHub owner/repo from remote URL: {url}")
}

fn split_owner_repo(path: &str) -> Result<(String, String)> {
    let mut parts = path.splitn(2, '/');
    let owner = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing owner in remote URL"))?;
    let repo = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing repo in remote URL"))?;
    Ok((owner.to_string(), repo.to_string()))
}

/// Returns the configured git user name and email for commit authorship.
pub fn user_identity() -> (String, String) {
    let name = git(&["config", "user.name"]).unwrap_or_else(|_| "calver".to_string());
    let email = git(&["config", "user.email"]).unwrap_or_else(|_| "calver@localhost".to_string());
    (name, email)
}
