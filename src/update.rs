//! Self-update checks.

use std::{
    fs,
    io::{self, Write as _},
    time::Duration,
};

use eyre::eyre;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::cache::cache_dir;

const DAY_IN_SECS: u64 = 3600 * 24;
const LATEST_URL: &str = "https://github.com/robjtede/alfred-caniuse-rs/releases";
const LATEST_ZIP_PATH: &str = "/latest/download/package.zip";
const SELF_VERSION: &str = env!("CARGO_PKG_VERSION");
const UPDATE_CHECK_FILENAME: &str = "update-check.json";

/// Returning None means no action to take.
pub fn self_update_check_item() -> Option<alfred::Item<'static>> {
    self_update_check().map(|url| {
        alfred::ItemBuilder::new("A workflow update is available.")
            .subtitle("Press enter to go to download page.")
            .arg(url)
            .into_item()
    })
}

/// Returning None means no action to take.
fn self_update_check() -> Option<&'static str> {
    match self_need_update_check() {
        // fall through to update check
        Ok(NeedsCheck::Yes) => {
            eprintln!("update check will be perform")
        }

        Ok(NeedsCheck::No) => {
            eprintln!("skip update check");
            return None;
        }

        // cached file shows that self is outdated so skip API lookup
        Ok(NeedsCheck::KnownOutdated) => return Some(LATEST_URL),

        // eg. time::Date changes it's serde format causing json deserialization to fail
        Err(err) => {
            let check_file = cache_dir().join(UPDATE_CHECK_FILENAME);

            eprintln!("update check cache failed: {err}");
            eprintln!("deleting update check file from: {check_file:?}");

            // attempt to clean up any potentially corrupted cache state
            let _ = fs::remove_file(check_file);

            // fall through to update check
        }
    }

    // ignore errors from fetching for cases when no internet connection is available
    match self_update_check_inner() {
        Ok(true) => return Some(LATEST_URL),
        Ok(false) => {
            eprintln!("no update available");
        }
        Err(err) => {
            eprintln!("error fetching update: {err}");
        }
    }

    None
}

#[derive(Debug, Clone)]
enum NeedsCheck {
    Yes,
    No,
    KnownOutdated,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct UpdateCheck {
    /// After a check, store the result.
    update_needed: bool,

    /// Self version that filled in update_needed.
    checked_with: String,

    /// When last API call was made to check for new latest version.
    last_check: time::OffsetDateTime,
}

impl UpdateCheck {
    /// Returns true if API call should be made to check for new version.
    fn remote_check_needed(&self) -> NeedsCheck {
        // immediately after an update versions will not match indicating stale data
        if SELF_VERSION != self.checked_with {
            return NeedsCheck::Yes;
        }

        // if true, it is already known that most self version is not latest so
        // no check is necessary; flag will be reset after updated
        if self.update_needed {
            return NeedsCheck::KnownOutdated;
        }

        let last_check_delta = time::OffsetDateTime::now_utc() - self.last_check;

        // only thing to check is whether check has occurred recently
        if last_check_delta > time::Duration::seconds(DAY_IN_SECS as i64) {
            NeedsCheck::Yes
        } else {
            NeedsCheck::No
        }
    }
}

// Returning errors to signal a clean up of the cache file may be necessary.
fn self_need_update_check() -> eyre::Result<NeedsCheck> {
    let update_check_cache_path = cache_dir().join(UPDATE_CHECK_FILENAME);
    let json = match fs::read(update_check_cache_path) {
        Ok(val) => val,

        // special case when no cache file exists, check is needed
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(NeedsCheck::Yes),

        Err(err) => return Err(err.into()),
    };

    let update_check = serde_json::from_slice::<UpdateCheck>(&json)?;
    Ok(update_check.remote_check_needed())
}

// Makes API call to GitHub to check latest
fn self_update_check_inner() -> eyre::Result<bool> {
    let client = ureq::Agent::config_builder()
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(1)))
        .build()
        .new_agent();

    let url = [LATEST_URL, LATEST_ZIP_PATH].concat();
    let res = client.get(&url).call()?;
    let latest_url = res
        .headers()
        .get("location")
        .ok_or_else(|| eyre!("no location header in update check response"))?
        .to_str()?;

    let update_needed = release_is_newer(latest_url, SELF_VERSION)?;

    // ensure containing direction of cache file exists
    fs::create_dir_all(cache_dir())?;

    let update_check_cache_path = cache_dir().join(UPDATE_CHECK_FILENAME);
    let mut file = fs::File::create(&update_check_cache_path)?;

    let last_check = UpdateCheck {
        update_needed,
        checked_with: SELF_VERSION.to_owned(),
        last_check: time::OffsetDateTime::now_utc(),
    };

    let update_check = serde_json::to_vec_pretty(&last_check)?;
    // TODO: less resilient than other file ops
    file.write_all(&update_check)?;

    eprintln!("checking cache at {:?}", &update_check_cache_path);

    Ok(update_needed)
}

