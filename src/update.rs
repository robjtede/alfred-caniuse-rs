//! Self-update checks.

use std::{fs, io, path::Path, time::Duration};

use eyre::eyre;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::cache::cache_dir;

const DAY_IN_SECS: u64 = 3600 * 24;
const RETRY_IN_SECS: i64 = 5 * 60;
const LATEST_URL: &str = "https://github.com/robjtede/alfred-caniuse-rs/releases";
const LATEST_ZIP_PATH: &str = "/latest/download/package.zip";
const SELF_VERSION: &str = env!("CARGO_PKG_VERSION");
const UPDATE_CHECK_FILENAME: &str = "update-check.json";

/// Returning None means no action to take.
pub fn self_update_check_item() -> Option<alfred::Item<'static>> {
    self_update_check(
        &cache_dir().join(UPDATE_CHECK_FILENAME),
        time::OffsetDateTime::now_utc(),
        SELF_VERSION,
        self_update_check_inner,
    )
    .map(|url| {
        alfred::ItemBuilder::new("A workflow update is available.")
            .subtitle("Press enter to go to download page.")
            .arg(url)
            .into_item()
    })
}

/// Returning None means no action to take.
fn self_update_check(
    check_file: &Path,
    now: time::OffsetDateTime,
    self_version: &str,
    fetch_update: impl FnOnce() -> eyre::Result<bool>,
) -> Option<&'static str> {
    match self_need_update_check(check_file, now, self_version) {
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
            eprintln!("update check cache failed: {err}");
            eprintln!("deleting update check file from: {check_file:?}");

            // attempt to clean up any potentially corrupted cache state
            let _ = fs::remove_file(check_file);

            // fall through to update check
        }
    }

    // ignore errors from fetching for cases when no internet connection is available
    let (update_needed, retry_after) = match fetch_update() {
        Ok(update_needed) => (update_needed, None),
        Err(err) => {
            eprintln!("error fetching update: {err}");

            (false, Some(now + time::Duration::seconds(RETRY_IN_SECS)))
        }
    };

    let last_check = UpdateCheck {
        update_needed,
        checked_with: self_version.to_owned(),
        last_check: now,
        retry_after,
    };

    if let Err(err) = store_update_check(check_file, &last_check) {
        eprintln!("update check cache failed: {err}");

        return None;
    }

    if update_needed {
        return Some(LATEST_URL);
    }

    if retry_after.is_none() {
        eprintln!("no update available");
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

    /// Retry failed checks sooner than successful checks.
    #[serde(default)]
    retry_after: Option<time::OffsetDateTime>,
}

impl UpdateCheck {
    /// Returns true if API call should be made to check for new version.
    fn remote_check_needed(&self, now: time::OffsetDateTime, self_version: &str) -> NeedsCheck {
        // immediately after an update versions will not match indicating stale data
        if self_version != self.checked_with {
            return NeedsCheck::Yes;
        }

        // if true, it is already known that most self version is not latest so
        // no check is necessary; flag will be reset after updated
        if self.update_needed {
            return NeedsCheck::KnownOutdated;
        }

        if let Some(retry_after) = self.retry_after {
            return if now >= retry_after {
                NeedsCheck::Yes
            } else {
                NeedsCheck::No
            };
        }

        let last_check_delta = now - self.last_check;

        // only thing to check is whether check has occurred recently
        if last_check_delta > time::Duration::seconds(DAY_IN_SECS as i64) {
            NeedsCheck::Yes
        } else {
            NeedsCheck::No
        }
    }
}

