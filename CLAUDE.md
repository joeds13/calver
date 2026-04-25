# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

AnnoVer is a CalVer CLI tool (`<year>.<int>` on main, `<year>.<int>-dev<int>` on feature branches) that reads Git tags to compute the next version, updates all recognized version files, commits, tags, pushes, and creates GitHub releases.

## Commands

Tasks are run via `mise`:

```bash
mise run build    # cargo build
mise run test     # cargo test
mise run lint     # cargo clippy -- -D warnings
mise run fmt      # cargo fmt
mise run check    # prek run --all-files (fmt + lint + test)
mise run release  # cargo build --release
```

Pre-commit hooks (via prek) enforce fmt, clippy, and tests before every commit.

## Architecture

All logic lives in `src/`:

- **`lib.rs`** — public API: `compute_next_version()`, `current_version()` (exposed for future PyO3 bindings)
- **`main.rs`** — CLI: `bump`, `current`, `next`, `deploy` subcommands
- **`version.rs`** — `AnnoVer` struct: parsing, formatting, ordering, `next_main()`, `next_dev()`
- **`git.rs`** — shells out to `git`; detects branch (handles detached HEAD and `GITHUB_HEAD_REF`), reads tags, creates commits/tags, pushes
- **`github.rs`** — GitHub Releases API via `GITHUB_TOKEN`/`GH_TOKEN`
- **`gitops.rs`** — updates image tags in YAML files (three patterns: Kustomize `newTag:`, URL `ref=`, inline `image:tag`)
- **`config.rs`** — loads `.annover.toml` for named deploy targets
- **`project/`** — `ProjectFile` trait + implementations for Cargo.toml, package.json, pyproject.toml, Chart.yaml, `.annover`/`VERSION`; `detect_all()` finds all present files automatically

## Key Invariants

- Cargo requires semver, so `.0` is appended on write (`2026.4` → `2026.4.0`) and stripped on read.
- Year rollover resets the increment to `1`; same year just increments.
- Dev pre-releases are based on what *would* be the next main version, so feature branches don't collide with each other.
- `--dry-run` skips all file writes and git operations.
- GitHub token is optional for `bump` (release silently skipped if absent) but required for `deploy` against a remote gitops repo.
