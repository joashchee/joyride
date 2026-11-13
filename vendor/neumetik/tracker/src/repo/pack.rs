//! **soundpak.rep**: a samples repository in one file, to take to another
//! machine or give to someone, and merge into theirs.
//!
//! ```text
//! SOUNDPAK            8 bytes
//! version             u16, 1
//! flags               u16, 0
//! entries, each:
//!   kind              u8: 1 the database (its text, packed), 2 a waveform, 3 a synth patch
//!   id                32 bytes (a waveform's or patch's ID; zeros for the database)
//!   length            u64
//!   bytes             a waveform or patch as its repository file (already packed)
//! end                 u8 0
//! ```
//!
//! Everything in it is checked on the way in: a waveform or patch must
//! hash to its ID, or it's left out. An older Neumetik passes patches over. Since IDs come from content, merging two
//! repositories can only join what's the same and add what isn't.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::Path;

use super::canon::Id;
use super::db::Db;
use super::patch::Patch;
use super::{unpack_wave, Import, Repository};
use crate::lz;

const MAGIC: &[u8; 8] = b"SOUNDPAK";
/// The most a pack's database may unpack to.
const MOST_DB: usize = 1 << 30;

/// What an export wrote.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Export {
    pub waves: usize,
    pub samples: usize,
    pub patches: usize,
    pub bytes: u64,
}

impl Repository {
    /// Writes the repository (or only the samples in `only`, with their
    /// waveforms, instruments, patches and modules) to a soundpak file at `to`.
    /// `progress` hears 0 to 1 as it goes.
    pub fn export(&self, to: &Path, only: Option<&[Id]>, progress: &mut dyn FnMut(f32)) -> Result<Export, String> {
        let db = match only {
            None => self.db.clone(),
            Some(ids) => subset(&self.db, ids),
        };
        let part = to.with_extension("saving");
        let mut f = std::io::BufWriter::new(std::fs::File::create(&part).map_err(|e| format!("Couldn't write the soundpak. ({e})"))?);
        let fail = |e: std::io::Error| format!("Couldn't write the soundpak. ({e})");
        let mut written = 0u64;
        let mut entry = |f: &mut std::io::BufWriter<std::fs::File>, kind: u8, id: &Id, bytes: &[u8]| -> std::io::Result<()> {
            f.write_all(&[kind])?;
            f.write_all(&id.0)?;
            f.write_all(&(bytes.len() as u64).to_le_bytes())?;
            f.write_all(bytes)?;
            written += 41 + bytes.len() as u64;
            Ok(())
        };
        (|| -> std::io::Result<()> {
            f.write_all(MAGIC)?;
            f.write_all(&1u16.to_le_bytes())?;
            f.write_all(&0u16.to_le_bytes())?;
            Ok(())
        })()
        .map_err(fail)?;
        entry(&mut f, 1, &Id::default(), &lz::compress(db.write().as_bytes())).map_err(fail)?;
        let total = (db.waves.len() + db.patches.len()).max(1);
        let mut waves = 0;
        for (i, id) in db.waves.keys().enumerate() {
            let b = self.wave_file(id)?;
            entry(&mut f, 2, id, &b).map_err(fail)?;
            waves += 1;
            progress((i + 1) as f32 / total as f32);
        }
        for (i, id) in db.patches.keys().enumerate() {
            let b = self.patch_file(id)?;
            entry(&mut f, 3, id, &b).map_err(fail)?;
            progress((waves + i + 1) as f32 / total as f32);
        }
        f.write_all(&[0]).map_err(fail)?;
        f.flush().map_err(fail)?;
        drop(f);
        std::fs::rename(&part, to).map_err(|e| {
            let _ = std::fs::remove_file(&part);
            format!("Couldn't write the soundpak. ({e})")
        })?;
        Ok(Export { waves, samples: db.samples.len(), patches: db.patches.len(), bytes: written + 13 })
    }

