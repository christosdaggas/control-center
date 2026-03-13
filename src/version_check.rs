//! GitHub release version check.
//!
//! On startup, makes a single HTTP request to the GitHub Releases API
//! to determine whether a newer version is available. All errors are
//! silently swallowed — the check never interrupts the user.

use serde::Deserialize;
use tracing::{debug, warn};

/// GitHub repository owner.
const GITHUB_OWNER: &str = "christosdaggas";

/// GitHub repository name.
const GITHUB_REPO: &str = "control-center";

/// Result of a successful version check.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// Latest version string (e.g. "1.2.0").
    pub latest_version: String,
    /// URL the user can visit to download the release.
    pub download_url: String,
    /// Release name / title (may be empty).
    pub release_name: String,
}

/// Subset of the GitHub Releases API response we care about.
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    name: Option<String>,
}

/// Check GitHub for the latest release (blocking).
///
/// Returns `Some(UpdateInfo)` if a newer version exists,
/// `None` if the local version is current **or on any error**.
///
/// This function performs a blocking HTTP request and should be
/// called from a background thread, never from the GTK main loop.
pub fn check_for_update(current_version: &str) -> Option<UpdateInfo> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        GITHUB_OWNER, GITHUB_REPO
    );

    debug!("Checking for updates at {}", url);


    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(format!("{}/{}", GITHUB_REPO, current_version))
        .build()
        .ok()?;

    let response = match client.get(&url).send() {
        Ok(resp) => resp,
        Err(e) => {
            debug!("Update check HTTP request failed (not an error): {}", e);
            return None;
        }
    };

    if !response.status().is_success() {
        debug!(
            "Update check got HTTP {} (repo may not have releases yet)",
            response.status()
        );
        return None;
    }

    let release: GitHubRelease = match response.json() {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to parse GitHub release JSON: {}", e);
            return None;
        }
    };


    let latest = release
        .tag_name
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string();

    debug!(
        "Update check: local={}, remote={}",
        current_version, latest
    );

    if is_newer(&latest, current_version) {
        Some(UpdateInfo {
            latest_version: latest,
            download_url: release.html_url,
            release_name: release.name.unwrap_or_default(),
        })
    } else {
        debug!("Application is up to date");
        None
    }
}

/// Compare two semver-ish version strings.
/// Returns `true` if `remote` is strictly newer than `local`.
fn is_newer(remote: &str, local: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect()
    };

    let r = parse(remote);
    let l = parse(local);

    let max_len = r.len().max(l.len());
    for i in 0..max_len {
        let rv = r.get(i).copied().unwrap_or(0);
        let lv = l.get(i).copied().unwrap_or(0);
        if rv > lv {
            return true;
        }
        if rv < lv {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_patch() {
        assert!(is_newer("1.0.1", "1.0.0"));
    }

    #[test]
    fn test_is_newer_minor() {
        assert!(is_newer("1.1.0", "1.0.0"));
    }

    #[test]
    fn test_is_newer_major() {
        assert!(is_newer("2.0.0", "1.9.9"));
    }

    #[test]
    fn test_same_version() {
        assert!(!is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn test_older_version() {
        assert!(!is_newer("0.9.0", "1.0.0"));
    }

    #[test]
    fn test_longer_remote() {
        assert!(is_newer("1.0.0.1", "1.0.0"));
    }

    #[test]
    fn test_longer_local() {
        assert!(!is_newer("1.0.0", "1.0.0.1"));
    }

    #[test]
    fn test_single_component() {
        assert!(is_newer("2", "1"));
        assert!(!is_newer("1", "2"));
    }
}
