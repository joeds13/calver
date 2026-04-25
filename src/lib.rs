pub mod git;
pub mod github;
pub mod gitops;
pub mod project;
pub mod version;

pub use version::AnnoVer;

/// Compute the next version for the current repo state.
///
/// Reads git tags and current branch to determine whether to produce a
/// release version or a dev pre-release.
pub fn compute_next_version() -> anyhow::Result<AnnoVer> {
    let branch = git::current_branch()?;
    let tags = git::version_tags()?;
    let latest_release = git::latest_release_tag(&tags);

    if git::is_main_branch(&branch) {
        Ok(AnnoVer::next_main(latest_release))
    } else {
        let base = AnnoVer::next_main(latest_release);
        let latest_dev = git::latest_dev_tag(&tags, &base);
        Ok(AnnoVer::next_dev(&base, latest_dev))
    }
}

/// Return the highest version tag in the repo, or None if there are no tags.
pub fn current_version() -> anyhow::Result<Option<AnnoVer>> {
    let tags = git::version_tags()?;
    Ok(tags.into_iter().max())
}
