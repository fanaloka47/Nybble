//! Self-update against GitHub Releases.
//!
//! Nybble ships in several flavors — a portable archive, a Windows MSI, a
//! Debian package, and an AppImage — and they do not all get to update the same
//! way. A portable build owns the directory it lives in and can swap its own
//! executable; an MSI build sits in `Program Files` and cannot, because the app
//! runs unelevated (`asInvoker`); a `.deb` build must never be touched behind
//! dpkg's back. [`UpdateStrategy`] resolves which case we are in, and
//! [`apply_update`] does the corresponding thing.
//!
//! Orthogonally, [`Channel`] decides *which* releases this build is willing to
//! see. The check and the download must agree on that — see [`newer_release_info`].

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use self_update::cargo_crate_version;
use self_update::update::{Release, ReleaseAsset, ReleaseUpdate};

const OWNER: &str = "fanaloka47";
const REPO: &str = "nybble";
const BIN: &str = "nybble";

/// Where to send users whose install flavor can't update itself.
pub const RELEASES_URL: &str = "https://github.com/fanaloka47/nybble/releases/latest";

type Res<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// How this build is allowed to update itself.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UpdateStrategy {
    /// Portable build in a writable directory: swap the executable in place.
    ReplaceBinary,
    /// AppImage: replace the outer `.AppImage` artifact, not the running exe.
    ReplaceAppImage,
    /// Windows, non-writable install dir: download the MSI and let it upgrade us.
    RunInstaller,
    /// System package manager owns these files. Inform only, never write.
    Managed,
}

impl UpdateStrategy {
    /// Whether the app itself performs the update, vs. pointing at a download.
    pub fn is_self_applying(self) -> bool {
        !matches!(self, Self::Managed)
    }

    /// Button label offering the update to version `v`.
    pub fn button_label(self, v: &str) -> String {
        match self {
            Self::ReplaceBinary | Self::ReplaceAppImage => format!("Update & restart (v{v})"),
            Self::RunInstaller => format!("Update (v{v})"),
            Self::Managed => format!("v{v} available"),
        }
    }

    /// Hover text explaining what clicking will actually do.
    pub fn hover_text(self) -> &'static str {
        match self {
            Self::ReplaceBinary | Self::ReplaceAppImage => "Download the new version and restart",
            Self::RunInstaller => "Download the installer and run it — Windows will ask to elevate",
            Self::Managed => "Installed via your package manager — open the releases page",
        }
    }
}

/// What the caller should do once [`apply_update`] returns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Applied {
    /// The new build is in place; relaunch it.
    Restart,
    /// An external installer is running and needs our files free; just exit.
    Exit,
    /// Nothing was installed because we were already current. Restarting here
    /// would be a pointless relaunch that leaves the same offer on screen.
    NoChange,
}

/// Which releases this build is willing to be offered.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Channel {
    /// Whatever GitHub calls the latest release: never a draft or pre-release.
    #[default]
    Stable,
    /// Also consider pre-releases, newest first.
    Beta,
}

/// Opt into pre-release builds with `PC_UPDATE_CHANNEL=beta`.
///
/// Deliberately an environment variable and not a settings checkbox. Release
/// candidates are for people who went looking for them; a visible toggle would
/// invite users onto a channel they didn't mean to join, and there is no
/// downgrade path once an RC is installed.
pub fn channel() -> Channel {
    static CACHE: OnceLock<Channel> = OnceLock::new();
    *CACHE.get_or_init(|| channel_from_env(std::env::var("PC_UPDATE_CHANNEL").ok().as_deref()))
}

fn channel_from_env(value: Option<&str>) -> Channel {
    match value {
        Some(v) if v.trim().eq_ignore_ascii_case("beta") => Channel::Beta,
        _ => Channel::Stable,
    }
}

/// Resolve (once) how this build can update itself.
pub fn strategy() -> UpdateStrategy {
    static CACHE: OnceLock<UpdateStrategy> = OnceLock::new();
    *CACHE.get_or_init(detect_strategy)
}

