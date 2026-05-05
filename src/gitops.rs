use anyhow::{Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};

/// Updates all occurrences of an image reference to a new tag within file content.
///
/// Four patterns are supported (all are applied, any that match are updated):
///
/// 1. **Kustomize images block** — `newTag:` on the line following `name: <image>`:
///    ```yaml
///    images:
///      - name: ghcr.io/owner/app
///        newTag: 2026.3
///    ```
///
/// 2. **URL `ref=` parameter** — anywhere `<image>` appears on a line with `ref=`:
///    ```yaml
///    - oci://ghcr.io/owner/app//kustomize?ref=2026.3
///    ```
///
/// 3. **Inline `image:tag`** — the image followed immediately by `:tag`:
///    ```yaml
///    image: ghcr.io/owner/app:2026.3
///    ```
///
/// 4. **Argo CD Application `targetRevision`** — the `targetRevision` field that
///    is a sibling of a `repoURL` containing the image or its `owner/repo` path:
///    ```yaml
///    source:
///      repoURL: https://github.com/owner/app
///      targetRevision: "2026.3"
///    ```
///
/// Returns `(updated_content, was_changed)`.
pub fn update_image_in_content(content: &str, image: &str, tag: &str) -> Result<(String, bool)> {
    let mut lines: Vec<String> = content.lines().map(str::to_owned).collect();
    let mut changed = false;

    // Pattern 1: kustomize newTag — requires tracking state across lines
    changed |= apply_kustomize_new_tag(&mut lines, image, tag)?;

    // Pattern 4: Argo CD Application targetRevision — requires tracking state across lines
    changed |= apply_target_revision(&mut lines, image, tag)?;

    // Pattern 2 & 3: single-line replacements
    let ref_re = Regex::new(r#"(ref=)[^\s"'&>]+"#)?;
    let tag_re = Regex::new(&format!(r#"({}:)[^\s"',\]}}]+"#, regex::escape(image)))?;
    // Strip the registry prefix so "ghcr.io/owner/repo" also matches resource
    // URLs like "https://github.com/owner/repo/k8s?ref=..." where only the
    // owner/repo portion appears.
    let image_repo = image.find('/').map(|i| &image[i + 1..]).unwrap_or("");

    for line in &mut lines {
        let has_image = line.contains(image);
        let has_repo = !image_repo.is_empty() && line.contains(image_repo);

        if !has_image && !has_repo {
            continue;
        }
        // Pattern 2: ref= on a line mentioning the image or its owner/repo path
        if line.contains("ref=") {
            let new = ref_re.replace_all(line, format!("${{1}}{tag}")).to_string();
            if new != *line {
                *line = new;
                changed = true;
            }
        }
        // Pattern 3: image:oldtag inline (requires exact image name)
        if has_image && tag_re.is_match(line) {
            let new = tag_re.replace_all(line, format!("${{1}}{tag}")).to_string();
            if new != *line {
                *line = new;
                changed = true;
            }
        }
    }

    let mut result = lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }
    Ok((result, changed))
}

/// State-machine pass to update `newTag:` in a kustomize images block.
fn apply_kustomize_new_tag(lines: &mut [String], image: &str, tag: &str) -> Result<bool> {
    let name_re = Regex::new(&format!(r"(?:^|\s)-?\s*name:\s+{}", regex::escape(image)))?;
    let new_tag_re = Regex::new(r"^(\s*newTag:\s*)(.+)$")?;

    let mut changed = false;
    let mut in_target = false;
    let mut target_indent: usize = 0;

    for line in lines.iter_mut() {
        if name_re.is_match(line) {
            in_target = true;
            // Record indentation of the list item so we know when it ends
            target_indent = line.len() - line.trim_start().len();
            continue;
        }

        if in_target {
            let indent = line.len() - line.trim_start().len();
            let trimmed = line.trim();

            // Empty or comment lines: keep scanning
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // New list item or same/lower indentation key → left the image block
            if indent <= target_indent && !trimmed.starts_with("newTag") {
                in_target = false;
                continue;
            }

            if let Some(caps) = new_tag_re.captures(line) {
                let new_line = format!("{}{}", &caps[1], tag);
                if new_line != *line {
                    *line = new_line;
                    changed = true;
                }
                in_target = false;
            }
        }
    }

    Ok(changed)
}

/// Find all YAML files under `dir` that contain `image`, update them, and return
/// the list of paths that were actually changed.
pub fn update_files_in_dir(dir: &Path, image: &str, tag: &str) -> Result<Vec<PathBuf>> {
    let mut changed = Vec::new();
    // Strip the registry prefix so that Argo CD Application files whose
    // `repoURL` only contains the `owner/repo` portion are also considered.
    let image_repo = image.find('/').map(|i| &image[i + 1..]).unwrap_or("");

    for entry in walkdir(dir) {
        let path = entry?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "yaml" | "yml") {
            continue;
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;

        let content_has_match =
            content.contains(image) || (!image_repo.is_empty() && content.contains(image_repo));
        if !content_has_match {
            continue;
        }

        let (updated, was_changed) = update_image_in_content(&content, image, tag)?;
        if was_changed {
            std::fs::write(&path, updated)
                .with_context(|| format!("writing {}", path.display()))?;
            changed.push(path);
        }
    }

    Ok(changed)
}

/// State-machine pass to update `targetRevision:` in Argo CD `Application`
/// resources where the sibling `repoURL` field matches `image` (or its
/// `owner/repo` suffix after stripping the registry host).
///
/// Handles both single-source (`source:`) and multi-source (`sources:`) shapes.
fn apply_target_revision(lines: &mut [String], image: &str, tag: &str) -> Result<bool> {
    let image_repo = image.find('/').map(|i| &image[i + 1..]).unwrap_or("");
    // Match `targetRevision:` with optional surrounding quotes (single, double,
    // or none) so we preserve whatever quoting style was already in use.
    let revision_re = Regex::new(r#"^(\s*targetRevision:\s*["']?)([^"'\s]+)(["']?\s*)$"#)?;

    let mut changed = false;
    let mut looking = false;
    // Column position of the 'r' in `repoURL` — used as the reference indent
    // level so that both `repoURL:` (single source) and `- repoURL:` (list
    // item in `sources:`) work correctly.
    let mut base_col: usize = 0;

    for line in lines.iter_mut() {
        if !looking {
            let has_image = line.contains(image);
            let has_repo = !image_repo.is_empty() && line.contains(image_repo);
            if (has_image || has_repo) && line.contains("repoURL:") {
                looking = true;
                base_col = line.find("repoURL").unwrap_or(0);
            }
            continue;
        }

        // --- inside the "looking for targetRevision" state ---
        let trimmed = line.trim();

        // Skip blank lines and comments.
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = line.len() - line.trim_start().len();

        // A line whose text starts at a lower column than `repoURL` means we
        // have left the enclosing block without finding `targetRevision`.
        if indent < base_col {
            looking = false;
            // This line may itself be a new `repoURL` match; don't skip it.
            let has_image = line.contains(image);
            let has_repo = !image_repo.is_empty() && line.contains(image_repo);
            if (has_image || has_repo) && line.contains("repoURL:") {
                looking = true;
                base_col = line.find("repoURL").unwrap_or(0);
            }
            continue;
        }

        if let Some(caps) = revision_re.captures(line) {
            let new_line = format!("{}{}{}", &caps[1], tag, &caps[3]);
            if new_line != *line {
                *line = new_line;
                changed = true;
            }
            looking = false;
        }
    }

    Ok(changed)
}

fn walkdir(dir: &Path) -> impl Iterator<Item = Result<PathBuf>> {
    let mut stack = vec![dir.to_path_buf()];
    std::iter::from_fn(move || {
        while let Some(path) = stack.pop() {
            if path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        // Skip hidden directories (e.g. .git)
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
                return Some(Ok(path));
            }
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kustomize_new_tag() {
        let content = "\
images:
  - name: ghcr.io/owner/app
    newTag: 2026.1
  - name: ghcr.io/owner/other
    newTag: 1.0
";
        let (updated, changed) =
            update_image_in_content(content, "ghcr.io/owner/app", "2026.4").unwrap();
        assert!(changed);
        assert!(updated.contains("newTag: 2026.4"));
        assert!(
            updated.contains("newTag: 1.0"),
            "other image should be unchanged"
        );
    }

    #[test]
    fn url_ref_pattern() {
        let content = "\
resources:
  - oci://ghcr.io/owner/app//kustomize?ref=2026.1
";
        let (updated, changed) =
            update_image_in_content(content, "ghcr.io/owner/app", "2026.4").unwrap();
        assert!(changed);
        assert!(updated.contains("ref=2026.4"));
    }

    #[test]
    fn kustomize_resource_github_ref() {
        let content = "\
resources:
  - https://github.com/owner/app/k8s?ref=2026.1
  - externalsecret.yaml
";
        let (updated, changed) =
            update_image_in_content(content, "ghcr.io/owner/app", "2026.4").unwrap();
        assert!(changed);
        assert!(updated.contains("ref=2026.4"));
        assert!(
            updated.contains("externalsecret.yaml"),
            "other resources unchanged"
        );
    }

    #[test]
    fn inline_image_tag() {
        let content = "image: ghcr.io/owner/app:2026.1\n";
        let (updated, changed) =
            update_image_in_content(content, "ghcr.io/owner/app", "2026.4").unwrap();
        assert!(changed);
        assert!(updated.contains("ghcr.io/owner/app:2026.4"));
    }

    #[test]
    fn no_match_returns_unchanged() {
        let content = "image: ghcr.io/owner/other:2026.1\n";
        let (_, changed) = update_image_in_content(content, "ghcr.io/owner/app", "2026.4").unwrap();
        assert!(!changed);
    }

    #[test]
    fn preserves_trailing_newline() {
        let content = "image: ghcr.io/owner/app:2026.1\n";
        let (updated, _) = update_image_in_content(content, "ghcr.io/owner/app", "2026.4").unwrap();
        assert!(updated.ends_with('\n'));
    }

    // ── Pattern 4: Argo CD Application targetRevision ────────────────────────

    #[test]
    fn argocd_target_revision_double_quoted() {
        let content = "\
apiVersion: argoproj.io/v1alpha1
kind: Application
spec:
  source:
    path: k8s
    repoURL: https://github.com/owner/app
    targetRevision: \"2026.1\"
";
        let (updated, changed) =
            update_image_in_content(content, "ghcr.io/owner/app", "2026.4").unwrap();
        assert!(changed);
        assert!(
            updated.contains("targetRevision: \"2026.4\""),
            "expected double-quoted version to be updated: {updated}"
        );
    }

    #[test]
    fn argocd_target_revision_unquoted() {
        let content = "\
spec:
  source:
    repoURL: https://github.com/owner/app
    targetRevision: 2026.1
";
        let (updated, changed) =
            update_image_in_content(content, "ghcr.io/owner/app", "2026.4").unwrap();
        assert!(changed);
        assert!(
            updated.contains("targetRevision: 2026.4"),
            "expected unquoted version to be updated: {updated}"
        );
    }

    #[test]
    fn argocd_target_revision_only_repo_match() {
        // The image has a different registry host; only the owner/repo portion
        // appears in the repoURL — it should still be updated.
        let content = "\
spec:
  source:
    repoURL: https://github.com/joeds13/raceweek
    targetRevision: \"2026.1\"
";
        let (updated, changed) =
            update_image_in_content(content, "ghcr.io/joeds13/raceweek", "2026.4").unwrap();
        assert!(changed);
        assert!(updated.contains("targetRevision: \"2026.4\""));
    }

    #[test]
    fn argocd_target_revision_list_sources() {
        // Multi-source Application: only the matching repoURL's targetRevision
        // should be updated.
        let content = "\
spec:
  sources:
    - repoURL: https://github.com/owner/app
      targetRevision: \"2026.1\"
      path: k8s
    - repoURL: https://github.com/owner/other
      targetRevision: \"1.0\"
      path: charts
";
        let (updated, changed) =
            update_image_in_content(content, "ghcr.io/owner/app", "2026.4").unwrap();
        assert!(changed);
        assert!(
            updated.contains("targetRevision: \"2026.4\""),
            "matching source should be updated: {updated}"
        );
        assert!(
            updated.contains("targetRevision: \"1.0\""),
            "non-matching source should be unchanged: {updated}"
        );
    }

    #[test]
    fn argocd_target_revision_unrelated_repo_unchanged() {
        let content = "\
spec:
  source:
    repoURL: https://github.com/owner/completely-different
    targetRevision: \"2026.1\"
";
        let (_, changed) = update_image_in_content(content, "ghcr.io/owner/app", "2026.4").unwrap();
        assert!(!changed, "unrelated repoURL must not be touched");
    }
}
