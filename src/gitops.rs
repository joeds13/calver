use anyhow::{Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};

/// Updates all occurrences of an image reference to a new tag within file content.
///
/// Three patterns are supported (all are applied, any that match are updated):
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
/// Returns `(updated_content, was_changed)`.
pub fn update_image_in_content(content: &str, image: &str, tag: &str) -> Result<(String, bool)> {
    let mut lines: Vec<String> = content.lines().map(str::to_owned).collect();
    let mut changed = false;

    // Pattern 1: kustomize newTag — requires tracking state across lines
    changed |= apply_kustomize_new_tag(&mut lines, image, tag)?;

    // Pattern 2 & 3: single-line replacements
    let ref_re = Regex::new(r#"(ref=)[^\s"'&>]+"#)?;
    let tag_re = Regex::new(&format!(r#"({}:)[^\s"',\]}}]+"#, regex::escape(image)))?;

    for line in &mut lines {
        if !line.contains(image) {
            continue;
        }
        // Pattern 2: ref= on a line that also mentions the image
        if line.contains("ref=") {
            let new = ref_re.replace_all(line, format!("${{1}}{tag}")).to_string();
            if new != *line {
                *line = new;
                changed = true;
            }
        }
        // Pattern 3: image:oldtag inline
        if tag_re.is_match(line) {
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
fn apply_kustomize_new_tag(lines: &mut Vec<String>, image: &str, tag: &str) -> Result<bool> {
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

    for entry in walkdir(dir) {
        let path = entry?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "yaml" | "yml") {
            continue;
        }

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;

        if !content.contains(image) {
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
}