fn detect_strategy() -> UpdateStrategy {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf));
    resolve_strategy(appimage_path(), exe_dir, cfg!(windows), dir_is_writable)
}

/// The decision itself, with the environment passed in so it can be tested.
fn resolve_strategy(
    appimage: Option<PathBuf>,
    exe_dir: Option<PathBuf>,
    windows: bool,
    writable: impl Fn(&Path) -> bool,
) -> UpdateStrategy {
    // $APPIMAGE is checked first: inside an AppImage the running executable
    // lives in a throwaway extraction directory that looks perfectly writable,
    // so probing the exe's location would reach the wrong conclusion.
    if let Some(img) = appimage {
        return match img.parent() {
            Some(dir) if writable(dir) => UpdateStrategy::ReplaceAppImage,
            _ => UpdateStrategy::Managed,
        };
    }

    match exe_dir {
        Some(dir) if writable(&dir) => UpdateStrategy::ReplaceBinary,
        // Not writable. On Windows an MSI can upgrade us with elevation; on
        // Unix a non-writable install dir means a package manager owns it.
        _ if windows => UpdateStrategy::RunInstaller,
        _ => UpdateStrategy::Managed,
    }
}

/// Path to the outer `.AppImage` artifact, if we are running as one.
fn appimage_path() -> Option<PathBuf> {
    let p = std::env::var_os("APPIMAGE")?;
    if p.is_empty() {
        return None;
    }
    Some(PathBuf::from(p))
}

/// Can this process create files in `dir`? Answers the question that actually
/// matters — whether an in-place swap will succeed — rather than pattern
/// matching on install paths we may not have anticipated.
fn dir_is_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".nybble-write-probe-{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Build a configured updater, optionally pinned to one exact release tag.
fn updater(target_tag: Option<&str>) -> Res<Box<dyn ReleaseUpdate>> {
    let mut b = self_update::backends::github::Update::configure();
    b.repo_owner(OWNER)
        .repo_name(REPO)
        .bin_name(BIN)
        .show_download_progress(false)
        .current_version(cargo_crate_version!())
        .no_confirm(true);
    if let Some(tag) = target_tag {
        b.target_version_tag(tag);
    }
    Ok(b.build()?)
}

/// The candidate release for this channel, if it is newer than the running build.
///
/// The two channels deliberately use different endpoints:
///
/// * `Stable` asks for `/releases/latest`, which GitHub documents as the most
///   recent non-draft, non-pre-release release. Filtering server-side beats
///   guessing from the tag name — it honours the actual "Set as a pre-release"
///   checkbox rather than a naming convention.
/// * `Beta` lists `/releases` and takes the newest entry, pre-releases included.
fn newer_release_info() -> Res<Option<Release>> {
    let release = match channel() {
        Channel::Stable => updater(None)?.get_latest_release()?,
        Channel::Beta => {
            let releases = self_update::backends::github::ReleaseList::configure()
                .repo_owner(OWNER)
                .repo_name(REPO)
                .build()?
                .fetch()?;
            match releases.into_iter().next() {
                Some(r) => r,
                None => return Ok(None),
            }
        }
    };

    let current = cargo_crate_version!();
    Ok(
        match self_update::version::bump_is_greater(current, &release.version) {
            Ok(true) => Some(release),
            _ => None,
        },
    )
}

/// Latest release version string if it is newer than the running build, else `None`.
pub fn newer_release() -> Res<Option<String>> {
    Ok(newer_release_info()?.map(|r| r.version))
}

/// Bring this install up to date, however its flavor allows.
///
/// Resolves the target release once and hands it to every path, so the version
/// we install is always the version we offered.
pub fn apply_update() -> Res<Applied> {
    let release = match newer_release_info()? {
        Some(r) => r,
        None => return Ok(Applied::NoChange),
    };
    match strategy() {
        UpdateStrategy::ReplaceBinary => replace_binary(&release.version),
        UpdateStrategy::ReplaceAppImage => {
            replace_appimage(&release)?;
            Ok(Applied::Restart)
        }
        UpdateStrategy::RunInstaller => {
            run_installer(&release)?;
            Ok(Applied::Exit)
        }
        UpdateStrategy::Managed => {
            Err("this install is managed by a package manager and cannot self-update".into())
        }
    }
}

