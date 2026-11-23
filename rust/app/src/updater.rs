//! Dual-channel automatic updater for uWidgets (Rust Edition).
//!
//! 1. **Microsoft Store (MSIX packaged)** channel:
//!    - Detected natively via `GetCurrentPackageFullName`.
//!    - Queries `Windows.Services.Store.StoreContext` for pending Store package updates and
//!      silently downloads & installs them via `TrySilentDownloadAndInstallStorePackageUpdatesAsync`
//!      (with graceful fallback to Store UI / App Installer / release feeds for sideloaded MSIX).
//! 2. **Direct Release (GitHub Releases + NeosparkX Website)** channel:
//!    - Checks both the NeosparkX JSON release manifest (`neosparkx.com/releases/uwidgets/latest.json`)
//!      and the GitHub Releases API (`nabil24024004/uWidgets-Rust-Rebuild/releases/latest`), picking
//!      the highest SemVer strictly newer than the running version.
//!    - Automatically downloads, verifies PE header + optional SHA-256 checksum, installs in place
//!      (silent Setup.exe handoff for installed builds, atomic rename-and-swap for portable EXEs),
//!      and relaunches cleanly.

use std::fs;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::time::Duration;

use serde::Deserialize;
use windows::Foundation::Collections::IIterable;
use windows::Services::Store::{StoreContext, StorePackageUpdate, StorePackageUpdateState};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
use windows_core::{Interface, PWSTR};

use crate::models::UpdateConfig;
use crate::platform::system::open_url;

pub const GITHUB_API_LATEST: &str =
    "https://api.github.com/repos/nabil24024004/uWidgets-Rust-Rebuild/releases/latest";
pub const GITHUB_WEB_LATEST: &str =
    "https://github.com/nabil24024004/uWidgets-Rust-Rebuild/releases/latest";
pub const DEFAULT_STORE_PRODUCT_ID: &str = "9NDGKTPBF48G";

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallChannel {
    MicrosoftStore,
    DirectSetup,
    DirectPortable,
}

impl InstallChannel {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::MicrosoftStore => "Microsoft Store (MSIX)",
            Self::DirectSetup => "Direct Install (GitHub / NeosparkX)",
            Self::DirectPortable => "Portable Build (GitHub / NeosparkX)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateSource {
    MicrosoftStore,
    GitHubReleases,
    NeosparkX,
}

