//! The **universal tracker samples repository**: every waveform modules
//! use, each kept once, and `umf-samples`, the database that says what
//! each sample is and where it was found.
//!
//! When a module is scanned, each of its samples is cut to what's heard
//! (`canon`), given its ID, and looked up: a waveform already kept is
//! matched, not stored again; the same waveform with other loop points
//! is a new sample of the old waveform; an instrument matches only with
//! the same keymap (to the same samples) and envelopes; a synth patch
//! (`patch.rs`) only with the same kind, bytes and samples. Only what's
//! new is added. A module scanned before is known by its file's hash.
//!
//! On disk, a folder:
//!
//! ```text
//! umf-samples.db          the database (db.rs)
//! waves/3a/3a9e...umfw    each waveform, packed (lz.rs), named by its ID
//! patches/5c/5c1d...umfp  each synth patch, named by its ID
//! ```
//!
//! `pack` puts it all in one `soundpak.rep` to take elsewhere and merge.

pub mod analysis;
pub use crate::canon;
pub mod db;
pub mod pack;
pub mod patch;
pub mod rights;
pub mod search;
pub mod tagging;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::hash::{hex, Sha256};
use crate::model::{Instrument, Module, Wave, NOTE_MAX};
use canon::{canonical_wave, instrument_id, module_id, sample_id, wave_id, Id};
use db::{add_unique, Catalog, Db, InstrumentRec, ModuleRec, PatchRec, SampleRec, Source, UmfRec, WaveRec};

/// The longest a song's length is worked out to, in seconds: past
/// this it's long-form whatever it is.
const SONG_MOST: f64 = 1800.0;

/// The database's file in the repository's folder.
pub const DB_FILE: &str = "umf-samples.db";
const WAVES: &str = "waves";
const PATCHES: &str = "patches";
pub use crate::wavefile::{pack_wave, unpack_wave, MAGIC};

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// A new repository's ID: random enough that no two repositories share
/// one (the time to the nanosecond, the process, where it's running).
pub fn new_repository_id() -> String {
    let mut h = Sha256::new();
    let t = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    h.update(&t.to_le_bytes());
    h.update(&std::process::id().to_le_bytes());
    let here = 0u8;
    h.update(&(&here as *const u8 as usize).to_le_bytes());
    h.update(format!("{:?}", std::thread::current().id()).as_bytes());
    if let Ok(dir) = std::env::current_dir() {
        h.update(dir.to_string_lossy().as_bytes());
    }
    hex(&h.finish()[..16])
}

/// What `Repository::repack` did: the waveforms it tried, the ones it
/// packed smaller, and the bytes that saved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Repacked {
    pub tried: usize,
    pub repacked: usize,
    pub saved: u64,
}

/// How much the repository holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Storage {
    /// Bytes on disk: the waveforms' files and the database.
    pub bytes: u64,
    pub database_bytes: u64,
    pub waves: usize,
    pub samples: usize,
    pub instruments: usize,
    pub patches: usize,
    pub modules: usize,
    /// The waveforms at their own depth, unpacked.
    pub raw_bytes: u64,
    /// What the modules' samples took, all told, in the modules scanned.
    pub found_bytes: u64,
}

/// One sample slot of a scanned module.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotScan {
    /// 1-based.
    pub slot: u16,
    pub name: String,
    pub sample: Id,
    pub wave: Id,
    /// The waveform wasn't kept before (else it was matched).
    pub new_wave: bool,
    /// The sample (the waveform with its loops) wasn't kept before.
    pub new_sample: bool,
    pub frames: usize,
}

/// What a scan found and did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scan {
    pub module: Id,
    /// The module had been scanned before.
    pub seen_before: bool,
    pub slots: Vec<SlotScan>,
    /// Each instrument slot's ID and whether it's new.
    pub instruments: Vec<(u16, Id, bool)>,
    pub new_waves: usize,
    pub new_samples: usize,
    pub new_instruments: usize,
    /// Each synth patch's kind, slot and ID, and whether it's new.
    pub patches: Vec<(String, u16, Id, bool)>,
    pub new_patches: usize,
    /// Bytes the repository grew by.
    pub bytes_added: u64,
}

impl Scan {
    /// The samples that matched something kept.
    pub fn matched(&self) -> usize {
        self.slots.iter().filter(|s| !s.new_wave).count()
    }

    /// Each sample slot's IDs, for UMF (`slots` of them).
    pub fn refs(&self, slots: usize) -> Vec<Option<crate::umf::SampleRef>> {
        let mut v = vec![None; slots];
        for s in &self.slots {
            if let Some(r) = v.get_mut(usize::from(s.slot) - 1) {
                *r = Some(crate::umf::SampleRef { sample: s.sample, wave: s.wave });
            }
        }
        v
    }

    /// Each instrument slot's ID, for UMF (`slots` of them).
    pub fn instrument_ids(&self, slots: usize) -> Vec<Option<Id>> {
        let mut v = vec![None; slots];
        for &(slot, id, _) in &self.instruments {
            if let Some(r) = v.get_mut(usize::from(slot) - 1) {
                *r = Some(id);
            }
        }
        v
    }
}

/// What an import brought in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Import {
    pub waves: usize,
    pub samples: usize,
    pub instruments: usize,
    pub patches: usize,
    pub modules: usize,
    /// Samples both had, with something new said of them.
    pub joined: usize,
    /// Waveforms and patches that didn't match their IDs, left out.
    pub damaged: usize,
    pub bytes_added: u64,
}

pub struct Repository {
    root: PathBuf,
    pub db: Db,
    db_bytes: u64,
}