/// Portable build: let `self_update` swap the running executable.
///
/// Pinned to an exact tag rather than left to find "latest" on its own. Without
/// the pin the download path always resolves `/releases/latest`, which excludes
/// pre-releases — so a beta user offered an RC would silently receive the stable
/// build instead, and a user with no newer stable release would be told the
/// update succeeded when nothing happened.
fn replace_binary(version: &str) -> Res<Applied> {
    match updater(Some(&format!("v{version}")))?.update()? {
        self_update::Status::UpToDate(_) => Ok(Applied::NoChange),
        self_update::Status::Updated(_) => Ok(Applied::Restart),
    }
}

/// Pick the single release asset whose name matches `pred`.
fn pick_asset(release: &Release, what: &str, pred: impl Fn(&str) -> bool) -> Res<ReleaseAsset> {
    release
        .assets
        .iter()
        .find(|a| pred(&a.name.to_ascii_lowercase()))
        .cloned()
        .ok_or_else(|| format!("release v{} has no {what} asset", release.version).into())
}

/// Stream a release asset to `dest`.
///
/// `self_update` records GitHub's *API* asset URL, which serves JSON metadata
/// unless the request explicitly asks for the bytes.
fn download_asset(asset: &ReleaseAsset, dest: &mut std::fs::File) -> Res<()> {
    self_update::Download::from_url(&asset.download_url)
        .set_header(reqwest::header::ACCEPT, "application/octet-stream".parse()?)
        .download_to(dest)?;
    Ok(())
}

/// AppImage: replace the artifact `$APPIMAGE` points at.
///
/// Downloads alongside the target so the final step is a same-filesystem
/// rename: atomic, and it swaps the directory entry rather than truncating the
/// file the kernel still has mounted underneath us.
fn replace_appimage(release: &Release) -> Res<()> {
    let current = appimage_path().ok_or("APPIMAGE is not set")?;
    let dir = current.parent().ok_or("APPIMAGE has no parent directory")?;

    let asset = pick_asset(release, "AppImage", |n| n.ends_with(".appimage"))?;

    let tmp = dir.join(format!(".nybble-{}.AppImage.part", release.version));
    let mut file = std::fs::File::create(&tmp)?;
    let result = download_asset(&asset, &mut file).and_then(|()| {
        file.sync_all()?;
        Ok(())
    });
    drop(file);
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }

    // The download carries no permission bits; without this the replacement
    // AppImage is not executable and the relaunch fails.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }

    if let Err(e) = std::fs::rename(&tmp, &current) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Windows MSI: fetch the installer and hand off to `msiexec`.
