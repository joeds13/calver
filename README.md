# AnnoVer

Convention-based [CalVer](https://calver.org) versioning CLI for code projects.

Named doubly: first as a differentiator from CalVer as we only use the year and "anno" is Latin for "in the year"; and also because it sounds a bit like "another" which is apt as it's for bumping an incrementing version to the next.

Versions follow the scheme `<year>.<int>` on the main branch and `<year>.<int>-dev<int>` on all other branches. Git tags are the source of truth. One command updates every version file in the project, commits as the current user, pushes, and creates a GitHub release.

## Usage

### Commands

```
annover bump     Update project files, commit, tag, push, and create a GitHub release
annover current  Print the current version (highest git tag)
annover next     Preview the next version without making any changes
annover deploy   Update an image tag in a gitops repository to trigger CD
```

### bump

Run in any project that has a recognised version file (see [Supported files](#supported-files)):

```sh
annover bump
```

On `main` this produces `2026.1`, `2026.2`, … On any other branch it produces `2026.1-dev1`, `2026.1-dev2`, … (based on what the next main release would be).

Flags:

| Flag | Default | Description |
|---|---|---|
| `--message` | `chore: bump version to <v>` | Override the commit message |
| `--no-push` | — | Skip pushing the branch and tag |
| `--no-release` | — | Skip creating a GitHub release |
| `--dry-run` | — | Preview changes without writing anything |

A `GITHUB_TOKEN` (or `GH_TOKEN`) environment variable is required to create releases. If it is absent the release step is silently skipped.

### deploy

Updates an image tag in a checked-out gitops repository and pushes the result, intended to be called from CI after building a new container image.

```sh
annover deploy --image ghcr.io/owner/app --tag 2026.4
```

When `--file` is omitted, annover searches all YAML files in the working directory for the image name and updates every match it finds. Supported reference patterns:

| Pattern | Example |
|---|---|
| Kustomize `images` block | `newTag:` under `name: ghcr.io/owner/app` |
| OCI/git URL `ref=` | `oci://ghcr.io/owner/app//kustomize?ref=2026.3` |
| Inline `image:tag` | `image: ghcr.io/owner/app:2026.3` |

To target a specific file:

```sh
annover deploy --image ghcr.io/owner/app --tag 2026.4 --file apps/myapp/kustomization.yaml
```

### Supported files

`annover bump` detects and updates all of the following that exist in the working directory:

| File | Field |
|---|---|
| `Cargo.toml` | `[package] version` |
| `package.json` | `version` |
| `pyproject.toml` | `[project] version` or `[tool.poetry] version` |
| `Chart.yaml` | `appVersion` |
| `.annover` / `VERSION` | plain text |

### Installation

**mise** (recommended):

```toml
# mise.toml
[tools]
"github:joeds13/annover" = "latest"
```

**Cargo**:

```sh
cargo install annover
```

### GitHub Action

```yaml
- uses: joeds13/annover@main
  with:
    token: ${{ secrets.GITHUB_TOKEN }}
```

For a full CD pipeline — bump in the app repo, then update the gitops repo:

```yaml
jobs:
  release:
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0   # tags are needed to compute the next version

      - name: Bump version
        id: annover
        uses: joeds13/annover@main
        with:
          token: ${{ secrets.GITHUB_TOKEN }}

      # build & push your Docker image here, tagged ${{ steps.annover.outputs.version }}

      - name: Update gitops repo
        uses: joeds13/annover@main
        with:
          repository: you/gitops
          token: ${{ secrets.GITOPS_TOKEN }}
          image: ghcr.io/you/app
          tag: ${{ steps.annover.outputs.version }}
```

---

## Development

**Prerequisites:** Rust stable (≥ 1.86), mise.

```sh
git clone https://github.com/joeds13/annover
cd annover
mise install        # installs Rust stable
```

### Tasks

```sh
mise run build      # cargo build
mise run test       # cargo test
mise run lint       # cargo clippy
mise run fmt        # cargo fmt
mise run check      # fmt + lint + test
```

### Project layout

```
src/
  lib.rs            public API (compute_next_version, current_version)
  main.rs           CLI definitions and command dispatch
  version.rs        AnnoVer type — parsing, ordering, next-version logic
  git.rs            git operations (shells out to git)
  github.rs         GitHub releases API
  gitops.rs         image-tag update logic for gitops YAML files
  project/          convention-based file updaters (bump)
    cargo.rs
    npm.rs
    python.rs
    helm.rs
    plain.rs
action.yml          composite GitHub Action
.github/workflows/
  ci.yml            lint + test on push/PR
  release.yml       cross-compiled binaries published on tag
```

The library crate (`src/lib.rs`) exposes a stable API surface kept deliberately minimal to make future Python bindings (PyO3) straightforward to add.