impl Repository {
    /// Opens the repository in `root`, making it when it isn't there.
    pub fn open(root: &Path) -> Result<Repository, String> {
        std::fs::create_dir_all(root.join(WAVES)).map_err(|e| format!("Couldn't make the samples repository's folder. ({e})"))?;
        let path = root.join(DB_FILE);
        let (db, db_bytes) = match std::fs::read(&path) {
            Ok(b) => {
                let text = String::from_utf8_lossy(&b);
                (Db::read(&text).map_err(|e| format!("The samples database couldn't be read: {e}."))?, b.len() as u64)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Db { repository: new_repository_id(), name: "Tracker samples".into(), created: now(), ..Default::default() }, 0),
            Err(e) => return Err(format!("The samples database couldn't be read. ({e})")),
        };
        let mut r = Repository { root: root.to_path_buf(), db, db_bytes };
        // Modules tagged by older rules (or before there were tags) are tagged again.
        if r.db.retag_modules(false) > 0 || r.db_bytes == 0 {
            r.save()?;
        }
        Ok(r)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Writes the database, through a file beside it renamed into place,
    /// so a failed write leaves what was there.
    pub fn save(&mut self) -> Result<(), String> {
        let text = self.db.write();
        let path = self.root.join(DB_FILE);
        let part = self.root.join(format!("{DB_FILE}.saving"));
        std::fs::write(&part, text.as_bytes()).and_then(|_| std::fs::rename(&part, &path)).map_err(|e| {
            let _ = std::fs::remove_file(&part);
            format!("The samples database couldn't be saved. ({e})")
        })?;
        self.db_bytes = text.len() as u64;
        Ok(())
    }

    fn wave_path(&self, id: &Id) -> PathBuf {
        let s = id.to_string();
        self.root.join(WAVES).join(&s[..2]).join(format!("{s}.umfw"))
    }

    pub fn has_wave(&self, id: &Id) -> bool {
        self.db.waves.contains_key(id) && self.wave_path(id).exists()
    }

    /// A waveform kept here, checked against its ID.
    pub fn wave(&self, id: &Id) -> Result<Wave, String> {
        let b = std::fs::read(self.wave_path(id)).map_err(|_| format!("The waveform {} isn't in the samples repository.", id.short()))?;
        unpack_wave(&b, Some(*id)).map_err(|e| format!("The waveform {} couldn't be read: {e}.", id.short()))
    }

    /// The packed file of a waveform kept here.
    pub fn wave_file(&self, id: &Id) -> Result<Vec<u8>, String> {
        std::fs::read(self.wave_path(id)).map_err(|_| format!("The waveform {} isn't in the samples repository.", id.short()))
    }

    /// Keeps a waveform (its canonical form) unless it's kept already:
    /// its ID, whether it's new, and the bytes it took.
    pub fn put_wave(&mut self, w: &Wave) -> Result<(Id, bool, u64), String> {
        let id = wave_id(w);
        if self.has_wave(&id) {
            return Ok((id, false, 0));
        }
        let packed = pack_wave(w);
        self.write_wave_file(&id, &packed)?;
        let f = analysis::measure(w);
        self.db.waves.insert(
            id,
            WaveRec {
                id,
                bits: w.bits,
                channels: w.channels,
                frames: w.frames() as u64,
                stored: packed.len() as u64,
                peak: f.peak,
                rms: f.rms,
                crossings: f.crossings,
                period: f.period,
                added: now(),
                spectrum: analysis::fingerprint(w),
                extra: Vec::new(),
            },
        );
        Ok((id, true, packed.len() as u64))
    }

    /// Packs again every waveform kept before `wavepack` (its file's
    /// version 1), keeping the new file where it's smaller. The sound
    /// doesn't change: each is checked against its ID first.
    pub fn repack(&mut self, progress: &mut dyn FnMut(f32)) -> Result<Repacked, String> {
        let ids: Vec<Id> = self.db.waves.keys().copied().collect();
        let mut r = Repacked::default();
        for (i, id) in ids.iter().enumerate() {
            progress(i as f32 / ids.len().max(1) as f32);
            let Ok(old) = std::fs::read(self.wave_path(id)) else { continue };
            if old.len() < 16 || old[4] != 1 {
                continue;
            }
            let Ok(w) = unpack_wave(&old, Some(*id)) else { continue };
            r.tried += 1;
            let new = pack_wave(&w);
            if new.len() < old.len() {
                self.write_wave_file(id, &new)?;
                r.repacked += 1;
                r.saved += (old.len() - new.len()) as u64;
                if let Some(rec) = self.db.waves.get_mut(id) {
                    rec.stored = new.len() as u64;
                }
            }
        }
        if r.repacked > 0 {
            self.save()?;
        }
        progress(1.0);
        Ok(r)
    }

    fn write_wave_file(&self, id: &Id, packed: &[u8]) -> Result<(), String> {
        let path = self.wave_path(id);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't make a folder in the samples repository. ({e})"))?;
        }
        let part = path.with_extension("saving");
        std::fs::write(&part, packed).and_then(|_| std::fs::rename(&part, &path)).map_err(|e| {
            let _ = std::fs::remove_file(&part);
            format!("A waveform couldn't be saved in the samples repository. ({e})")
        })
    }

    fn patch_path(&self, id: &Id) -> PathBuf {
        let s = id.to_string();
        self.root.join(PATCHES).join(&s[..2]).join(format!("{s}.umfp"))
    }

    pub fn has_patch(&self, id: &Id) -> bool {
        self.db.patches.contains_key(id) && self.patch_path(id).exists()
    }

    /// A synth patch kept here, checked against its ID.
    pub fn patch(&self, id: &Id) -> Result<patch::Patch, String> {
        let b = self.patch_file(id)?;
        patch::Patch::unpack(&b, Some(*id)).map_err(|e| format!("The synth patch {} couldn't be read: {e}.", id.short()))
    }

    /// The file of a synth patch kept here.
    pub fn patch_file(&self, id: &Id) -> Result<Vec<u8>, String> {
        std::fs::read(self.patch_path(id)).map_err(|_| format!("The synth patch {} isn't in the samples repository.", id.short()))
    }