    /// Merges a soundpak into this repository: waveforms not kept here
    /// are added (each checked against its ID), and the database joined
    /// (`Db::merge`). Saves the database.
    pub fn import(&mut self, from: &Path, progress: &mut dyn FnMut(f32)) -> Result<Import, String> {
        let file = std::fs::File::open(from).map_err(|e| format!("Couldn't open the soundpak. ({e})"))?;
        let size = file.metadata().map(|m| m.len()).unwrap_or(1).max(1);
        let mut r = std::io::BufReader::new(file);
        let bad = |why: &str| format!("It isn't a soundpak Neumetik can read: {why}.");
        let mut head = [0u8; 12];
        r.read_exact(&mut head).map_err(|_| bad("it's too short"))?;
        if &head[..8] != MAGIC {
            return Err(bad("it doesn't start as one"));
        }
        if u16::from_le_bytes([head[8], head[9]]) != 1 {
            return Err(bad("it was made by a newer Neumetik"));
        }
        let mut theirs: Option<Db> = None;
        let mut report = Import::default();
        let mut have: BTreeSet<Id> = BTreeSet::new();
        let mut have_patches: BTreeSet<Id> = BTreeSet::new();
        let mut read = 12u64;
        loop {
            let mut kind = [0u8; 1];
            r.read_exact(&mut kind).map_err(|_| bad("it ends early"))?;
            if kind[0] == 0 {
                break;
            }
            let mut idb = [0u8; 32];
            r.read_exact(&mut idb).map_err(|_| bad("it ends early"))?;
            let mut lenb = [0u8; 8];
            r.read_exact(&mut lenb).map_err(|_| bad("it ends early"))?;
            let len = u64::from_le_bytes(lenb);
            if len > size {
                return Err(bad("an entry is longer than the file"));
            }
            let mut body = vec![0u8; len as usize];
            r.read_exact(&mut body).map_err(|_| bad("it ends early"))?;
            read += 41 + len;
            progress((read as f32 / size as f32).min(1.0));
            let id = Id(idb);
            match kind[0] {
                1 => {
                    let text = lz::decompress(&body, MOST_DB).map_err(|_| bad("its database is damaged"))?;
                    theirs = Some(Db::read(&String::from_utf8_lossy(&text)).map_err(|e| bad(&e))?);
                }
                2 => {
                    if self.has_wave(&id) {
                        have.insert(id);
                        continue;
                    }
                    match unpack_wave(&body, Some(id)) {
                        Ok(_) => {
                            self.write_wave_file(&id, &body)?;
                            have.insert(id);
                            report.waves += 1;
                            report.bytes_added += body.len() as u64;
                        }
                        Err(_) => report.damaged += 1,
                    }
                }
                3 => {
                    if self.has_patch(&id) {
                        have_patches.insert(id);
                        continue;
                    }
                    match Patch::unpack(&body, Some(id)) {
                        Ok(_) => {
                            self.write_patch_file(&id, &body)?;
                            have_patches.insert(id);
                            report.bytes_added += body.len() as u64;
                        }
                        Err(_) => report.damaged += 1,
                    }
                }
                _ => {}
            }
        }
        let mut theirs = theirs.ok_or_else(|| bad("it has no database"))?;
        // Waveforms whose files didn't come (or were damaged) aren't recorded.
        theirs.waves.retain(|id, _| have.contains(id) || self.db.waves.contains_key(id));
        for (id, w) in theirs.waves.iter_mut() {
            if let Ok(meta) = std::fs::metadata(self.wave_path(id)) {
                w.stored = meta.len();
            }
        }
        theirs.patches.retain(|id, _| have_patches.contains(id) || self.db.patches.contains_key(id));
        let m = self.db.merge(&theirs);
        self.db.retag_modules(false);
        report.samples = m.samples;
        report.instruments = m.instruments;
        report.patches = m.patches;
        report.modules = m.modules;
        report.joined = m.joined;
        self.save()?;
        Ok(report)
    }
}

