//! Multi-select support for Explorer context-menu verbs.
//!
//! Explorer starts one process per selected file for static verbs. Every process drops its
//! paths into a shared spool folder; the first one becomes the leader, waits until the
//! burst of launches is over, then handles all files in one window. The others exit.

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

const POLL: Duration = Duration::from_millis(50);
const QUIET: Duration = Duration::from_millis(350);
const MAX_WAIT: Duration = Duration::from_secs(4);

fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

fn spool_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "txt")).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Returns `Some(all files)` in the process that should do the work, `None` in the others.
pub fn collect(action: &str, files: Vec<PathBuf>) -> Option<Vec<PathBuf>> {
    let Some(first) = files.first() else { return Some(files) };
    let parent = first.parent().map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();
    let dir = std::env::temp_dir().join("OptiRAR").join(format!("batch-{:016x}", fnv1a(&format!("{action}|{parent}"))));
    if fs::create_dir_all(&dir).is_err() {
        return Some(files);
    }

    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mine = dir.join(format!("{}-{}.txt", std::process::id(), seq));
    let body: String = files.iter().map(|p| format!("{}\n", p.display())).collect();
    let tmp = mine.with_extension("part");
    if fs::write(&tmp, body).and_then(|_| fs::rename(&tmp, &mine)).is_err() {
        return Some(files);
    }

    let lock = dir.join("leader.lock");
    let try_lead = || {
        // A crashed leader must not block forever.
        if let Ok(meta) = fs::metadata(&lock) {
            let stale = meta.modified().ok().and_then(|m| SystemTime::now().duration_since(m).ok()).is_some_and(|age| age > Duration::from_secs(15));
            if stale {
                let _ = fs::remove_file(&lock);
            }
        }
        OpenOptions::new().write(true).create_new(true).open(&lock).is_ok()
    };

    if !try_lead() {
        // Follower: wait for the leader to take our file.
        let start = Instant::now();
        loop {
            std::thread::sleep(POLL);
            if !mine.exists() {
                return None;
            }
            if !lock.exists() && try_lead() {
                break; // leader finished before seeing us: handle the leftovers ourselves
            }
            if start.elapsed() > MAX_WAIT {
                let _ = fs::remove_file(&mine);
                return Some(files);
            }
        }
    }

    // Leader: wait until no new process has written for QUIET.
    let start = Instant::now();
    let mut count = spool_files(&dir).len();
    let mut last_change = Instant::now();
    while start.elapsed() < MAX_WAIT && last_change.elapsed() < QUIET {
        std::thread::sleep(POLL);
        let n = spool_files(&dir).len();
        if n != count {
            count = n;
            last_change = Instant::now();
        }
    }

    let mut all = Vec::new();
    for f in spool_files(&dir) {
        if let Ok(text) = fs::read_to_string(&f) {
            all.extend(text.lines().filter(|l| !l.is_empty()).map(PathBuf::from));
        }
        let _ = fs::remove_file(&f);
    }
    let _ = fs::remove_file(&lock);
    let _ = fs::remove_dir(&dir);
    all.sort();
    all.dedup();
    Some(all)
}

#[cfg(test)]
mod tests {
    #[test]
    fn concurrent_launches_merge_into_one_leader() {
        let action = format!("test-{}", std::process::id());
        let handles: Vec<_> = (0..5)
            .map(|i| {
                let action = action.clone();
                std::thread::spawn(move || super::collect(&action, vec![std::path::PathBuf::from(format!("C:/x/{i}.txt"))]))
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let leaders: Vec<_> = results.iter().flatten().collect();
        assert_eq!(leaders.len(), 1, "{results:?}");
        assert_eq!(leaders[0].len(), 5);
    }
}