// Returning errors to signal a clean up of the cache file may be necessary.
fn self_need_update_check(
    check_file: &Path,
    now: time::OffsetDateTime,
    self_version: &str,
) -> eyre::Result<NeedsCheck> {
    let json = match fs::read(check_file) {
        Ok(val) => val,

        // special case when no cache file exists, check is needed
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(NeedsCheck::Yes),

        Err(err) => return Err(err.into()),
    };

    let update_check = serde_json::from_slice::<UpdateCheck>(&json)?;
    Ok(update_check.remote_check_needed(now, self_version))
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

fn store_update_check(check_file: &Path, last_check: &UpdateCheck) -> eyre::Result<()> {
    if let Some(parent) = check_file.parent() {
        fs::create_dir_all(parent)?;
    }

    let update_check = serde_json::to_vec_pretty(last_check)?;
    // TODO: less resilient than other file ops
    fs::write(check_file, update_check)?;

    eprintln!("checking cache at {check_file:?}");

    Ok(())
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

#[cfg(test)]
mod backoff_tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    const NOW: time::OffsetDateTime = time::macros::datetime!(2026-10-17 12:00 UTC);

    struct TestCache(PathBuf);

    impl TestCache {
        fn new() -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

            let dir = std::env::temp_dir().join(format!(
                "alfred-caniuse-update-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed),
            ));

            fs::create_dir(&dir).unwrap();

            Self(dir.join(UPDATE_CHECK_FILENAME))
        }
    }

    impl Drop for TestCache {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(self.0.parent().unwrap());
        }
    }

    #[test]
    fn failed_check_retries_only_after_five_minutes() {
        let cache = TestCache::new();

        assert_eq!(
            self_update_check(&cache.0, NOW, "0.4.1", || Err(eyre!("offline"))),
            None,
        );

        for now in [NOW, NOW + time::Duration::seconds(299)] {
            assert_eq!(
                self_update_check(&cache.0, now, "0.4.1", || panic!("retry too soon")),
                None,
            );
        }

        let retry_at = NOW + time::Duration::minutes(5);

        assert_eq!(
            self_update_check(&cache.0, retry_at, "0.4.1", || Ok(true)),
            Some("https://github.com/robjtede/alfred-caniuse-rs/releases"),
        );
    }

    #[test]
    fn successful_check_is_cached_for_a_day() {
        let cache = TestCache::new();

        assert_eq!(
            self_update_check(&cache.0, NOW, "0.4.1", || Ok(false)),
            None,
        );

        for now in [
            NOW + time::Duration::minutes(5),
            NOW + time::Duration::days(1),
        ] {
            assert_eq!(
                self_update_check(&cache.0, now, "0.4.1", || panic!("check too soon")),
                None,
            );
        }

        assert_eq!(
            self_update_check(
                &cache.0,
                NOW + time::Duration::days(1) + time::Duration::seconds(1),
                "0.4.1",
                || Ok(true),
            ),
            Some("https://github.com/robjtede/alfred-caniuse-rs/releases"),
        );
    }

    #[test]
    fn successful_retry_restores_the_daily_interval() {
        let cache = TestCache::new();

        self_update_check(&cache.0, NOW, "0.4.1", || Err(eyre!("offline")));

        let mut fetched = false;
        let notice = self_update_check(&cache.0, NOW + time::Duration::minutes(5), "0.4.1", || {
            fetched = true;

            Ok(false)
        });

        assert!(fetched, "the failed check must be retried");
        assert_eq!(notice, None);

        assert_eq!(
            self_update_check(&cache.0, NOW + time::Duration::minutes(10), "0.4.1", || {
                panic!("a successful retry must use the daily interval")
            }),
            None,
        );
    }

    #[test]
    fn cache_without_retry_deadline_keeps_the_daily_interval() {
        let cache = TestCache::new();
        let legacy_cache = serde_json::json!({
            "update_needed": false,
            "checked_with": "0.4.1",
            "last_check": NOW,
        });

        fs::write(&cache.0, serde_json::to_vec(&legacy_cache).unwrap()).unwrap();

        assert_eq!(
            self_update_check(&cache.0, NOW + time::Duration::hours(23), "0.4.1", || {
                panic!("the existing cache must remain valid")
            }),
            None,
        );
    }

    #[test]
    fn known_outdated_version_keeps_the_update_notice_without_fetching() {
        let cache = TestCache::new();

        assert!(self_update_check(&cache.0, NOW, "0.4.1", || Ok(true)).is_some());

        assert_eq!(
            self_update_check(&cache.0, NOW + time::Duration::days(2), "0.4.1", || {
                panic!("known update does not need another fetch")
            }),
            Some("https://github.com/robjtede/alfred-caniuse-rs/releases"),
        );
    }

    #[test]
    fn changed_version_rechecks_a_cached_success_or_outdated_notice() {
        for update_needed in [false, true] {
            let cache = TestCache::new();

            self_update_check(&cache.0, NOW, "0.4.1", || Ok(update_needed));

            let mut fetched = false;
            let notice = self_update_check(&cache.0, NOW, "0.4.2", || {
                fetched = true;

                Ok(false)
            });

            assert!(fetched, "a changed version must be checked");
            assert_eq!(notice, None);
        }
    }

    #[test]
    fn changed_version_rechecks_during_failure_backoff() {
        let cache = TestCache::new();

        self_update_check(&cache.0, NOW, "0.4.1", || Err(eyre!("offline")));

        assert_eq!(
            self_update_check(&cache.0, NOW, "0.4.2", || Ok(true)),
            Some("https://github.com/robjtede/alfred-caniuse-rs/releases"),
        );
    }
}