///
/// The MSI is deliberately named without a Rust target triple so that
/// `self_update`'s triple matcher can't mistake it for the portable archive —
/// which is why this selects by extension instead.
#[cfg(windows)]
fn run_installer(release: &Release) -> Res<()> {
    let asset = pick_asset(release, "MSI", |n| n.ends_with(".msi"))?;

    let dest = std::env::temp_dir().join(&asset.name);
    let mut file = std::fs::File::create(&dest)?;
    let result = download_asset(&asset, &mut file);
    drop(file);
    result?;

    // The upgrade replaces this very executable, so we can't wait around for
    // msiexec ourselves — the caller exits the moment this returns, freeing our
    // files so Windows doesn't raise a FilesInUse prompt.
    //
    // That leaves nobody to reopen the app afterwards: the MSI's "Launch
    // Nybble" lives on the wizard's exit dialog, and /qb suppresses the wizard.
    // So hand the sequencing to a detached `cmd`, which outlives us, waits for
    // msiexec, and relaunches on success only (`&&`). The path is unchanged by
    // the upgrade, so resolving it now is safe.
    let exe = std::env::current_exe()?;
    let script = format!(
        r#"msiexec /i "{}" /qb && start "" "{}""#,
        dest.display(),
        exe.display()
    );

    let mut cmd = std::process::Command::new("cmd");
    cmd.arg("/C").arg(script);
    {
        // CREATE_NO_WINDOW: without it a console window flashes up behind the
        // installer progress bar.
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.spawn()?;
    Ok(())
}

#[cfg(not(windows))]
fn run_installer(_release: &Release) -> Res<()> {
    Err("installer-based updates are Windows-only".into())
}

/// Relaunch the freshly installed build and exit the current process.
pub fn restart() -> ! {
    let exe = appimage_path().or_else(|| std::env::current_exe().ok());
    if let Some(exe) = exe {
        let _ = std::process::Command::new(exe).spawn();
    }
    std::process::exit(0);
}

/// Exit without relaunching, leaving an external installer to do its work.
pub fn quit() -> ! {
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> ReleaseAsset {
        ReleaseAsset {
            name: name.to_owned(),
            download_url: format!("https://example.invalid/{name}"),
        }
    }

    fn release(assets: &[&str]) -> Release {
        Release {
            name: "v9.9.9".to_owned(),
            version: "9.9.9".to_owned(),
            date: String::new(),
            body: None,
            assets: assets.iter().map(|n| asset(n)).collect(),
        }
    }

    fn path(p: &str) -> Option<PathBuf> {
        Some(PathBuf::from(p))
    }

    #[test]
    fn writable_exe_dir_replaces_the_binary() {
        let s = resolve_strategy(None, path("/home/u/nybble/nybble"), false, |_| true);
        assert_eq!(s, UpdateStrategy::ReplaceBinary);
    }

    #[test]
    fn read_only_exe_dir_is_managed_on_unix() {
        let s = resolve_strategy(None, path("/usr/bin/nybble"), false, |_| false);
        assert_eq!(s, UpdateStrategy::Managed);
    }

    #[test]
    fn read_only_exe_dir_runs_the_installer_on_windows() {
        let s = resolve_strategy(
            None,
            path(r"C:\Program Files\Nybble\nybble.exe"),
            true,
            |_| false,
        );
        assert_eq!(s, UpdateStrategy::RunInstaller);
    }

    /// The ordering that matters: an AppImage's extraction dir looks writable,
    /// so $APPIMAGE has to win over the exe-location probe or we'd try to swap
    /// a binary that is about to evaporate.
    #[test]
    fn appimage_wins_over_a_writable_exe_dir() {
        let s = resolve_strategy(
            path("/home/u/Downloads/Nybble.AppImage"),
            path("/tmp/.mount_abc123/usr/bin/nybble"),
            false,
            |_| true,
        );
        assert_eq!(s, UpdateStrategy::ReplaceAppImage);
    }

    /// An AppImage somewhere the user can't write (say /opt) can't self-replace.
    #[test]
    fn read_only_appimage_is_managed() {
        let s = resolve_strategy(path("/opt/Nybble.AppImage"), None, false, |_| false);
        assert_eq!(s, UpdateStrategy::Managed);
    }

    #[test]
    fn missing_exe_path_does_not_claim_it_can_self_replace() {
        assert_eq!(
            resolve_strategy(None, None, false, |_| true),
            UpdateStrategy::Managed
        );
    }

    #[test]
    fn channel_defaults_to_stable_unless_explicitly_beta() {
        assert_eq!(channel_from_env(None), Channel::Stable);
        assert_eq!(channel_from_env(Some("")), Channel::Stable);
        assert_eq!(channel_from_env(Some("stable")), Channel::Stable);
        // Anything unrecognised must fall back to stable rather than silently
        // opting someone into pre-releases.
        assert_eq!(channel_from_env(Some("1")), Channel::Stable);
        assert_eq!(channel_from_env(Some("true")), Channel::Stable);
        assert_eq!(channel_from_env(Some("betaa")), Channel::Stable);
    }

    #[test]
    fn channel_accepts_beta_case_and_whitespace_insensitively() {
        assert_eq!(channel_from_env(Some("beta")), Channel::Beta);
        assert_eq!(channel_from_env(Some("Beta")), Channel::Beta);
        assert_eq!(channel_from_env(Some("BETA")), Channel::Beta);
        assert_eq!(channel_from_env(Some(" beta ")), Channel::Beta);
    }

    /// The bug this whole channel split exists to prevent: a pre-release ranks
    /// *above* any earlier version, so a stale client comparing versions alone
    /// would happily accept an RC. Only GitHub's own "latest" endpoint keeps it
    /// out, which is why `newer_release_info` uses it for the stable channel
    /// instead of pattern-matching the tag.
    #[test]
    fn a_prerelease_outranks_earlier_stable_versions() {
        let greater = |cur, other| self_update::version::bump_is_greater(cur, other).unwrap();
        assert!(greater("1.4.0", "1.5.0-rc.1"));
        // ...and only ranks below its own final release.
        assert!(!greater("1.5.0", "1.5.0-rc.1"));
    }

    #[test]
    fn only_managed_installs_defer_to_the_browser() {
        assert!(UpdateStrategy::ReplaceBinary.is_self_applying());
        assert!(UpdateStrategy::ReplaceAppImage.is_self_applying());
        assert!(UpdateStrategy::RunInstaller.is_self_applying());
        assert!(!UpdateStrategy::Managed.is_self_applying());
    }

    /// The MSI is deliberately named without a target triple so `self_update`'s
    /// matcher can't grab it; this selects by extension and must not be fooled
    /// by the portable archive sitting in the same release.
    #[test]
    fn picks_the_msi_and_not_the_portable_zip() {
        let r = release(&[
            "nybble-x86_64-pc-windows-msvc.zip",
            "Nybble-9.9.9-x64.msi",
            "nybble_9.9.9_amd64.deb",
        ]);
        let a = pick_asset(&r, "MSI", |n| n.ends_with(".msi")).unwrap();
        assert_eq!(a.name, "Nybble-9.9.9-x64.msi");
    }

    #[test]
    fn picks_the_appimage_case_insensitively() {
        let r = release(&[
            "nybble-x86_64-unknown-linux-gnu.tar.gz",
            "Nybble-9.9.9-x86_64.AppImage",
        ]);
        let a = pick_asset(&r, "AppImage", |n| n.ends_with(".appimage")).unwrap();
        assert_eq!(a.name, "Nybble-9.9.9-x86_64.AppImage");
    }

    #[test]
    fn missing_asset_is_an_error_rather_than_a_wrong_download() {
        let r = release(&["nybble-x86_64-pc-windows-msvc.zip"]);
        assert!(pick_asset(&r, "MSI", |n| n.ends_with(".msi")).is_err());
    }

    #[test]
    fn write_probe_distinguishes_writable_from_missing_dirs() {
        assert!(dir_is_writable(&std::env::temp_dir()));
        assert!(!dir_is_writable(Path::new(
            "/nonexistent-nybble-probe-target"
        )));
    }

    /// The case that actually decides `Managed` on a real `.deb` install: a
    /// directory that exists and is readable but denies writes. Running as root
    /// bypasses permission bits entirely, so skip there rather than assert
    /// something false.
    #[test]
    #[cfg(unix)]
    fn write_probe_rejects_a_read_only_directory() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("nybble-ro-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();

        let writable = dir_is_writable(&dir);

        // Restore write access so the cleanup can succeed.
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();

        let running_as_root = unsafe { libc_geteuid() } == 0;
        if running_as_root {
            return;
        }
        assert!(!writable, "a 0555 directory must not be reported writable");
    }

    #[cfg(unix)]
    unsafe fn libc_geteuid() -> u32 {
        extern "C" {
            fn geteuid() -> u32;
        }
        geteuid()
    }
}
