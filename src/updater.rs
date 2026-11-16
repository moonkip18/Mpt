//! In-app updates against the repo's GitHub Releases.
//!
//! One small button in the subject-panel footer drives the whole flow:
//! check (GitHub API) → download (the bare `Mpt.exe` asset) → self-swap.
//! The swap exploits the one thing Windows allows on a running exe — a
//! rename — so nothing but the app itself is involved: no browser, no
//! second installer. Everything slow runs on a worker thread; the UI only
//! reads a mutex-protected state value.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const RELEASES_API: &str = "https://api.github.com/repos/moonkip18/Mpt/releases/latest";
const EXE_ASSET: &str = "Mpt.exe";

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    /// Nothing checked yet.
    Idle,
    Checking,
    /// The latest release is not newer than this build.
    UpToDate,
    /// A newer release is published; `url` points at its bare exe asset.
    Available {
        version: String,
        url: String,
        size: u64,
    },
    Downloading { progress: f32 },
    /// Downloaded and staged next to the exe; a click restarts into it.
    Ready,
    Failed(String),
}

/// Where the worker stands for the UI to poll each frame.
pub type UpdateShared = Arc<Mutex<UpdateState>>;

/// Compare "v1.2.3" against the running build's Cargo version.
fn parse_version(s: &str) -> Vec<u64> {
    s.trim()
        .trim_start_matches('v')
        .split('.')
        .map(|p| p.trim().parse::<u64>().unwrap_or(0))
        .collect()
}

fn is_newer(release: &str, current: &str) -> bool {
    let a = parse_version(release);
    let b = parse_version(current);
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

/// Paths used by download and swap, both derived from the running exe.
fn staging_paths() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    Some((
        exe,
        dir.join("mpt.exe.new"),
        dir.join("mpt.exe.old"),
    ))
}

/// Spawn the check thread. Never blocks the UI; failures surface on the
/// control (used by the retry click).
pub fn check(state: UpdateShared) {
    spawn_check(state, false)
}

/// Startup probe: a newer release lights the control up; anything else —
/// including a failed request — stays invisible.
pub fn check_quiet(state: UpdateShared) {
    spawn_check(state, true)
}

fn spawn_check(state: UpdateShared, quiet: bool) {
    std::thread::spawn(move || {
        set(&state, UpdateState::Checking);
        match check_blocking() {
            Ok(Some((version, url, size))) => set(
                &state,
                UpdateState::Available { version, url, size },
            ),
            Ok(None) => set(&state, UpdateState::UpToDate),
            Err(e) => set(
                &state,
                if quiet {
                    UpdateState::Idle
                } else {
                    UpdateState::Failed(e)
                },
            ),
        }
    });
}

fn check_blocking() -> Result<Option<(String, String, u64)>, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("mpt-updater")
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let release: serde_json::Value = client
        .get(RELEASES_API)
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .map_err(|e| e.to_string())?;

    let tag = release["tag_name"].as_str().unwrap_or_default().to_string();
    if tag.is_empty() {
        return Err("release had no tag".into());
    }
    if !is_newer(&tag, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    for asset in release["assets"].as_array().into_iter().flatten() {
        if asset["name"].as_str() == Some(EXE_ASSET) {
            let url = asset["browser_download_url"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            if !url.is_empty() {
                let size = asset["size"].as_u64().unwrap_or(0);
                return Ok(Some((tag, url, size)));
            }
        }
    }
    Err("latest release has no Mpt.exe asset".into())
}

/// Spawn the download thread: streams the new exe to `mpt.exe.new` with
/// progress published as a 0..1 float.
pub fn download(state: UpdateShared, url: String) {
    std::thread::spawn(move || {
        let Some((_, new_path, _)) = staging_paths() else {
            set(&state, UpdateState::Failed("could not locate the exe".into()));
            return;
        };
        set(&state, UpdateState::Downloading { progress: 0.0 });
        match download_blocking(&url, &new_path, &state) {
            Ok(()) => set(&state, UpdateState::Ready),
            Err(e) => {
                let _ = std::fs::remove_file(&new_path);
                set(&state, UpdateState::Failed(e));
            }
        }
    });
}

fn download_blocking(url: &str, dest: &std::path::Path, state: &UpdateShared) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("mpt-updater")
        .connect_timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let mut resp = client
        .get(url)
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let total = resp.content_length().unwrap_or(0);

    let mut file = std::fs::File::create(dest)
        .map_err(|e| format!("could not write next to the exe: {e}"))?;
    let mut done: u64 = 0;
    use std::io::{Read, Write};
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = resp.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        let progress = if total > 0 { done as f32 / total as f32 } else { 0.0 };
        set(state, UpdateState::Downloading { progress });
    }
    file.flush().ok();
    Ok(())
}

/// Stage the swap and restart. Windows refuses to delete or overwrite a
/// running exe but allows renaming it: current → `.old`, staged → current,
/// spawn the new exe, exit. The fresh instance clears `.old` on start.
pub fn apply(state: UpdateShared) {
    let Some((exe, new_path, old_path)) = staging_paths() else {
        return;
    };
    if !new_path.exists() {
        set(&state, UpdateState::Failed("staged update is gone".into()));
        return;
    }
    let _ = std::fs::remove_file(&old_path);
    if std::fs::rename(&exe, &old_path).is_err() {
        set(&state, UpdateState::Failed("could not move the running exe".into()));
        return;
    }
    if std::fs::rename(&new_path, &exe).is_err() {
        // Put the running exe back rather than leave it renamed.
        let _ = std::fs::rename(&old_path, &exe);
        set(&state, UpdateState::Failed("could not place the new exe".into()));
        return;
    }
    let _ = std::process::Command::new(&exe).spawn();
    std::process::exit(0);
}

/// Best-effort cleanup of the previous version's exe, retried briefly
/// because the old process may still hold the file for a moment.
pub fn cleanup_old() {
    let Some((_, _, old_path)) = staging_paths() else {
        return;
    };
    if !old_path.exists() {
        return;
    }
    for _ in 0..10 {
        if std::fs::remove_file(&old_path).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

fn set(state: &UpdateShared, s: UpdateState) {
    *state.lock().unwrap() = s;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison() {
        assert!(is_newer("v1.0.7", "1.0.6"));
        assert!(is_newer("1.1.0", "1.0.99"));
        assert!(is_newer("v2.0", "1.9.9"));
        assert!(!is_newer("v1.0.6", "1.0.6"));
        assert!(!is_newer("v1.0.5", "1.0.6"));
        assert!(!is_newer("", "1.0.6"));
    }
}
