use anyhow::{Context, Result};
use serde::Serialize;

use crate::version::AnnoVer;

#[derive(Serialize)]
struct CreateReleaseRequest<'a> {
    tag_name: &'a str,
    name: &'a str,
    body: &'a str,
    prerelease: bool,
    draft: bool,
}

/// Create a GitHub release for the given version.
///
/// Requires the tag to already exist in the remote repository.
pub fn create_release(
    owner: &str,
    repo: &str,
    version: &AnnoVer,
    token: &str,
    body: &str,
) -> Result<String> {
    let tag = version.to_string();
    let name = format!("Release {tag}");
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases");

    let payload = CreateReleaseRequest {
        tag_name: &tag,
        name: &name,
        body,
        prerelease: version.is_dev(),
        draft: false,
    };

    let response = ureq::post(&url)
        .set("Authorization", &format!("token {token}"))
        .set("User-Agent", "annover-cli")
        .set("Accept", "application/vnd.github.v3+json")
        .send_json(ureq::serde_json::to_value(&payload)?)
        .context("failed to call GitHub releases API")?;

    let json: serde_json::Value = response.into_json()?;
    let html_url = json["html_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("unexpected GitHub API response: {json}"))?
        .to_string();

    Ok(html_url)
}

/// Resolve a GitHub token from common environment variables.
pub fn resolve_token() -> Option<String> {
    std::env::var("GITHUB_TOKEN")
        .or_else(|_| std::env::var("GH_TOKEN"))
        .ok()
        .filter(|t| !t.is_empty())
}

/// Resolve a GitHub token, falling back to an interactive prompt in a terminal.
/// The pasted token is never written to disk or environment.
pub fn resolve_token_or_prompt() -> Option<String> {
    use std::io::{IsTerminal, Write};

    if let Some(token) = resolve_token() {
        return Some(token);
    }

    if !std::io::stdin().is_terminal() {
        return None;
    }

    eprint!("  paste GitHub token to create a release (Enter to skip): ");
    std::io::stderr().flush().ok()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok()?;

    let token = input.trim().to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}
