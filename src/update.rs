//! Self-update checks.

use std::{
    fs,
    io::{self, Write},
};

use eyre::eyre;
use serde::{Deserialize, Serialize};

use crate::cache::cache_dir;

const DAY_IN_SECS: u64 = 3600 * 24;
const LATEST_ZIP_URL: &str =
    "https://github.com/robjtede/alfred-caniuse-rs/releases/latest/download/package.zip";
const SELF_VERSION: &str = env!("CARGO_PKG_VERSION");
const UPDATE_CHECK_FILENAME: &str = "update-check.json";

/// Returning None means no action to take.
pub fn self_update_check_item() -> Option<alfred::Item<'static>> {
    self_update_check().map(|url| {
        alfred::ItemBuilder::new("A workflow update is available.")
            .subtitle("Press enter to update.")
            .arg(url)
            .into_item()
    })
}

/// Returning None means no action to take.
fn self_update_check() -> Option<&'static str> {
    match self_need_update_check() {
        // fall through to update check
        Ok(NeedsCheck::Yes) => {}

        Ok(NeedsCheck::No) => return None,

        // cached file shows that self is outdated so skip API lookup
        Ok(NeedsCheck::KnownOutdated) => return Some(LATEST_ZIP_URL),

        Err(err) => {
            eprintln!("update check cache failed: {}", err);

            // attempt to clean up any potentially corrupted cache state
            let _ = fs::remove_file(UPDATE_CHECK_FILENAME);

            return None;
        }
    }

    // ignore errors from fetching for cases when no internet connection is available
    match self_update_check_inner() {
        Ok(true) => return Some(LATEST_ZIP_URL),
        Ok(false) => {
            eprintln!("no update available");
        }
        Err(err) => {
            eprintln!("error fetching update: {}", err);
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
    let client = ureq::builder()
        .redirects(0)
        .timeout(std::time::Duration::from_secs(1))
        .build();

    let res = client.get(LATEST_ZIP_URL).call()?;
    let latest_url = res
        .header("location")
        .ok_or_else(|| eyre!("no location header in update check response"))?;

    // ensure containing direction of cache file exists
    fs::create_dir_all(cache_dir())?;

    let update_check_cache_path = cache_dir().join(UPDATE_CHECK_FILENAME);
    let mut file = fs::File::create(&update_check_cache_path)?;

    // for some download URL like:
    // update-server.com/release/v1.2.3/download
    // it should only be required that the current version exists somewhere in that URL
    // to be considered the latest to avoid needing regex and oddities with v* prefixes
    let update_needed = !latest_url.contains(SELF_VERSION);

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
