use std::{
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
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
    cache_fetch_at(&cache_path())
}

fn cache_fetch_at(path: &Path) -> Option<Db> {
    match cache_fetch_inner(path) {
        Ok(cached_db) => cached_db,

        // if any error occurs regarding file access or decoding
        // we try to delete the file to reset state for next time
        Err(err) => {
            eprintln!("cache fetch error: {err}");

            // attempt clean up
            // errors on this are unlikely and are therefore ignored
            if let Err(err) = fs::remove_file(path) {
                eprintln!("failed to clean up cache file: {err}");
            }

            None
        }
    }
}

fn cache_fetch_inner(path: &Path) -> eyre::Result<Option<Db>> {
    let file = fs::File::open(path);

    let file = match file {
        Ok(file) => file,

        // special case for file not found; cache state is clean
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),

        // other errors should be reported so clean up can happen
        Err(err) => return Err(err.into()),
    };

    // macOS can preserve the destination's creation time when a file is replaced.
    let cache_modified = file.metadata()?.modified()?;
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
/// If a write fails, the previous cache is kept.
pub fn cache_put(db: &Db) {
    cache_put_at(db, &cache_path());
}

fn cache_put_at(db: &impl serde::Serialize, path: &Path) {
    if let Err(err) = cache_put_inner(db, path) {
        eprintln!("cache write error: {err}");
    }
}

