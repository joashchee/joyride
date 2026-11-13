//! Telling which files are the same file, reading as little as possible.
//!
//! Identity is the BLAKE3 hash of the whole content. To avoid reading every
//! file in full, candidates are narrowed in stages: files of a unique size
//! cannot match anything; of the rest, files whose first few KiB differ
//! cannot match; only the survivors are read in full.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Bytes read for the cheap second stage.
pub const PREFIX_BYTES: usize = 4096;

/// BLAKE3 of the whole file, as hex.
pub fn hash_file(path: &Path) -> io::Result<String> {
    let mut f = File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().to_hex().to_string())
}

/// BLAKE3 of some bytes, as hex.
pub fn hash_bytes(b: &[u8]) -> String {
    blake3::hash(b).to_hex().to_string()
}

/// BLAKE3 of the first `PREFIX_BYTES` (or the whole file if shorter).
pub fn prefix_hash(path: &Path) -> io::Result<String> {
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; PREFIX_BYTES];
    let mut got = 0;
    while got < PREFIX_BYTES {
        let n = f.read(&mut buf[got..])?;
        if n == 0 {
            break;
        }
        got += n;
    }
    Ok(blake3::hash(&buf[..got]).to_hex().to_string())
}

/// Runs `f` over every item on several threads, results in input order.
pub fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<R>>> = Mutex::new((0..items.len()).map(|_| None).collect());
    let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).min(8).min(items.len().max(1));
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= items.len() {
                    break;
                }
                let r = f(&items[i]);
                out.lock().unwrap()[i] = Some(r);
            });
        }
    });
    out.into_inner().unwrap().into_iter().map(|r| r.unwrap()).collect()
}

/// A file to identify.
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: std::path::PathBuf,
    pub size: u64,
}

/// How a staged run went, so callers (and tests) can see the saving.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stats {
    pub prefix_reads: usize,
    pub full_reads: usize,
}

/// Groups entries of identical content. Returns groups of indexes into
/// `entries` (each of two or more), and the read statistics. Unreadable
/// files are left out of every group. Zero-byte files are never grouped.
pub fn find_identical(entries: &[Entry]) -> (Vec<Vec<usize>>, Stats) {
    let mut stats = Stats::default();

    let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        if e.size > 0 {
            by_size.entry(e.size).or_default().push(i);
        }
    }
    let same_size: Vec<usize> = by_size.into_values().filter(|g| g.len() > 1).flatten().collect();

    stats.prefix_reads = same_size.len();
    let prefixes = par_map(&same_size, |&i| prefix_hash(&entries[i].path).ok());
    let mut by_prefix: HashMap<(u64, String), Vec<usize>> = HashMap::new();
    for (&i, p) in same_size.iter().zip(prefixes) {
        if let Some(p) = p {
            by_prefix.entry((entries[i].size, p)).or_default().push(i);
        }
    }
    let survivors: Vec<usize> = by_prefix.into_values().filter(|g| g.len() > 1).flatten().collect();

    stats.full_reads = survivors.len();
    let fulls = par_map(&survivors, |&i| hash_file(&entries[i].path).ok());
    let mut by_hash: HashMap<String, Vec<usize>> = HashMap::new();
    for (&i, h) in survivors.iter().zip(fulls) {
        if let Some(h) = h {
            by_hash.entry(h).or_default().push(i);
        }
    }
    let mut groups: Vec<Vec<usize>> = by_hash.into_values().filter(|g| g.len() > 1).collect();
    for g in &mut groups {
        g.sort_unstable();
    }
    groups.sort();
    (groups, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(d: &Path, name: &str, bytes: &[u8]) -> Entry {
        let p = d.join(name);
        std::fs::write(&p, bytes).unwrap();
        Entry { size: bytes.len() as u64, path: p }
    }

    #[test]
    fn finds_copies_and_reads_little() {
        let d = tempfile::tempdir().unwrap();
        let a = vec![7u8; 100_000];
        let mut b = a.clone();
        b[99_999] ^= 1; // same size and prefix, different at the end
        let entries = vec![
            write(d.path(), "a1", &a),
            write(d.path(), "a2", &a),
            write(d.path(), "b", &b),
            write(d.path(), "unique", &[1, 2, 3]),
            write(d.path(), "empty1", b""),
            write(d.path(), "empty2", b""),
        ];
        let (groups, stats) = find_identical(&entries);
        assert_eq!(groups, vec![vec![0, 1]]);
        // Three same-size files got a prefix read; the unique-size and empty ones none.
        assert_eq!(stats.prefix_reads, 3);
        assert_eq!(stats.full_reads, 3); // a1, a2, b share a prefix, so all are read in full
    }

    #[test]
    fn different_prefixes_are_never_read_in_full() {
        let d = tempfile::tempdir().unwrap();
        let e = vec![write(d.path(), "x", &[1u8; 9000]), write(d.path(), "y", &[2u8; 9000])];
        let (groups, stats) = find_identical(&e);
        assert!(groups.is_empty());
        assert_eq!((stats.prefix_reads, stats.full_reads), (2, 0));
    }

    #[test]
    fn a_missing_file_is_skipped_not_fatal() {
        let d = tempfile::tempdir().unwrap();
        let mut e = vec![write(d.path(), "x", b"hello"), write(d.path(), "y", b"hello")];
        e.push(Entry { path: d.path().join("gone"), size: 5 });
        let (groups, _) = find_identical(&e);
        assert_eq!(groups, vec![vec![0, 1]]);
    }

    #[test]
    fn hash_is_content_not_name() {
        let d = tempfile::tempdir().unwrap();
        let a = write(d.path(), "one.png", b"same bytes");
        let b = write(d.path(), "two.png", b"same bytes");
        assert_eq!(hash_file(&a.path).unwrap(), hash_file(&b.path).unwrap());
        assert_eq!(prefix_hash(&a.path).unwrap().len(), 64);
    }

    #[test]
    fn par_map_keeps_order() {
        let v: Vec<u32> = (0..200).collect();
        assert_eq!(par_map(&v, |x| x * 2), v.iter().map(|x| x * 2).collect::<Vec<_>>());
    }
}