impl UpdateSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::MicrosoftStore => "Microsoft Store",
            Self::GitHubReleases => "GitHub Releases",
            Self::NeosparkX => "NeosparkX",
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub source: UpdateSource,
    pub download_url: Option<String>,
    pub release_page: String,
    pub sha256: Option<String>,
    pub store_product_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub enum UpdateCheckOutcome {
    UpToDate,
    StoreUpdatedSilently,
    DirectUpdatedAndRelaunching(String),
    Available(UpdateInfo),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyResult {
    Relaunching,
    StoreInstalled,
    OpenedExternal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim().trim_start_matches(['v', 'V']);
        let core = trimmed.split(['-', '+']).next()?.trim();
        let mut parts = core.split('.');
        let major = parts.next()?.parse::<u32>().ok()?;
        let minor = parts.next().unwrap_or("0").parse::<u32>().ok()?;
        let patch = parts.next().unwrap_or("0").parse::<u32>().ok()?;
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

/// Returns `true` when running inside an MSIX / Microsoft Store AppX package identity.
pub fn is_packaged_msix() -> bool {
    let mut len: u32 = 0;
    let err = unsafe { GetCurrentPackageFullName(&mut len, PWSTR::null()) };
    err == ERROR_INSUFFICIENT_BUFFER || err == ERROR_SUCCESS
}

pub fn detect_install_channel() -> InstallChannel {
    if is_packaged_msix() {
        return InstallChannel::MicrosoftStore;
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
        && dir.join("uWidgets-Uninstall.exe").exists()
    {
        return InstallChannel::DirectSetup;
    }
    InstallChannel::DirectPortable
}

/// Removes leftover `uwidgets-rs.exe.old` after a portable self-update relaunch.
pub fn cleanup_old_portable_binary() {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let old = dir.join("uwidgets-rs.exe.old");
        if old.exists() {
            let _ = fs::remove_file(old);
        }
    }
}

fn build_agent(read_secs: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(6))
        .timeout_read(Duration::from_secs(read_secs))
        .timeout_write(Duration::from_secs(6))
        .user_agent("uWidgets-rs/1.0.0 (+https://github.com/nabil24024004/uWidgets-Rust-Rebuild)")
        .build()
}

// --- Channel A: Native Microsoft Store (WinRT StoreContext) ---

enum StoreCheckResult {
    NoUpdates,
    InstalledSilently,
    UpdatePending,
    Unavailable,
}

fn check_microsoft_store_native(auto_install: bool) -> StoreCheckResult {
    let ro_ok = unsafe { RoInitialize(RO_INIT_MULTITHREADED).is_ok() };
    let res = (|| -> windows_core::Result<StoreCheckResult> {
        let ctx = StoreContext::GetDefault()?;
        let updates = ctx.GetAppAndOptionalStorePackageUpdatesAsync()?.get()?;
        if updates.Size()? == 0 {
            return Ok(StoreCheckResult::NoUpdates);
        }
        if auto_install
            && ctx
                .CanSilentlyDownloadStorePackageUpdates()
                .unwrap_or(false)
        {
            let iterable: IIterable<StorePackageUpdate> = updates.cast()?;
            let dl = ctx
                .TrySilentDownloadAndInstallStorePackageUpdatesAsync(&iterable)?
                .get()?;
            if dl.OverallState()? == StorePackageUpdateState::Completed {
                return Ok(StoreCheckResult::InstalledSilently);
            }
        }
        Ok(StoreCheckResult::UpdatePending)
    })();
    if ro_ok {
        unsafe { RoUninitialize() };
    }
    res.unwrap_or(StoreCheckResult::Unavailable)
}

fn trigger_microsoft_store_install(store_product_id: Option<&str>) -> Result<ApplyResult, String> {
    let ro_ok = unsafe { RoInitialize(RO_INIT_MULTITHREADED).is_ok() };
    let installed = (|| -> windows_core::Result<bool> {
        let ctx = StoreContext::GetDefault()?;
        let updates = ctx.GetAppAndOptionalStorePackageUpdatesAsync()?.get()?;
        if updates.Size()? == 0 {
            return Ok(false);
        }
        let iterable: IIterable<StorePackageUpdate> = updates.cast()?;
        if ctx
            .CanSilentlyDownloadStorePackageUpdates()
            .unwrap_or(false)
        {
            let dl = ctx
                .TrySilentDownloadAndInstallStorePackageUpdatesAsync(&iterable)?
                .get()?;
            if dl.OverallState()? == StorePackageUpdateState::Completed {
                return Ok(true);
            }
        }
        let dl = ctx
            .RequestDownloadAndInstallStorePackageUpdatesAsync(&iterable)?
            .get()?;
        Ok(dl.OverallState()? == StorePackageUpdateState::Completed)
    })();
    if ro_ok {
        unsafe { RoUninitialize() };
    }
    if matches!(installed, Ok(true)) {
        return Ok(ApplyResult::StoreInstalled);
    }
    let store_uri = match store_product_id {
        Some(id) if !id.trim().is_empty() => {
            format!("ms-windows-store://pdp/?ProductId={}", id.trim())
        }
        _ => format!("ms-windows-store://pdp/?ProductId={DEFAULT_STORE_PRODUCT_ID}"),
    };
    open_url(&store_uri);
    Ok(ApplyResult::OpenedExternal)
}

// --- Channel B: GitHub Releases + NeosparkX Website Dual Feed Checker ---

#[derive(Debug, Deserialize)]
struct NeosparkxManifest {
    version: String,
    #[serde(default)]
    setup_url: Option<String>,
    #[serde(default)]
    portable_url: Option<String>,
    #[serde(default)]
    msix_url: Option<String>,
    #[serde(default)]
    release_page: Option<String>,
    #[serde(default)]
    sha256_setup: Option<String>,
    #[serde(default)]
    sha256_portable: Option<String>,
    #[serde(default)]
    store_product_id: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

fn fetch_neosparkx_release(
    agent: &ureq::Agent,
    manifest_url: &str,
    channel: InstallChannel,
) -> Option<(SemVer, UpdateInfo)> {
    let url = manifest_url.trim();
    if url.is_empty() {
        return None;
    }
    let resp = agent.get(url).call().ok()?;
    let m: NeosparkxManifest = resp.into_json().ok()?;
    let sv = SemVer::parse(&m.version)?;
    let (download_url, sha256) = match channel {
        InstallChannel::DirectSetup => (m.setup_url.or(m.portable_url), m.sha256_setup),
        InstallChannel::DirectPortable => (m.portable_url.or(m.setup_url), m.sha256_portable),
        InstallChannel::MicrosoftStore => (m.msix_url, None),
    };
    Some((
        sv,
        UpdateInfo {
            version: m.version.trim_start_matches(['v', 'V']).to_string(),
            source: UpdateSource::NeosparkX,
            download_url,
            release_page: m
                .release_page
                .unwrap_or_else(|| "https://neosparkx.com".to_string()),
            sha256,
            store_product_id: m.store_product_id,
            notes: m.notes,
        },
    ))
}

fn fetch_github_release(
    agent: &ureq::Agent,
    channel: InstallChannel,
) -> Option<(SemVer, UpdateInfo)> {
    if let Ok(resp) = agent
        .get(GITHUB_API_LATEST)
        .set("Accept", "application/vnd.github+json")
        .call()
        && let Ok(rel) = resp.into_json::<GitHubRelease>()
        && let Some(sv) = SemVer::parse(&rel.tag_name)
    {
        let mut setup_url = None;
        let mut portable_url = None;
        let mut msix_url = None;
        for a in &rel.assets {
            let lower = a.name.to_ascii_lowercase();
            if lower.ends_with(".exe") && lower.contains("setup") {
                setup_url = Some(a.browser_download_url.clone());
            } else if lower.ends_with(".exe") {
                portable_url = Some(a.browser_download_url.clone());
            } else if lower.ends_with(".msix") || lower.ends_with(".appinstaller") {
                msix_url = Some(a.browser_download_url.clone());
            }
        }
        let download_url = match channel {
            InstallChannel::DirectSetup => setup_url.or(portable_url),
            InstallChannel::DirectPortable => portable_url.or(setup_url),
            InstallChannel::MicrosoftStore => msix_url,
        };
        return Some((
            sv,
            UpdateInfo {
                version: rel.tag_name.trim_start_matches(['v', 'V']).to_string(),
                source: UpdateSource::GitHubReleases,
                download_url,
                release_page: rel.html_url,
                sha256: None,
                store_product_id: None,
                notes: rel.body,
            },
        ));
    }

    // Lightweight HTML redirect fallback if GitHub API rate-limits anonymous IP
    let resp = agent.get(GITHUB_WEB_LATEST).call().ok()?;
    let final_url = resp.get_url().to_string();
    let tag = final_url.rsplit('/').next()?.trim_start_matches(['v', 'V']);
    let sv = SemVer::parse(tag)?;
    Some((
        sv,
        UpdateInfo {
            version: tag.to_string(),
            source: UpdateSource::GitHubReleases,
            download_url: None,
            release_page: final_url,
            sha256: None,
            store_product_id: None,
            notes: None,
        },
    ))
}

/// Unified entry point for both background auto-checks and manual "Check Now" clicks.
pub fn check_for_updates(
    current_version: &str,
    cfg: &UpdateConfig,
    ignore_update: Option<&str>,
    allow_auto_install: bool,
    manual: bool,
) -> UpdateCheckOutcome {
    let channel = detect_install_channel();
    let current_sv = SemVer::parse(current_version).unwrap_or(SemVer {
        major: 1,
        minor: 0,
        patch: 0,
    });

    // 1. Microsoft Store / MSIX channel check first
    if channel == InstallChannel::MicrosoftStore {
        match check_microsoft_store_native(allow_auto_install && cfg.auto_install) {
            StoreCheckResult::NoUpdates => return UpdateCheckOutcome::UpToDate,
            StoreCheckResult::InstalledSilently => return UpdateCheckOutcome::StoreUpdatedSilently,
            StoreCheckResult::UpdatePending => {
                return UpdateCheckOutcome::Available(UpdateInfo {
                    version: "Store Update".to_string(),
                    source: UpdateSource::MicrosoftStore,
                    download_url: None,
                    release_page: cfg
                        .store_product_id
                        .as_deref()
                        .map(|id| format!("ms-windows-store://pdp/?ProductId={id}"))
                        .unwrap_or_else(|| "ms-windows-store://downloadsandupdates".to_string()),
                    sha256: None,
                    store_product_id: cfg.store_product_id.clone(),
                    notes: Some("A new Microsoft Store update is ready to install.".to_string()),
                });
            }
            StoreCheckResult::Unavailable => {
                // Sideloaded MSIX or offline Store service -> fall through to GitHub + NeosparkX feeds!
            }
        }
    }

    // 2. Query both GitHub Releases and NeosparkX website manifest, pick highest newer SemVer
    let agent = build_agent(8);
    let cand_neo = fetch_neosparkx_release(&agent, &cfg.neosparkx_manifest_url, channel);
    let cand_gh = fetch_github_release(&agent, channel);

    let best = match (cand_neo, cand_gh) {
        (Some(a), Some(b)) => {
            if a.0 >= b.0 {
                Some(a)
            } else {
                Some(b)
            }
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };

    let Some((new_sv, mut info)) = best else {
        return if manual {
            UpdateCheckOutcome::Error("No release server reachable".to_string())
        } else {
            UpdateCheckOutcome::UpToDate
        };
    };

    if new_sv <= current_sv {
        return UpdateCheckOutcome::UpToDate;
    }
    if !manual
        && let Some(ignored) = ignore_update
        && ignored.trim_start_matches(['v', 'V']) == info.version
    {
        return UpdateCheckOutcome::UpToDate;
    }
    if info.store_product_id.is_none() {
        info.store_product_id = cfg.store_product_id.clone();
    }

    // If auto-install is enabled for direct releases and we have an installer/portable URL, apply seamlessly!
    if allow_auto_install && cfg.auto_install && info.download_url.is_some() {
        match apply_update_sync(&info, channel) {
            Ok(ApplyResult::Relaunching) => {
                return UpdateCheckOutcome::DirectUpdatedAndRelaunching(info.version);
            }
            Ok(ApplyResult::StoreInstalled) => {
                return UpdateCheckOutcome::StoreUpdatedSilently;
            }
            Ok(ApplyResult::OpenedExternal) | Err(_) => {}
        }
    }

    UpdateCheckOutcome::Available(info)
}

/// Downloads and applies an update according to the active install channel.
pub fn apply_update_sync(
    info: &UpdateInfo,
    channel: InstallChannel,
) -> Result<ApplyResult, String> {
    if info.source == UpdateSource::MicrosoftStore {
        return trigger_microsoft_store_install(info.store_product_id.as_deref());
    }

    let Some(url) = info.download_url.as_deref() else {
        open_url(&info.release_page);
        return Ok(ApplyResult::OpenedExternal);
    };

    // Sideloaded MSIX update via Windows built-in App Installer protocol
    if channel == InstallChannel::MicrosoftStore {
        let uri = format!("ms-appinstaller:?source={url}");
        open_url(&uri);
        return Ok(ApplyResult::OpenedExternal);
    }

    // Download binary payload (max 40 MB safety cap)
    let agent = build_agent(45);
    let resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("Download failed: {e}"))?;
    let mut bytes = Vec::with_capacity(4 * 1024 * 1024);
    resp.into_reader()
        .take(40 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Read stream failed: {e}"))?;

    if bytes.len() < 150_000 || &bytes[0..2] != b"MZ" {
        return Err("Downloaded file is not a valid Windows executable".to_string());
    }
    if let Some(expected) = info.sha256.as_deref() {
        let actual = sha256_hex(&bytes);
        if !actual.eq_ignore_ascii_case(expected.trim()) {
            return Err("SHA-256 verification failed for downloaded update".to_string());
        }
    }

    let cur_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let cur_dir = cur_exe
        .parent()
        .ok_or_else(|| "Missing executable directory".to_string())?
        .to_path_buf();

    if channel == InstallChannel::DirectSetup || url.to_ascii_lowercase().contains("setup") {
        let tmp_setup =
            std::env::temp_dir().join(format!("uWidgets-Update-v{}-Setup.exe", info.version));
        fs::write(&tmp_setup, &bytes).map_err(|e| format!("Write setup failed: {e}"))?;

        // Run silent Setup into current install dir, then relaunch updated uwidgets-rs.exe cleanly!
        let cmd_line = format!(
            "\"{}\" --dir=\"{}\" /S && start \"\" \"{}\" --restarted",
            tmp_setup.display(),
            cur_dir.display(),
            cur_exe.display()
        );
        Command::new("cmd")
            .args(["/C", &cmd_line])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Failed to launch updater: {e}"))?;
        Ok(ApplyResult::Relaunching)
    } else {
        // Atomic Windows portable self-swap: Windows permits renaming a running .exe!
        let new_path = cur_dir.join("uwidgets-rs.exe.new");
        let old_path = cur_dir.join("uwidgets-rs.exe.old");
        fs::write(&new_path, &bytes).map_err(|e| format!("Write new binary failed: {e}"))?;
        let _ = fs::remove_file(&old_path);
        fs::rename(&cur_exe, &old_path).map_err(|e| format!("Stage old binary failed: {e}"))?;
        if let Err(e) = fs::rename(&new_path, &cur_exe) {
            let _ = fs::rename(&old_path, &cur_exe);
            return Err(format!("Swap new binary failed: {e}"));
        }
        Command::new(&cur_exe)
            .arg("--restarted")
            .current_dir(&cur_dir)
            .spawn()
            .map_err(|e| format!("Relaunch failed: {e}"))?;
        Ok(ApplyResult::Relaunching)
    }
}

/// Zero-dependency FIPS-180-4 SHA-256 hex digest for update payload integrity verification.
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = Vec::with_capacity(data.len() + 72);
    msg.extend_from_slice(data);
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    let mut w = [0u32; 64];
    for chunk in msg.as_chunks::<64>().0 {
        for (i, b) in chunk.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*b);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = String::with_capacity(64);
    for word in h {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{word:08x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_ordering_handles_prefixed_and_double_digits() {
        let v100 = SemVer::parse("v1.0.0").unwrap();
        let v101 = SemVer::parse("1.0.1").unwrap();
        let v120 = SemVer::parse("v1.2.0").unwrap();
        let v1100 = SemVer::parse("1.10.0-beta.1").unwrap();
        assert!(v101 > v100);
        assert!(v120 > v101);
        assert!(v1100 > v120);
        assert_eq!(SemVer::parse("1.0"), Some(v100));
        assert!(SemVer::parse("invalid").is_none());
    }

    #[test]
    fn sha256_matches_nist_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn unpackaged_test_runner_detects_direct_channel() {
        assert!(!is_packaged_msix());
        assert_eq!(detect_install_channel(), InstallChannel::DirectPortable);
    }
}