fn release_is_newer(location: &str, current_version: &str) -> eyre::Result<bool> {
    let release_tag = location
        .strip_prefix(LATEST_URL)
        .and_then(|path| path.strip_prefix("/download/"))
        .and_then(|path| path.strip_suffix("/package.zip"))
        .ok_or_else(|| eyre!("unexpected release download URL: {location}"))?;

    let latest_version = Version::parse(release_tag.strip_prefix('v').unwrap_or(release_tag))?;
    let current_version = Version::parse(current_version)?;

    Ok(latest_version.cmp_precedence(&current_version).is_gt())
}

#[cfg(test)]
mod tests {
    use super::release_is_newer;

    #[test]
    fn newer_release_with_shared_version_prefix_is_an_update() {
        assert!(release_is_newer(
            "https://github.com/robjtede/alfred-caniuse-rs/releases/download/v0.4.10/package.zip",
            "0.4.1",
        )
        .unwrap());
    }

    #[test]
    fn matching_release_tag_is_not_an_update() {
        for tag in ["v0.4.1", "0.4.1"] {
            let location = format!(
                "https://github.com/robjtede/alfred-caniuse-rs/releases/download/{tag}/package.zip"
            );

            assert!(!release_is_newer(&location, "0.4.1").unwrap());
        }
    }

    #[test]
    fn release_without_v_prefix_is_an_update() {
        assert!(release_is_newer(
            "https://github.com/robjtede/alfred-caniuse-rs/releases/download/0.4.10/package.zip",
            "0.4.1",
        )
        .unwrap());
    }

    #[test]
    fn older_release_is_not_an_update() {
        assert!(!release_is_newer(
            "https://github.com/robjtede/alfred-caniuse-rs/releases/download/v0.4.1/package.zip",
            "0.4.10",
        )
        .unwrap());
    }

    #[test]
    fn final_release_updates_prerelease() {
        assert!(release_is_newer(
            "https://github.com/robjtede/alfred-caniuse-rs/releases/download/v1.0.0/package.zip",
            "1.0.0-rc.1",
        )
        .unwrap());
    }

    #[test]
    fn prerelease_does_not_replace_final_release() {
        assert!(!release_is_newer(
            "https://github.com/robjtede/alfred-caniuse-rs/releases/download/v1.0.0-rc.1/package.zip",
            "1.0.0",
        )
        .unwrap());
    }

    #[test]
    fn prerelease_numbers_use_numeric_order() {
        assert!(release_is_newer(
            "https://github.com/robjtede/alfred-caniuse-rs/releases/download/v1.0.0-rc.10/package.zip",
            "1.0.0-rc.2",
        )
        .unwrap());
    }

    #[test]
    fn build_metadata_does_not_change_release_precedence() {
        for (tag, current_version) in [
            ("v0.4.1+build.2", "0.4.1+build.1"),
            ("v0.4.1+build.1", "0.4.1+build.2"),
            ("v0.4.1+build.1", "0.4.1"),
        ] {
            let location = format!(
                "https://github.com/robjtede/alfred-caniuse-rs/releases/download/{tag}/package.zip"
            );

            assert!(!release_is_newer(&location, current_version).unwrap());
        }
    }

    #[test]
    fn malformed_or_unexpected_release_urls_are_errors() {
        for location in [
            "https://example.com/package.zip",
            "https://example.com/releases/download/v0.4.10/package.zip",
            "https://github.com/robjtede/other/releases/download/v0.4.10/package.zip",
            "https://example.com/?next=https://github.com/robjtede/alfred-caniuse-rs/releases/download/v0.4.10/package.zip",
            "https://github.com/robjtede/alfred-caniuse-rs/releases/tag/v0.4.10",
            "https://github.com/robjtede/alfred-caniuse-rs/releases/download//package.zip",
            "https://github.com/robjtede/alfred-caniuse-rs/releases/download/vnot-a-version/package.zip",
            "https://github.com/robjtede/alfred-caniuse-rs/releases/download/v0.4.10/other.zip",
            "https://github.com/robjtede/alfred-caniuse-rs/releases/download/v0.4.10/extra/package.zip",
            "https://github.com/robjtede/alfred-caniuse-rs/releases/download/v0.4.10/package.zip?next=/releases/download/v0.4.1/package.zip",
        ] {
            assert!(release_is_newer(location, "0.4.1").is_err(), "{location}");
        }
    }
}
