use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "calver",
    about = "Convention-based CalVer versioning for code projects",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Bump the version, update project files, commit, tag, and create a GitHub release.
    Bump(BumpArgs),

    /// Print the current version (highest git tag).
    Current,

    /// Preview the next version without making any changes.
    Next,

    /// Update an image tag in a gitops repository to trigger continuous deployment.
    Deploy(DeployArgs),
}

#[derive(clap::Args)]
struct BumpArgs {
    /// Commit message (default: "chore: bump version to <version>").
    #[arg(short, long)]
    message: Option<String>,

    /// Push the commit and tag to origin.
    #[arg(long, default_value_t = true, env = "CI")]
    push: bool,

    /// Skip pushing the commit and tag.
    #[arg(long, conflicts_with = "push")]
    no_push: bool,

    /// Skip creating a GitHub release even if GITHUB_TOKEN is set.
    #[arg(long)]
    no_release: bool,

    /// Print what would happen without making changes.
    #[arg(long)]
    dry_run: bool,
}

#[derive(clap::Args)]
struct DeployArgs {
    /// Container image name to update (e.g. ghcr.io/owner/app).
    #[arg(long)]
    image: String,

    /// New image tag to set (e.g. 2026.4).
    #[arg(long)]
    tag: String,

    /// Specific file to update. When omitted, all YAML files in the working
    /// directory that contain the image name are updated.
    #[arg(long)]
    file: Option<PathBuf>,

    /// Commit message (default: "chore: deploy <image>:<tag>").
    #[arg(long)]
    message: Option<String>,

    /// Push the commit to origin after updating.
    #[arg(long, default_value_t = true, env = "CI")]
    push: bool,

    /// Skip pushing.
    #[arg(long, conflicts_with = "push")]
    no_push: bool,

    /// Print what would happen without writing any files.
    #[arg(long)]
    dry_run: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Bump(args) => cmd_bump(args),
        Commands::Current => cmd_current(),
        Commands::Next => cmd_next(),
        Commands::Deploy(args) => cmd_deploy(args),
    }
}

// ── bump ─────────────────────────────────────────────────────────────────────

fn cmd_bump(args: BumpArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let push = !args.no_push && args.push;

    let branch = calver::git::current_branch()?;
    let next = calver::compute_next_version()?;

    println!(
        "{} {} → {}  (branch: {})",
        "calver".bold(),
        "bump".cyan(),
        next.to_string().green().bold(),
        branch.dimmed()
    );

    let project_files = calver::project::detect_all(&cwd);
    if project_files.is_empty() {
        eprintln!(
            "{} no project files found (Cargo.toml, package.json, pyproject.toml, Chart.yaml, .calver, VERSION)",
            "warning:".yellow()
        );
    }

    if args.dry_run {
        println!("{}", "dry run — no changes made".dimmed());
        for f in &project_files {
            println!("  would update {}", f.path().display());
        }
        return Ok(());
    }

    for f in &project_files {
        f.update_version(&next)
            .with_context(|| format!("updating {}", f.path().display()))?;
        println!("  updated {}", f.path().display().to_string().dimmed());
    }

    let msg = args
        .message
        .unwrap_or_else(|| format!("chore: bump version to {next}"));
    calver::git::create_commit(&msg)?;
    calver::git::create_tag(&next)?;
    println!("  committed and tagged {}", next.to_string().green());

    if push {
        calver::git::push_branch()?;
        calver::git::push_tag(&next)?;
        println!("  pushed branch and tag to origin");

        if !args.no_release {
            if let Some(token) = calver::github::resolve_token() {
                match calver::git::repo_info() {
                    Ok((owner, repo)) => {
                        let body = release_body(&next);
                        match calver::github::create_release(&owner, &repo, &next, &token, &body) {
                            Ok(url) => println!("  release created: {}", url.cyan()),
                            Err(e) => eprintln!("{} creating release: {e}", "warning:".yellow()),
                        }
                    }
                    Err(e) => eprintln!("{} resolving repo: {e}", "warning:".yellow()),
                }
            } else {
                println!(
                    "  {} GITHUB_TOKEN not set — skipping release",
                    "note:".dimmed()
                );
            }
        }
    }

    println!("{}", format!("✓ {next}").green().bold());
    Ok(())
}

fn release_body(version: &calver::CalVer) -> String {
    if version.is_dev() {
        format!("Pre-release build `{version}` from a feature branch.")
    } else {
        format!("Release `{version}`.")
    }
}

// ── current ──────────────────────────────────────────────────────────────────

fn cmd_current() -> Result<()> {
    match calver::current_version()? {
        Some(v) => println!("{v}"),
        None => println!("{}", "no version tags found".dimmed()),
    }
    Ok(())
}

// ── next ─────────────────────────────────────────────────────────────────────

fn cmd_next() -> Result<()> {
    let next = calver::compute_next_version()?;
    println!("{next}");
    Ok(())
}

// ── deploy ───────────────────────────────────────────────────────────────────

fn cmd_deploy(args: DeployArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let push = !args.no_push && args.push;

    println!(
        "{} {} → {}:{}",
        "calver".bold(),
        "deploy".cyan(),
        args.image.dimmed(),
        args.tag.green().bold(),
    );

    if args.dry_run {
        println!("{}", "dry run — no changes made".dimmed());
        return Ok(());
    }

    let changed_files = match &args.file {
        Some(path) => {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            let (updated, changed) =
                calver::gitops::update_image_in_content(&content, &args.image, &args.tag)?;
            if !changed {
                anyhow::bail!(
                    "no references to '{}' found in {}",
                    args.image,
                    path.display()
                );
            }
            std::fs::write(path, updated)?;
            vec![path.clone()]
        }
        None => {
            let files = calver::gitops::update_files_in_dir(&cwd, &args.image, &args.tag)?;
            if files.is_empty() {
                anyhow::bail!(
                    "no YAML files containing '{}' found under {}",
                    args.image,
                    cwd.display()
                );
            }
            files
        }
    };

    for path in &changed_files {
        println!("  updated {}", path.display().to_string().dimmed());
    }

    let msg = args
        .message
        .unwrap_or_else(|| format!("chore: deploy {}:{}", args.image, args.tag));
    calver::git::create_commit(&msg)?;
    println!("  committed");

    if push {
        calver::git::push_branch()?;
        println!("  pushed to origin");
    }

    println!(
        "{}",
        format!("✓ {}:{}", args.image, args.tag).green().bold()
    );
    Ok(())
}