    /// Writes a synth patch's file (already checked against its ID).
    pub(crate) fn write_patch_file(&self, id: &Id, file: &[u8]) -> Result<(), String> {
        let path = self.patch_path(id);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("Couldn't make a folder in the samples repository. ({e})"))?;
        }
        let part = path.with_extension("saving");
        std::fs::write(&part, file).and_then(|_| std::fs::rename(&part, &path)).map_err(|e| {
            let _ = std::fs::remove_file(&part);
            format!("A synth patch couldn't be saved in the samples repository. ({e})")
        })
    }

    /// Keeps a synth patch unless it's kept already (the same kind,
    /// bytes and samples): its ID, whether it's new, and the bytes it took.
    pub fn put_patch(&mut self, p: &patch::Patch, format_name: &str) -> Result<(Id, bool, u64), String> {
        let id = p.id();
        if self.has_patch(&id) {
            return Ok((id, false, 0));
        }
        let file = p.pack();
        self.write_patch_file(&id, &file)?;
        let rec = self.db.patches.entry(id).or_insert_with(|| PatchRec { id, kind: p.kind.clone(), samples: p.samples.clone(), catalog: Catalog { added: now(), ..Default::default() }, ..Default::default() });
        rec.bytes = p.data.len() as u64;
        rec.stored = file.len() as u64;
        if rec.format_name.is_empty() {
            rec.format_name = patch::kind_name(&p.kind).unwrap_or(format_name).to_string();
        }
        Ok((id, true, file.len() as u64))
    }

    /// Takes a synth patch out (its file and its record).
    pub fn remove_patch(&mut self, id: &Id) -> Result<(), String> {
        if self.db.patches.remove(id).is_some() {
            let _ = std::fs::remove_file(self.patch_path(id));
            self.save()?;
        }
        Ok(())
    }

    /// Files a synth patch: its name (first of its names), categories, tags, rating and notes.
    pub fn catalog_patch(&mut self, id: &Id, name: Option<&str>, categories: Vec<String>, tags: Vec<String>, rating: u8, notes: String) -> Result<(), String> {
        let p = self.db.patches.get_mut(id).ok_or("That synth patch isn't in the samples repository.")?;
        if let Some(n) = name.map(str::trim).filter(|n| !n.is_empty()) {
            p.names.retain(|x| x != n);
            p.names.insert(0, n.to_string());
        }
        p.catalog.categories = categories;
        p.catalog.tags = tags;
        p.catalog.rating = rating.min(5);
        p.catalog.notes = notes;
        self.save()
    }

    pub fn storage(&self) -> Storage {
        let waves: u64 = self.db.waves.values().map(|w| w.stored).sum();
        let patches: u64 = self.db.patches.values().map(|p| p.stored).sum();
        Storage {
            bytes: waves + patches + self.db_bytes,
            database_bytes: self.db_bytes,
            waves: self.db.waves.len(),
            samples: self.db.samples.len(),
            instruments: self.db.instruments.len(),
            patches: self.db.patches.len(),
            modules: self.db.modules.len(),
            raw_bytes: self.db.waves.values().map(|w| w.raw_bytes()).sum(),
            found_bytes: self.db.modules.values().map(|m| m.sample_bytes).sum(),
        }
    }

    /// Scans a module: every sample and instrument matched against what's
    /// kept, and only what's new added; the module recorded as where they
    /// were found. `file` is the module's file as read (its hash is the
    /// module's ID), `path` where it is. Saves the database.
    pub fn scan(&mut self, module: &Module, file: &[u8], file_name: &str, path: Option<&str>) -> Result<Scan, String> {
        let mid = module_id(file);
        let mut scan = self.register(module, Some((mid, file_name, path, file.len() as u64)))?;
        scan.module = mid;
        self.save()?;
        Ok(scan)
    }

    /// Keeps a module's samples and instruments (an edited song's, as it's
    /// saved): `from` is the module's file it came from, when there is one.
    pub fn register(&mut self, module: &Module, from: Option<(Id, &str, Option<&str>, u64)>) -> Result<Scan, String> {
        let mut scan = Scan::default();
        let seen_before = from.is_some_and(|(mid, ..)| self.db.modules.contains_key(&mid));
        scan.seen_before = seen_before;
        let added = now();
        let mut slot_ids: Vec<Option<Id>> = vec![None; module.samples.len()];
        for (i, s) in module.samples.iter().enumerate() {
            if s.wave.is_empty() {
                continue;
            }
            let canon = canonical_wave(s);
            if canon.is_empty() {
                continue;
            }
            let (wid, new_wave, bytes) = self.put_wave(&canon)?;
            scan.bytes_added += bytes;
            let sid = sample_id(wid, s);
            slot_ids[i] = Some(sid);
            let slot = (i + 1) as u16;
            let new_sample = !self.db.samples.contains_key(&sid);
            let frames = canon.frames();
            let rec = self.db.samples.entry(sid).or_insert_with(|| {
                let f = analysis::measure(&canon);
                let names: Vec<&str> = [s.name.as_str(), s.filename.as_str()].into_iter().filter(|n| !n.trim().is_empty()).collect();
                let mut tags = analysis::auto_tags(s, frames, s.c5);
                if !module.format.is_empty() {
                    tags.push(format!("from {}", module.format));
                }
                SampleRec {
                    id: sid,
                    wave: wid,
                    looped: s.looped.clamped(frames as u32),
                    sustain: s.sustain.clamped(frames as u32),
                    volume: s.volume,
                    pan: s.pan,
                    guessed: analysis::guess_category(&names, s, &f).to_string(),
                    catalog: Catalog { tags, added, ..Default::default() },
                    ..Default::default()
                }
            });
            if !s.name.trim().is_empty() && !rec.names.iter().any(|n| n == s.name.trim()) {
                rec.names.push(s.name.trim().to_string());
            }
            if !s.filename.trim().is_empty() {
                let f = s.filename.trim().to_string();
                add_unique(&mut rec.filenames, &f);
            }
            add_unique(&mut rec.rates, &s.c5);
            if let Some((mid, ..)) = from {
                add_unique(&mut rec.sources, &Source { module: mid, slot });
            }
            // Its own credit (a sample file's copyright or artist), as found.
            let credit = s.credit.trim();
            if !credit.is_empty() {
                let line = format!("its credit: {credit}");
                if !rec.rights.contains(&line) {
                    rec.rights.push(line);
                }
            }
            if new_wave {
                scan.new_waves += 1;
            }
            if new_sample {
                scan.new_samples += 1;
            }
            scan.slots.push(SlotScan { slot, name: s.name.clone(), sample: sid, wave: wid, new_wave, new_sample, frames });
        }
        let ids = |slot: u16| -> Option<Id> { slot_ids.get(usize::from(slot).checked_sub(1)?).copied().flatten() };
        let mut ins_ids = Vec::new();
        if module.flags.instruments {
            for (i, ins) in module.instruments.iter().enumerate() {
                let samples: Vec<Id> = {
                    let mut v: Vec<Id> = Vec::new();
                    for &(_, s) in &ins.keymap {
                        if let Some(id) = ids(s) {
                            add_unique(&mut v, &id);
                        }
                    }
                    v
                };
                if samples.is_empty() {
                    continue;
                }
                let iid = instrument_id(ins, &ids);
                let slot = (i + 1) as u16;
                let new = !self.db.instruments.contains_key(&iid);
                let rec = self.db.instruments.entry(iid).or_insert_with(|| InstrumentRec {
                    id: iid,
                    samples: samples.clone(),
                    keymap: keymap_words(ins, samples.len()),
                    envelopes: envelope_words(ins),
                    fadeout: ins.fadeout,
                    catalog: Catalog { added, ..Default::default() },
                    ..Default::default()
                });
                if !ins.name.trim().is_empty() && !rec.names.iter().any(|n| n == ins.name.trim()) {
                    rec.names.push(ins.name.trim().to_string());
                }
                if !ins.filename.trim().is_empty() {
                    let f = ins.filename.trim().to_string();
                    add_unique(&mut rec.filenames, &f);
                }
                if let Some((mid, ..)) = from {
                    add_unique(&mut rec.sources, &Source { module: mid, slot });
                }
                if new {
                    scan.new_instruments += 1;
                }
                scan.instruments.push((slot, iid, new));
                ins_ids.push((slot, iid));
            }
        }
        // Its synth patches, each kept once.
        self.keep_patches(module, &slot_ids, from.map(|(mid, ..)| mid), &mut scan)?;
        if let Some((mid, name, path, bytes)) = from {
            let rec = self.db.modules.entry(mid).or_insert_with(|| ModuleRec {
                id: mid,
                format: module.format.clone(),
                format_name: module.format_name.clone(),
                tracker: module.tracker.clone(),
                title: module.title.clone(),
                artist: module.artist.clone(),
                message: module.message.chars().take(4000).collect(),
                bytes,
                channels: module.channels.len() as u16,
                patterns: module.patterns.len() as u16,
                orders: module.orders.len() as u16,
                sample_bytes: module.samples.iter().map(|s| s.wave.bytes() as u64).sum(),
                samples: scan.slots.iter().map(|s| (s.slot, s.sample)).collect(),
                instruments: ins_ids.clone(),
                scanned: added,
                ..Default::default()
            });
            // Its file's SAUCE record, also for a record kept before
            // SAUCE was read (the same file always has the same record).
            if let Some(s) = &module.sauce {
                rec.group = s.group.clone();
                rec.date = s.date_text();
                rec.comments = s.comments.clone();
                if rec.title.is_empty() {
                    rec.title = s.title.clone();
                }
                if rec.artist.is_empty() {
                    rec.artist = s.author.clone();
                }
            }
            // What it says of its rights: a license, copyright lines.
            let (license, lines) = rights::in_module(module);
            if rec.license.is_empty() {
                if let Some((_, l)) = license {
                    rec.license = l.to_string();
                }
            }
            for l in lines {
                add_unique(&mut rec.rights, &l);
            }
            if !name.is_empty() {
                let n = name.to_string();
                add_unique(&mut rec.files, &n);
            }
            if let Some(p) = path {
                let p = p.to_string();
                add_unique(&mut rec.paths, &p);
            }
            // How long it plays, for its length tag: worked out once, without its sound.
            if rec.seconds == 0.0 {
                rec.seconds = crate::player::length(std::sync::Arc::new(module.clone()), SONG_MOST) as f32;
            }
            self.retag(&mid);
        }
        Ok(scan)
    }

    /// Keeps a module's synth patches (`patch::found`), its samples by
    /// `slot_ids`; `from` is the module they were found in.
    fn keep_patches(&mut self, module: &Module, slot_ids: &[Option<Id>], from: Option<Id>, scan: &mut Scan) -> Result<(), String> {
        for f in patch::found(module) {
            // A sample it names that's silent is none (a zero ID).
            let samples: Vec<Id> = f.samples.iter().map(|&i| slot_ids.get(usize::from(i)).copied().flatten().unwrap_or_default()).collect();
            if f.data.is_empty() && samples.iter().all(Id::is_zero) {
                continue;
            }
            let p = patch::Patch { kind: f.kind.clone(), samples, data: f.data };
            let (pid, new, bytes) = self.put_patch(&p, &module.format_name)?;
            scan.bytes_added += bytes;
            let rec = self.db.patches.get_mut(&pid).expect("just kept");
            if !f.name.is_empty() && !rec.names.iter().any(|n| *n == f.name) {
                rec.names.push(f.name.clone());
            }
            if let Some(mid) = from {
                add_unique(&mut rec.sources, &Source { module: mid, slot: f.slot });
            }
            if new {
                scan.new_patches += 1;
            }
            scan.patches.push((f.kind, f.slot, pid, new));
        }
        Ok(())
    }

    /// Keeps the synth patches of every UMF file in `folder` (the songs
    /// stored before patches were kept): (files read, patches found, new).
    /// Their samples are named by the IDs the files give them; their
    /// waveforms aren't read. Saves the database.
    pub fn gather_patches(&mut self, folder: &Path, progress: &mut dyn FnMut(f32)) -> Result<(usize, usize, usize), String> {
        let mut files = Vec::new();
        walk_umfs(folder, &mut files);
        let mut scan = Scan::default();
        let mut read = 0;
        for (i, p) in files.iter().enumerate() {
            progress(i as f32 / files.len().max(1) as f32);
            let Ok(data) = std::fs::read(p) else { continue };
            let Ok(loaded) = crate::umf::read(&data, &mut |_| None) else { continue };
            read += 1;
            let slot_ids: Vec<Option<Id>> = loaded.refs.iter().map(|r| r.map(|r| r.sample)).collect();
            let from = loaded.about.source.filter(|m| self.db.modules.contains_key(m));
            self.keep_patches(&loaded.module, &slot_ids, from, &mut scan)?;
        }
        self.save()?;
        progress(1.0);
        Ok((read, scan.patches.len(), scan.new_patches))
    }

    /// Notes a UMF file saved from a module.
    pub fn note_umf(&mut self, module: Id, path: &str) -> Result<(), String> {
        if let Some(m) = self.db.modules.get_mut(&module) {
            let p = path.to_string();
            if add_unique(&mut m.umfs, &p) {
                self.save()?;
            }
        }
        Ok(())
    }

    /// What a module's `.meta` file says (the archive it came from: its
    /// license, copyright, where it was): its license wins over what was
    /// found in the module.
    pub fn note_meta(&mut self, module: Id, meta: &rights::Meta) -> Result<(), String> {
        let Some(m) = self.db.modules.get_mut(&module) else { return Ok(()) };
        let before = m.clone();
        let from = if meta.archive.is_empty() { "its source".to_string() } else { meta.archive.clone() };
        if !meta.license.is_empty() {
            m.license = meta.license.clone();
            add_unique(&mut m.rights, &format!("{from}: license {}", meta.license));
        }
        for c in &meta.copyright {
            add_unique(&mut m.rights, &format!("{from}: {c}"));
        }
        for u in &meta.sources {
            add_unique(&mut m.origins, u);
        }
        if !meta.archive.is_empty() {
            m.archive = meta.archive.clone();
        }
        if m.artist.is_empty() {
            m.artist = meta.artist.clone();
        }
        self.retag(&module);
        let m = &self.db.modules[&module];
        if *m != before {
            self.save()?;
        }
        Ok(())
    }

    /// A module's tags guessed again (`tagging.rs`), from all that's known of it.
    pub fn retag(&mut self, module: &Id) {
        let Some(m) = self.db.modules.get(module) else { return };
        let tags = tagging::module_tags(&self.db, m);
        let m = self.db.modules.get_mut(module).expect("just found");
        m.guessed = tags;
        m.tagged = tagging::VERSION;
    }

    /// Samples marked free to use (`Some(true)`), not (`Some(false)`), or
    /// left to what's found (`None`). Returns the samples changed.
    pub fn mark_free(&mut self, ids: &[Id], free: Option<bool>) -> Result<usize, String> {
        let mut n = 0;
        for id in ids {
            if let Some(s) = self.db.samples.get_mut(id) {
                if s.free != free {
                    s.free = free;
                    n += 1;
                }
            }
        }
        if n > 0 {
            self.save()?;
        }
        Ok(n)
    }

    /// Files a sample: its categories, tags, rating and notes, as given.
    pub fn catalog_sample(&mut self, id: &Id, categories: Vec<String>, tags: Vec<String>, rating: u8, notes: String) -> Result<(), String> {
        let s = self.db.samples.get_mut(id).ok_or("That sample isn't in the repository.")?;
        let clean = |v: Vec<String>| -> Vec<String> {
            let mut out: Vec<String> = Vec::new();
            for x in v {
                let x = x.trim().to_string();
                if !x.is_empty() && !out.contains(&x) {
                    out.push(x);
                }
            }
            out
        };
        s.catalog.categories = clean(categories);
        s.catalog.tags = clean(tags);
        s.catalog.rating = rating.min(5);
        s.catalog.notes = notes;
        self.save()
    }

    /// Takes a sample out of the repository, and its waveform when no
    /// other sample uses it. UMF files that refer to it won't play it.
    pub fn remove_sample(&mut self, id: &Id) -> Result<(), String> {
        let s = self.db.samples.remove(id).ok_or("That sample isn't in the repository.")?;
        if !self.db.samples.values().any(|o| o.wave == s.wave) {
            self.db.waves.remove(&s.wave);
            let _ = std::fs::remove_file(self.wave_path(&s.wave));
        }
        self.save()
    }

    pub fn rename(&mut self, name: &str) -> Result<(), String> {
        self.db.name = name.trim().to_string();
        self.save()
    }

    /// Files several samples at once: categories and tags added and
    /// taken away, and a rating when given. Returns the samples changed.
    pub fn catalog_many(&mut self, ids: &[Id], add: &Filing, remove: &Filing, rating: Option<u8>) -> Result<usize, String> {
        let mut n = 0;
        for id in ids {
            let Some(s) = self.db.samples.get_mut(id) else { continue };
            let before = s.catalog.clone();
            let c = &mut s.catalog;
            for x in &remove.categories {
                c.categories.retain(|v| v != x);
            }
            for x in &remove.tags {
                c.tags.retain(|v| v != x);
            }
            for x in add.categories.iter().map(|x| x.trim()).filter(|x| !x.is_empty()) {
                if !c.categories.iter().any(|v| v == x) {
                    c.categories.push(x.to_string());
                }
            }
            for x in add.tags.iter().map(|x| x.trim()).filter(|x| !x.is_empty()) {
                if !c.tags.iter().any(|v| v == x) {
                    c.tags.push(x.to_string());
                }
            }
            if let Some(r) = rating {
                c.rating = r.min(5);
            }
            if s.catalog != before {
                n += 1;
            }
        }
        if n > 0 {
            self.save()?;
        }
        Ok(n)
    }

    /// A tag (or with `category`, a category) renamed everywhere, or
    /// merged into one that's there. Returns the samples changed.
    pub fn rename_tag(&mut self, from: &str, to: &str, category: bool) -> Result<usize, String> {
        let n = if category { self.db.rename_category(from, to) } else { self.db.rename_tag(from, to) };
        if n > 0 {
            self.save()?;
        }
        Ok(n)
    }

    /// Notes a UMF file saved at `path` and the samples it plays.
    pub fn note_umf_file(&mut self, path: &str, samples: &[Id]) -> Result<(), String> {
        let id = module_id(path.as_bytes());
        let rec = UmfRec { id, path: path.to_string(), samples: samples.to_vec(), saved: now() };
        if self.db.umfs.get(&id).map(|u| (&u.path, &u.samples)) != Some((&rec.path, &rec.samples)) {
            self.db.umfs.insert(id, rec);
            self.save()?;
        }
        Ok(())
    }

    /// The UMF files in a folder (and the folders in it) noted, as
    /// they're found: the samples each plays. Files gone from where
    /// they were noted are forgotten. Returns how many files it found.
    pub fn index_umfs(&mut self, folder: &Path) -> Result<usize, String> {
        let mut found = Vec::new();
        walk_umfs(folder, &mut found);
        for p in &found {
            let Ok(data) = std::fs::read(p) else { continue };
            let Ok(slots) = crate::umf::slots(&data) else { continue };
            let samples: Vec<Id> = slots.iter().filter_map(|e| e.refs.map(|r| r.sample)).collect();
            let path = p.to_string_lossy().to_string();
            let id = module_id(path.as_bytes());
            self.db.umfs.insert(id, UmfRec { id, path, samples, saved: now() });
        }
        self.db.umfs.retain(|_, u| Path::new(&u.path).exists());
        self.save()?;
        Ok(found.len())
    }

    /// Every UMF file that plays sample `from` made to play `to` instead
    /// (its waveform, loops and tuning; the name, volume and the rest
    /// as the song had them). Returns the files changed, and the ones
    /// that couldn't be, with why.
    pub fn repoint(&mut self, from: &Id, to: &Id) -> Result<Repointed, String> {
        let new = self.db.samples.get(to).ok_or("The sample to use instead isn't in the repository.")?.clone();
        let wave = self.db.waves.get(&new.wave).ok_or("The sample to use instead has no waveform here.")?.clone();
        let users: Vec<UmfRec> = self.db.umfs_using(from).into_iter().cloned().collect();
        let (mut done, mut failed) = (Vec::new(), Vec::new());
        for u in users {
            let result = std::fs::read(&u.path).map_err(|e| e.to_string()).and_then(|data| {
                crate::umf::rewrite_slots(
                    &data,
                    &mut |e| {
                        if e.refs.map(|r| r.sample) != Some(*from) {
                            return false;
                        }
                        e.refs = Some(crate::umf::SampleRef { sample: new.id, wave: new.wave });
                        e.frames = wave.frames as u32;
                        e.bits = wave.bits;
                        e.channels = wave.channels;
                        e.looped = new.looped;
                        e.sustain = new.sustain;
                        if let Some(&r) = new.rates.first() {
                            e.c5 = r;
                        }
                        true
                    },
                    &mut |id| self.wave_file(id).ok(),
                )
                .map_err(|e| e.to_string())
            });
            match result {
                Ok(Some(bytes)) => {
                    let part = format!("{}.saving", u.path);
                    match std::fs::write(&part, &bytes).and_then(|_| std::fs::rename(&part, &u.path)) {
                        Ok(()) => {
                            let rec = self.db.umfs.get_mut(&u.id).expect("noted");
                            rec.samples.retain(|s| s != from);
                            add_unique(&mut rec.samples, to);
                            done.push(u.path.clone());
                        }
                        Err(e) => {
                            let _ = std::fs::remove_file(&part);
                            failed.push((u.path.clone(), e.to_string()));
                        }
                    }
                }
                Ok(None) => {}
                Err(e) => failed.push((u.path.clone(), e)),
            }
        }
        self.save()?;
        Ok((done, failed))
    }

    /// The samples that sound most like one (by their fingerprints),
    /// best first, with how alike (0 to 1); `limit` of them. Waveforms
    /// with no fingerprint yet get theirs first (read from here), so the
    /// first search over an old repository takes a while.
    pub fn similar(&mut self, id: &Id, limit: usize, progress: &mut dyn FnMut(f32)) -> Result<Vec<(Id, f32)>, String> {
        let wave = self.db.samples.get(id).ok_or("That sample isn't in the repository.")?.wave;
        self.fingerprint_all(progress)?;
        let mine = self.db.waves.get(&wave).map(|w| w.spectrum.clone()).unwrap_or_default();
        if mine.is_empty() {
            return Ok(Vec::new());
        }
        let mut score: std::collections::HashMap<Id, f32> = std::collections::HashMap::new();
        for w in self.db.waves.values().filter(|w| w.id != wave && !w.spectrum.is_empty()) {
            score.insert(w.id, analysis::similarity(&mine, &w.spectrum));
        }
        let mut v: Vec<(Id, f32)> = self.db.samples.values().filter(|s| s.id != *id && s.wave != wave).filter_map(|s| score.get(&s.wave).map(|&k| (s.id, k))).collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
        v.truncate(limit);
        Ok(v)
    }

    /// Fingerprints for every waveform that hasn't one. Saves when it made any.
    pub fn fingerprint_all(&mut self, progress: &mut dyn FnMut(f32)) -> Result<usize, String> {
        let missing: Vec<Id> = self.db.waves.values().filter(|w| w.spectrum.is_empty()).map(|w| w.id).collect();
        let total = missing.len().max(1);
        for (i, id) in missing.iter().enumerate() {
            if i % 16 == 0 {
                progress(i as f32 / total as f32);
            }
            if let Ok(w) = self.wave(id) {
                let f = analysis::fingerprint(&w);
                if let Some(rec) = self.db.waves.get_mut(id) {
                    rec.spectrum = f;
                }
            }
        }
        if !missing.is_empty() {
            self.save()?;
        }
        progress(1.0);
        Ok(missing.len())
    }
}