/// The part of a database the samples `ids` need: them, their
/// waveforms, the instruments and synth patches that play only them,
/// and the modules they came from.
fn subset(db: &Db, ids: &[Id]) -> Db {
    let keep: BTreeSet<Id> = ids.iter().copied().filter(|i| db.samples.contains_key(i)).collect();
    let mut out = Db { repository: db.repository.clone(), name: db.name.clone(), created: db.created, merged: db.merged.clone(), ..Default::default() };
    for id in &keep {
        let s = &db.samples[id];
        out.samples.insert(*id, s.clone());
        if let Some(w) = db.waves.get(&s.wave) {
            out.waves.insert(s.wave, w.clone());
        }
        for src in &s.sources {
            if let Some(m) = db.modules.get(&src.module) {
                out.modules.insert(src.module, m.clone());
            }
        }
    }
    for (id, i) in &db.instruments {
        if !i.samples.is_empty() && i.samples.iter().all(|s| keep.contains(s)) {
            out.instruments.insert(*id, i.clone());
        }
    }
    for (id, p) in &db.patches {
        if !p.samples.is_empty() && p.samples.iter().all(|s| keep.contains(s)) {
            out.patches.insert(*id, p.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Loop, Module, Sample, Wave};
    use std::sync::Arc;

    fn temp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("neumetik-pack-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn module(seed: i16, name: &str) -> Module {
        let mut m = Module::new(4);
        m.format = "xm".into();
        m.samples = vec![Sample { name: name.into(), wave: Arc::new(Wave::new(16, 1, (0..3000).map(|i| (i as i16).wrapping_mul(seed)).collect())), looped: Loop::forward(100, 3000), ..Sample::empty() }];
        m
    }

    #[test]
    fn two_repositories_merge_through_a_soundpak() {
        let (da, db_) = (temp("a"), temp("b"));
        let mut a = Repository::open(&da).unwrap();
        let mut b = Repository::open(&db_).unwrap();
        a.scan(&module(3, "shared"), b"one", "one.xm", None).unwrap();
        a.scan(&module(5, "only a"), b"two", "two.xm", None).unwrap();
        let sb = b.scan(&module(3, "also shared"), b"three", "three.xm", None).unwrap();
        b.catalog_sample(&sb.slots[0].sample, vec!["Pad".into()], vec!["dreamy".into()], 4, "from b".into()).unwrap();
        let pak = da.join("out.rep");
        let e = b.export(&pak, None, &mut |_| {}).unwrap();
        assert_eq!((e.waves, e.samples), (1, 1));
        let r = a.import(&pak, &mut |_| {}).unwrap();
        assert_eq!((r.waves, r.samples, r.joined, r.damaged), (0, 0, 1, 0));
        let shared = &a.db.samples[&sb.slots[0].sample];
        assert_eq!(shared.names, vec!["shared", "also shared"]);
        assert_eq!(shared.catalog.categories, vec!["Pad"]);
        assert_eq!(shared.sources.len(), 2);
        assert_eq!(a.db.modules.len(), 3);
        // The other way: b gains a's own sample.
        let pak2 = db_.join("a.rep");
        a.export(&pak2, None, &mut |_| {}).unwrap();
        let r2 = b.import(&pak2, &mut |_| {}).unwrap();
        assert_eq!((r2.waves, r2.samples), (1, 1));
        assert_eq!(b.db.samples.len(), 2);
        assert!(b.db.merged.contains(&a.db.repository));
        // A damaged pack's waveform is left out.
        let mut bytes = std::fs::read(&pak2).unwrap();
        let n = bytes.len();
        bytes[n - 20] ^= 0xFF;
        std::fs::write(&pak2, &bytes).unwrap();
        let fresh = temp("c");
        let mut c = Repository::open(&fresh).unwrap();
        let r3 = c.import(&pak2, &mut |_| {}).unwrap();
        assert_eq!(r3.damaged, 1);
        for d in [da, db_, fresh] {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}
