use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    time::Duration,
};

use eyre::eyre;

use crate::Db;

const FOUR_HOURS_SECS: u64 = 3600 * 4;
const MAX_AGE: Duration = Duration::from_secs(FOUR_HOURS_SECS);

/// Tries to load and parse DB from disk.
///
/// Most errors will be caught, transformed into `None`s and then the cache path will be cleaned up,
/// returning None. The caller is then free to fetch from the web and attempt to cache again.
pub fn cache_fetch() -> Option<Db> {
    match cache_fetch_inner() {
        Ok(cached_db) => cached_db,

        // if any error occurs regarding file access or decoding
        // we try to delete the file to reset state for next time
        Err(err) => {
            eprintln!("cache fetch error: {}", err);

            // attempt clean up
            // errors on this are unlikely and are therefore ignored
            if let Err(err) = fs::remove_file(cache_path()) {
                eprintln!("failed to clean up cache file: {}", err);
            }

            None
        }
    }
}

fn cache_fetch_inner() -> eyre::Result<Option<Db>> {
    let file = fs::File::open(cache_path());

    let file = match file {
        Ok(file) => file,

        // special case for file not found; cache state is clean
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),

        // other errors should be reported so clean up can happen
        Err(err) => return Err(err.into()),
    };

    // check metadata for when file was updated
    let cache_modified = file.metadata()?.created()?;
    let cache_age = cache_modified.elapsed()?;

    // and signal caller to delete cache file
    if cache_age > MAX_AGE {
        return Err(eyre!("cache is too old"));
    }

    let buf = zstd::decode_all(file)?;
    let json = serde_json::from_slice(&buf)?;

    Ok(Some(json))
}

/// Attempt to cache feature database on disk.
///
/// Errors are ignored and a clean up is attempted.
pub fn cache_put(db: &Db) {
    // if any error occurs writing file access or encoding
    // we try to delete the file to reset state for next time
    if let Err(err) = cache_put_inner(db) {
        eprintln!("cache fetch error: {}", err);

        // attempt clean up
        // errors on this are unlikely and are therefore ignored
        if let Err(err) = fs::remove_file(cache_path()) {
            eprintln!("failed to clean up cache file: {}", err);
        }
    }
}

fn cache_put_inner(db: &Db) -> eyre::Result<()> {
    // ensure containing direction of cache file exists
    fs::create_dir_all(cache_dir())?;

    // we need to reset the created datetime
    // since the caching strategy relies on it
    let _ = fs::remove_file(cache_path())?;

    // if create is successful, any existing file is truncated
    let mut file = fs::File::create(cache_path())?;

    let json = serde_json::to_vec_pretty(db)?;
    let enc = zstd::encode_all(&json[..], zstd::DEFAULT_COMPRESSION_LEVEL)?;
    file.write_all(&enc)?;

    Ok(())
}

/// Returns absolute path to location of feature database cache file.
fn cache_path() -> PathBuf {
    cache_dir().join("caniuse.zst")
}

/// Returns absolute path to location of cache directory.
pub(crate) fn cache_dir() -> PathBuf {
    use dirs::cache_dir as macos_cache_dir;

    macos_cache_dir()
        .unwrap()
        .join("dev.robjtede.alfred-caniuse-rs")
}