fn cache_put_inner(db: &impl serde::Serialize, path: &Path) -> eyre::Result<()> {
    static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

    let dir = path
        .parent()
        .ok_or_else(|| eyre!("cache path has no parent"))?;
    fs::create_dir_all(dir)?;

    let (temp_path, mut file) = loop {
        let id = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        let temp_path = path.with_extension(format!("{}.{id}.tmp", std::process::id()));

        match fs::File::options()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => break (temp_path, file),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err.into()),
        }
    };

    let result = (|| -> eyre::Result<()> {
        let json = serde_json::to_vec_pretty(db)?;
        let enc = zstd::encode_all(&json[..], zstd::DEFAULT_COMPRESSION_LEVEL)?;
        file.write_all(&enc)?;
        drop(file);

        // Keep the old cache available until the complete replacement is ready.
        fs::rename(&temp_path, path)?;

        Ok(())
    })();

    if result.is_err() {
        if let Err(err) = fs::remove_file(&temp_path) {
            eprintln!("failed to clean up temporary cache file: {err}");
        }
    }

    result
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

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicU64, Ordering},
            mpsc,
        },
        thread,
        time::SystemTime,
    };

    use serde::{Serialize, Serializer};

    use super::*;

    struct TestCache(PathBuf);

    impl TestCache {
        fn new() -> Self {
            static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

            loop {
                let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
                let dir = std::env::temp_dir().join(format!(
                    "alfred-caniuse-rs-cache-test-{}-{id}",
                    std::process::id()
                ));

                match fs::create_dir(&dir) {
                    Ok(()) => return Self(dir),
                    Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(err) => panic!("failed to create test directory: {err}"),
                }
            }
        }

        fn path(&self) -> PathBuf {
            self.0.join("caniuse.zst")
        }
    }

    impl Drop for TestCache {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct PausedDb {
        db: Db,
        ready: mpsc::Sender<()>,
        resume: mpsc::Receiver<()>,
    }

    impl Serialize for PausedDb {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            self.ready.send(()).map_err(serde::ser::Error::custom)?;
            self.resume
                .recv_timeout(Duration::from_secs(10))
                .map_err(serde::ser::Error::custom)?;

            self.db.serialize(serializer)
        }
    }

    struct InvalidDb;

    impl Serialize for InvalidDb {
        fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("database serialization failed"))
        }
    }

    fn database(title: &str) -> Db {
        serde_json::from_value(serde_json::json!({
            "versions": {},
            "features": { "cache-test": { "title": title } },
        }))
        .unwrap()
    }

    fn paused_database(title: &str) -> (PausedDb, mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let db = PausedDb {
            db: database(title),
            ready: ready_tx,
            resume: resume_rx,
        };

        (db, ready_rx, resume_tx)
    }

    fn cached_title(path: &Path) -> Option<String> {
        cache_fetch_at(path).map(|db| db.get_feature("cache-test").unwrap().0.title.clone())
    }

    #[test]
    fn readers_keep_previous_cache_until_replacement_is_ready() {
        let cache = TestCache::new();
        let path = cache.path();
        cache_put_inner(&database("previous"), &path).unwrap();

        let (replacement, ready_rx, resume_tx) = paused_database("replacement");

        let during_write = thread::scope(|scope| {
            let writer_path = path.clone();
            let writer = scope.spawn(move || cache_put_inner(&replacement, &writer_path));
            ready_rx.recv_timeout(Duration::from_secs(10)).unwrap();

            let during_write = cached_title(&path);

            resume_tx.send(()).unwrap();
            writer.join().unwrap().unwrap();

            during_write
        });

        assert_eq!(during_write.as_deref(), Some("previous"));
        assert_eq!(cached_title(&path).as_deref(), Some("replacement"));
        assert_eq!(fs::read_dir(&cache.0).unwrap().count(), 1);
    }

    #[test]
    fn first_cache_is_visible_only_after_write_succeeds() {
        let cache = TestCache::new();
        let path = cache.path();
        let (db, ready_rx, resume_tx) = paused_database("first");

        let during_write = thread::scope(|scope| {
            let writer_path = path.clone();
            let writer = scope.spawn(move || cache_put_inner(&db, &writer_path));
            ready_rx.recv_timeout(Duration::from_secs(10)).unwrap();

            let during_write = cached_title(&path);

            resume_tx.send(()).unwrap();
            writer.join().unwrap().unwrap();

            during_write
        });

        assert_eq!(during_write, None);
        assert_eq!(cached_title(&path).as_deref(), Some("first"));
        assert_eq!(fs::read_dir(&cache.0).unwrap().count(), 1);
    }

    #[test]
    fn concurrent_writers_publish_complete_caches_independently() {
        let cache = TestCache::new();
        let path = cache.path();
        cache_put_inner(&database("previous"), &path).unwrap();

        let (first, first_ready, first_resume) = paused_database("first");
        let (second, second_ready, second_resume) = paused_database("second");

        let (before_first, after_first) = thread::scope(|scope| {
            let first_path = path.clone();
            let first_writer = scope.spawn(move || cache_put_inner(&first, &first_path));
            first_ready.recv_timeout(Duration::from_secs(10)).unwrap();

            let second_path = path.clone();
            let second_writer = scope.spawn(move || cache_put_inner(&second, &second_path));
            second_ready.recv_timeout(Duration::from_secs(10)).unwrap();

            let before_first = cached_title(&path);

            first_resume.send(()).unwrap();
            first_writer.join().unwrap().unwrap();

            let after_first = cached_title(&path);

            second_resume.send(()).unwrap();
            second_writer.join().unwrap().unwrap();

            (before_first, after_first)
        });

        assert_eq!(before_first.as_deref(), Some("previous"));
        assert_eq!(after_first.as_deref(), Some("first"));
        assert_eq!(cached_title(&path).as_deref(), Some("second"));
        assert_eq!(fs::read_dir(&cache.0).unwrap().count(), 1);
    }

    #[test]
    fn failed_write_preserves_previous_cache_and_cleans_temporary_file() {
        let cache = TestCache::new();
        let path = cache.path();
        cache_put_inner(&database("previous"), &path).unwrap();

        cache_put_at(&InvalidDb, &path);

        assert_eq!(cached_title(&path).as_deref(), Some("previous"));
        assert_eq!(fs::read_dir(&cache.0).unwrap().count(), 1);
    }

    #[test]
    fn failed_rename_cleans_only_its_own_temporary_file() {
        let cache = TestCache::new();
        let path = cache.path();
        fs::create_dir(&path).unwrap();
        fs::write(path.join("keep"), b"previous").unwrap();
        fs::write(cache.0.join("other.tmp"), b"other writer").unwrap();

        assert!(cache_put_inner(&database("replacement"), &path).is_err());

        assert_eq!(fs::read(path.join("keep")).unwrap(), b"previous");
        assert_eq!(
            fs::read(cache.0.join("other.tmp")).unwrap(),
            b"other writer"
        );
        assert_eq!(fs::read_dir(&cache.0).unwrap().count(), 2);
    }

    #[test]
    fn cache_age_tracks_last_write() {
        let cache = TestCache::new();
        let path = cache.path();
        cache_put_inner(&database("recent"), &path).unwrap();

        let file = fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(5 * 60 * 60))
            .unwrap();
        file.set_modified(SystemTime::now()).unwrap();

        assert_eq!(cached_title(&path).as_deref(), Some("recent"));
    }

    #[test]
    fn replacement_refreshes_cache_age() {
        let cache = TestCache::new();
        let path = cache.path();
        cache_put_inner(&database("previous"), &path).unwrap();

        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(5 * 60 * 60))
            .unwrap();

        assert!(cache_fetch_inner(&path).is_err());

        cache_put_inner(&database("replacement"), &path).unwrap();

        assert_eq!(cached_title(&path).as_deref(), Some("replacement"));
    }
}
