use self_update::cargo_crate_version;

const OWNER: &str = "fanaloka47";
const REPO: &str = "nybble";
const BIN: &str = "nybble";

/// Latest release version string if it is newer than the running build, else `None`.
pub fn newer_release() -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner(OWNER)
        .repo_name(REPO)
        .build()?
        .fetch()?;
    let current = cargo_crate_version!();
    Ok(releases.first().and_then(|r| {
        match self_update::version::bump_is_greater(current, &r.version) {
            Ok(true) => Some(r.version.clone()),
            _ => None,
        }
    }))
}

/// Download + swap the running binary for the latest release. Returns the new version.
pub fn apply_update() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner(OWNER)
        .repo_name(REPO)
        .bin_name(BIN)
        .show_download_progress(false)
        .current_version(cargo_crate_version!())
        .no_confirm(true)
        .build()?
        .update()?;
    Ok(status.version().to_string())
}

/// Relaunch the freshly installed binary and exit the current process.
pub fn restart() -> ! {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).spawn();
    }
    std::process::exit(0);
}