/// The UMF files a repoint changed, and the ones it couldn't, with why.
pub type Repointed = (Vec<String>, Vec<(String, String)>);

/// What filing adds or takes away.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filing {
    pub categories: Vec<String>,
    pub tags: Vec<String>,
}

fn walk_umfs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk_umfs(&p, out);
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("umf") || x.eq_ignore_ascii_case("ntt")) {
            out.push(p);
        }
    }
}

/// An instrument's keymap in words.
fn keymap_words(ins: &Instrument, samples: usize) -> String {
    let mapped: Vec<usize> = ins.keymap.iter().enumerate().filter(|(_, k)| k.1 > 0).map(|(n, _)| n).collect();
    match (mapped.first(), mapped.last()) {
        (Some(&a), Some(&b)) => format!(
            "{} to {}: {} {}",
            crate::model::note_name(a as u8 + 1),
            crate::model::note_name((b as u8 + 1).min(NOTE_MAX)),
            samples,
            if samples == 1 { "sample" } else { "samples" }
        ),
        _ => "no notes".into(),
    }
}

fn envelope_words(ins: &Instrument) -> Vec<String> {
    let mut v = Vec::new();
    if ins.volume_env.enabled {
        v.push("volume".into());
    }
    if ins.pan_env.enabled {
        v.push("panning".into());
    }
    if ins.pitch_env.enabled {
        v.push(if ins.pitch_env.filter { "filter" } else { "pitch" }.into());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lz;
    use crate::model::{Loop, Sample};
    use std::sync::Arc;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("neumetik-tracker-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn module(samples: Vec<Sample>) -> Module {
        let mut m = Module::new(4);
        m.format = "mod".into();
        m.samples = samples;
        m
    }

    fn smp(name: &str, pcm: Vec<i16>, looped: Loop) -> Sample {
        Sample { name: name.into(), wave: Arc::new(Wave::new(8, 1, pcm.into_iter().map(|v| v << 8).collect())), looped, ..Sample::empty() }
    }

    #[test]
    fn synth_patches_are_kept_once_and_play_again_as_samples() {
        use crate::model::{FmPatch, Program, Synth, SynthSound};
        let dir = temp("patches");
        let mut repo = Repository::open(&dir).unwrap();
        let fm = FmPatch::from_s3m(&[0x21, 0x31, 0x4F, 0x00, 0xF2, 0xF2, 0x52, 0x73, 0x00, 0x00, 0x06]);
        let sound = SynthSound { waveforms: vec![(0..32).map(|i| (i * 8 - 128) as i8).collect()], volumes: vec![64, 0xFF], waves: vec![0, 0xFF], vol_speed: 1, wave_speed: 1, hybrid: false };
        let song = |names: [&str; 3]| {
            let mut a = Sample::empty();
            a.name = names[0].into();
            a.fm = Some(fm);
            let mut b = Sample::empty();
            b.name = names[1].into();
            b.program = Some(Arc::new(Program::new(sound.clone(), &Arc::default())));
            let mut c = crate::sample_edit::synth(&Synth { wave: 2, width: 40, ..Default::default() });
            c.name = names[2].into();
            module(vec![a, b, c])
        };
        let s1 = repo.scan(&song(["organ", "buzz", "saw"]), b"one", "one.s3m", None).unwrap();
        let s2 = repo.scan(&song(["Organ 2", "buzz", "saw"]), b"two", "two.med", None).unwrap();
        assert_eq!((s1.patches.len(), s1.new_patches, s2.new_patches), (3, 3, 0));
        assert_eq!(repo.db.patches.len(), 3);
        let (_, _, opl, _) = s1.patches.iter().find(|p| p.0 == "opl").unwrap();
        let rec = &repo.db.patches[opl];
        assert_eq!((rec.names.clone(), rec.sources.len(), rec.format_name.as_str()), (vec!["organ".to_string(), "Organ 2".to_string()], 2, "AdLib FM (OPL)"));
        // Each plays again as a sample of a new song.
        for (kind, _, id, _) in &s1.patches {
            let p = repo.patch(id).unwrap();
            let s = patch::to_sample(&p, "again", &[]).unwrap_or_else(|| panic!("{kind} didn't make a sample"));
            assert!(s.sounds(), "{kind}");
        }
        assert_eq!(patch::to_sample(&repo.patch(opl).unwrap(), "x", &[]).unwrap().fm, Some(fm));
        // The database and soundpak carry them.
        repo.save().unwrap();
        let back = Repository::open(&dir).unwrap();
        assert_eq!(back.db.patches, repo.db.patches);
        let pak = dir.join("p.rep");
        assert_eq!(repo.export(&pak, None, &mut |_| {}).unwrap().patches, 3);
        let other = temp("patches-b");
        let mut b = Repository::open(&other).unwrap();
        let r = b.import(&pak, &mut |_| {}).unwrap();
        assert_eq!((r.patches, r.damaged), (3, 0));
        assert!(b.patch(opl).is_ok());
        // A UMF stored before patches were kept gives them up from the library.
        let lib = dir.join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        let m = song(["organ", "buzz", "saw"]);
        let sc = repo.register(&m, None).unwrap();
        let about = crate::umf::About { source: None, repository: repo.db.repository.clone(), bundled: false };
        std::fs::write(lib.join("old.umf"), crate::umf::write(&m, &sc.refs(m.samples.len()), &sc.instrument_ids(0), &about, false)).unwrap();
        let fresh = temp("patches-c");
        let mut c = Repository::open(&fresh).unwrap();
        assert_eq!(c.gather_patches(&lib, &mut |_| {}).unwrap(), (1, 3, 3));
        assert_eq!(c.db.patches.keys().collect::<Vec<_>>(), repo.db.patches.keys().collect::<Vec<_>>());
        assert_eq!(c.gather_patches(&lib, &mut |_| {}).unwrap(), (1, 3, 0));
        for d in [dir, other, fresh] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn older_waveform_files_still_read_and_repack_smaller() {
        let dir = temp("repack");
        let mut repo = Repository::open(&dir).unwrap();
        // A tone with some noise, as a recording is (an exact repeat, LZ would win).
        let mut noise = 7u32;
        let tone: Vec<i16> = (0..8000)
            .map(|i| {
                noise = noise.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (((i as f32 * 0.07).sin() * 90.0) as i16 + (noise >> 28) as i16 - 8) << 8
            })
            .collect();
        let w = Wave::new(8, 1, tone);
        let (id, ..) = repo.put_wave(&w).unwrap();
        // As an earlier Neumetik kept it: version 1, differences packed by lz.
        let mut old = Vec::from(*MAGIC);
        old.extend_from_slice(&[1, 8, 1, 0]);
        old.extend_from_slice(&(w.frames() as u64).to_le_bytes());
        old.extend_from_slice(&lz::compress(&lz::delta_pack(&w.pcm, 1, 8)));
        repo.write_wave_file(&id, &old).unwrap();
        assert_eq!(repo.wave(&id).unwrap(), w);
        let r = repo.repack(&mut |_| {}).unwrap();
        assert_eq!((r.tried, r.repacked), (1, 1));
        assert!(r.saved > 0);
        assert_eq!(repo.wave(&id).unwrap(), w);
        assert_eq!(repo.repack(&mut |_| {}).unwrap().tried, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_waveform_is_kept_once_however_many_modules_use_it() {
        let dir = temp("dedupe");
        let mut repo = Repository::open(&dir).unwrap();
        let kick: Vec<i16> = (0..2000).map(|i| ((i * 7) % 200 - 100) as i16).collect();
        let a = module(vec![smp("kick", kick.clone(), Loop::default()), smp("bass", vec![1, 2, 3, 4, 5, 6], Loop::forward(0, 6))]);
        let s1 = repo.scan(&a, b"module a", "a.mod", Some("/x/a.mod")).unwrap();
        assert_eq!((s1.new_waves, s1.new_samples, s1.matched()), (2, 2, 0));
        // The same kick, padded with silence, and the bass looped elsewhere.
        let mut padded = kick.clone();
        padded.extend([0; 300]);
        let b = module(vec![smp("BD 01", padded, Loop::default()), smp("bass", vec![1, 2, 3, 4, 5, 6], Loop::forward(2, 6))]);
        let s2 = repo.scan(&b, b"module b", "b.mod", None).unwrap();
        assert_eq!((s2.new_waves, s2.new_samples, s2.matched()), (0, 1, 2));
        assert_eq!(s2.bytes_added, 0);
        assert_eq!(repo.db.waves.len(), 2);
        assert_eq!(repo.db.samples.len(), 3);
        let kick_rec = &repo.db.samples[&s1.slots[0].sample];
        assert_eq!(kick_rec.names, vec!["kick", "BD 01"]);
        assert_eq!(kick_rec.sources.len(), 2);
        assert_eq!(kick_rec.guessed, "Kick");
        // Scanning a module again is known.
        assert!(repo.scan(&a, b"module a", "a copy.mod", None).unwrap().seen_before);
        // It all reads back.
        let again = Repository::open(&dir).unwrap();
        assert_eq!(again.db, repo.db);
        assert_eq!(again.wave(&s1.slots[0].wave).unwrap().pcm, kick.iter().map(|v| v << 8).collect::<Vec<_>>());
        assert!(again.storage().bytes > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn umf_users_are_repointed_and_like_samples_are_found() {
        let dir = temp("repoint");
        let mut repo = Repository::open(&dir).unwrap();
        let tone = |k: f32, n: usize| -> Vec<i16> { (0..n).map(|i| ((i as f32 * k).sin() * 100.0) as i16).collect() };
        let m = module(vec![smp("low", tone(0.05, 4000), Loop::default()), smp("high", tone(0.4, 4000), Loop::default()), smp("noise", (0..4000).map(|i| ((i * 7919 % 251) as i16) - 125).collect(), Loop::default())]);
        let scan = repo.scan(&m, b"m", "m.mod", None).unwrap();
        let (a, b) = (scan.slots[0].sample, scan.slots[1].sample);
        // A song saved as UMF, carrying its waveforms, noted.
        let refs = scan.refs(3);
        let path = dir.join("song.umf");
        std::fs::write(&path, crate::umf::write(&m, &refs, &[], &crate::umf::About::default(), true)).unwrap();
        repo.index_umfs(&dir).unwrap();
        assert_eq!(repo.db.umfs_using(&a).len(), 1);
        let (done, failed) = repo.repoint(&a, &b).unwrap();
        assert_eq!((done.len(), failed.len()), (1, 0));
        let back = crate::umf::read(&std::fs::read(&path).unwrap(), &mut |_| None).unwrap();
        assert!(back.missing.is_empty());
        assert_eq!(back.module.samples[0].wave.pcm, back.module.samples[1].wave.pcm);
        assert_eq!(back.module.samples[0].name, "low");
        assert!(repo.db.umfs_using(&a).is_empty());
        // Found by sound: the high tone, a fifth up and resampled, sounds more like itself than the noise.
        let mut repo = Repository::open(&dir).unwrap();
        let resampled = module(vec![smp("high again", tone(0.6, 2667), Loop::default())]);
        let s2 = repo.scan(&resampled, b"r", "r.mod", None).unwrap();
        let near = repo.similar(&s2.slots[0].sample, 5, &mut |_| {}).unwrap();
        assert_eq!(near[0].0, b, "{near:?}");
        // Filing several at once, and renaming a tag.
        let n = repo.catalog_many(&[a, b], &Filing { tags: vec!["keep".into()], ..Default::default() }, &Filing::default(), Some(3)).unwrap();
        assert_eq!(n, 2);
        assert_eq!(repo.rename_tag("keep", "kept", false).unwrap(), 2);
        assert!(repo.db.samples[&a].catalog.tags.contains(&"kept".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_waveform_is_refused() {
        let w = Wave::new(16, 2, (0..1000).map(|i| (i * 31) as i16).collect());
        let mut packed = pack_wave(&w);
        assert_eq!(unpack_wave(&packed, Some(wave_id(&w))).unwrap(), w);
        let n = packed.len();
        packed[n - 3] ^= 0xFF;
        assert!(unpack_wave(&packed, Some(wave_id(&w))).is_err());
    }
}
