//! **Synth patches**: the instruments of every kind of sound synthesis
//! Neumetik plays, kept once each beside the samples, so a song's synth
//! sounds can be found and used again.
//!
//! A patch is its kind (the synth or format that plays it), its bytes
//! (the format's own, names left out) and the samples it plays from, by
//! ID. Two are the same when all three are (`canon::patch_id`).
//!
//! Some kinds a UMF song plays note by note, as one of its samples: the
//! oscillator synth (`cycle`), an AdLib FM instrument (`opl`), an OctaMED
//! synth sound (`med`), a JamCracker AM wave (`jam`) and a Pumatracker
//! instrument (`puma`). The rest play only inside their own song's
//! replayer (Musicline's `ml`, ProTrekkr's `ptk`, and each synth or chip
//! tracker by its format's id: `ahx`, `kt`, `v2m`...), so they're kept
//! for when they can play on their own.
//!
//! A patch's file (`patches/5c/5c1d...umfp`):
//!
//! ```text
//! UMFP                4 bytes
//! version             u8: 1
//! kind                u8 length, then its letters
//! samples             u16 count, then each sample's ID (32 bytes)
//! data                its bytes, packed by lz.rs
//! ```

use std::sync::Arc;

use super::canon::{patch_id, Id};
use crate::lz;
use crate::model::{Module, Sample, Sound};

pub const MAGIC: &[u8; 4] = b"UMFP";
/// The most a patch's bytes may unpack to.
const MOST: usize = 64 << 20;

/// A kind of patch: its id, its name, and whether a UMF song can play
/// it as one of its samples.
pub struct Kind {
    pub id: &'static str,
    pub name: &'static str,
    pub playable: bool,
}

/// The kinds a song's samples can be.
pub const SAMPLE_KINDS: &[Kind] = &[
    Kind {
        id: "cycle",
        name: "Oscillator synth",
        playable: true,
    },
    Kind {
        id: "opl",
        name: "AdLib FM (OPL)",
        playable: true,
    },
    Kind {
        id: "med",
        name: "OctaMED synth sound",
        playable: true,
    },
    Kind {
        id: "jam",
        name: "JamCracker AM",
        playable: true,
    },
    Kind {
        id: "puma",
        name: "Pumatracker",
        playable: true,
    },
];

pub fn playable(kind: &str) -> bool {
    SAMPLE_KINDS.iter().any(|k| k.id == kind && k.playable)
}

/// A kind's name, or none for a song format's (the module's format name says it).
pub fn kind_name(kind: &str) -> Option<&'static str> {
    SAMPLE_KINDS.iter().find(|k| k.id == kind).map(|k| k.name)
}

/// A patch found in a module.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub kind: String,
    /// Its slot in the module: a sample's for the kinds samples are, an
    /// instrument's in its format's numbering for the rest (from 1).
    pub slot: u16,
    pub name: String,
    pub data: Vec<u8>,
    /// The module's samples it plays from (0-based).
    pub samples: Vec<u16>,
}

/// Every synth patch in a module: its samples that are synths, and the
/// instruments of a song that plays by its own replayer.
pub fn found(m: &Module) -> Vec<Found> {
    let mut out = Vec::new();
    for (i, s) in m.samples.iter().enumerate() {
        out.extend(of_sample(s, i));
    }
    let mut song = |kind: &str, patches: Vec<neumetik_synth::replay::Patch>| {
        out.extend(patches.into_iter().map(|p| Found {
            kind: kind.to_string(),
            slot: p.slot,
            name: p.name,
            data: p.data,
            samples: p.samples,
        }));
    };
    if let Some(ml) = &m.musicline {
        song("ml", ml.patches());
    }
    if let Some(ptk) = &m.ptk {
        song("ptk", ptk.patches());
    }
    if let Some(t) = &m.tune {
        song(t.0.format(), t.0.patches());
    }
    out
}

/// A sample's synth, when it's one (sample `i`, 0-based).
fn of_sample(s: &Sample, i: usize) -> Option<Found> {
    let slot = (i + 1) as u16;
    let (kind, data, samples) = if let Some(fm) = s.fm {
        ("opl", fm.to_s3m().to_vec(), Vec::new())
    } else if let Some(p) = s.program.as_deref() {
        match &p.sound {
            Sound::Med(y) => {
                let mut d = vec![u8::from(y.hybrid), y.vol_speed, y.wave_speed];
                for t in [&y.volumes, &y.waves] {
                    d.extend_from_slice(&(t.len() as u16).to_le_bytes());
                    d.extend_from_slice(t);
                }
                // A hybrid's waveform 0 is the sample's own: it's named, not kept here.
                let skip = usize::from(y.hybrid);
                d.extend_from_slice(&(y.waveforms.len() as u16).to_le_bytes());
                for (k, f) in y.waveforms.iter().enumerate() {
                    let f: &[i8] = if k < skip { &[] } else { f };
                    d.extend_from_slice(&(f.len() as u16).to_le_bytes());
                    d.extend(f.iter().map(|&x| x as u8));
                }
                ("med", d, if y.hybrid { vec![i as u16] } else { Vec::new() })
            }
            Sound::Jam => ("jam", Vec::new(), vec![i as u16]),
            Sound::Puma(y) => {
                let mut d = Vec::new();
                for script in [&y.volume, &y.pitch] {
                    d.extend_from_slice(&(script.len() as u16).to_le_bytes());
                    for c in script.iter() {
                        d.extend_from_slice(c);
                    }
                }
                ("puma", d, p.sources.clone())
            }
        }
    } else if let Some(y) = s.synth {
        ("cycle", vec![y.wave, y.width, y.ratio, y.depth, y.beat, y.soften], Vec::new())
    } else {
        return None;
    };
    Some(Found {
        kind: kind.to_string(),
        slot,
        name: s.name.trim().to_string(),
        data,
        samples,
    })
}

/// A patch as kept: its kind, the samples it plays (by ID) and its bytes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Patch {
    pub kind: String,
    pub samples: Vec<Id>,
    pub data: Vec<u8>,
}

impl Patch {
    pub fn id(&self) -> Id {
        patch_id(&self.kind, &self.samples, &self.data)
    }

    /// Its file.
    pub fn pack(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(16 + self.samples.len() * 32 + self.data.len());
        o.extend_from_slice(MAGIC);
        o.push(1);
        o.push(self.kind.len() as u8);
        o.extend_from_slice(self.kind.as_bytes());
        o.extend_from_slice(&(self.samples.len() as u16).to_le_bytes());
        for s in &self.samples {
            o.extend_from_slice(&s.0);
        }
        o.extend_from_slice(&lz::compress(&self.data));
        o
    }

    /// Reads its file, checked against `id` when given.
    pub fn unpack(b: &[u8], id: Option<Id>) -> Result<Patch, String> {
        let bad = || "it isn't a patch".to_string();
        if b.len() < 8 || &b[..4] != MAGIC {
            return Err(bad());
        }
        if b[4] != 1 {
            return Err("it's a newer kind of patch".into());
        }
        let n = usize::from(b[5]);
        let kind = String::from_utf8_lossy(b.get(6..6 + n).ok_or_else(bad)?).into_owned();
        let mut at = 6 + n;
        let k = usize::from(u16::from_le_bytes(b.get(at..at + 2).ok_or_else(bad)?.try_into().expect("two bytes")));
        at += 2;
        let samples = (0..k)
            .map(|i| b.get(at + i * 32..at + i * 32 + 32).map(|s| Id(s.try_into().expect("32 bytes"))))
            .collect::<Option<Vec<Id>>>()
            .ok_or_else(bad)?;
        at += k * 32;
        let data = lz::decompress(&b[at..], MOST).map_err(|_| "its bytes are damaged".to_string())?;
        let p = Patch { kind, samples, data };
        if id.is_some_and(|id| p.id() != id) {
            return Err("it doesn't match its ID".into());
        }
        Ok(p)
    }
}

/// A playable patch made into a sample for a song: `samples` are the
/// samples it plays from, as the song will have them (their indices in
/// the song, 0-based, for a Pumatracker one's sources). None for a kind
/// a song can't play, or bytes that don't read.
pub fn to_sample(p: &Patch, name: &str, samples: &[(u16, &Sample)]) -> Option<Sample> {
    use crate::model::{FmPatch, Program, PumaSound, Synth, SynthSound};
    let d = &p.data;
    let mut s = Sample::empty();
    s.name = name.to_string();
    match p.kind.as_str() {
        "cycle" => {
            let b: [u8; 6] = d.get(..6)?.try_into().ok()?;
            let y = Synth {
                wave: b[0],
                width: b[1],
                ratio: b[2],
                depth: b[3],
                beat: b[4],
                soften: b[5],
            };
            s = crate::sample_edit::synth(&y);
            s.name = name.to_string();
        }
        "opl" => s.fm = Some(FmPatch::from_s3m(&d.get(..11)?.try_into().ok()?)),
        "jam" => {
            let (_, from) = samples.first()?;
            s.wave = from.wave.clone();
            s.c5 = from.c5;
            s.looped = from.looped;
            s.program = Some(Arc::new(Program::jam(&s.wave)));
        }
        "med" => {
            let mut r = Bytes(d, 0);
            let (hybrid, vol_speed, wave_speed) = (r.u8()? != 0, r.u8()?, r.u8()?);
            let volumes = r.table()?;
            let waves = r.table()?;
            let n = usize::from(r.u16()?);
            let mut waveforms = Vec::with_capacity(n.min(64));
            for _ in 0..n {
                let k = usize::from(r.u16()?);
                waveforms.push(r.take(k)?.iter().map(|&b| b as i8).collect::<Vec<i8>>());
            }
            if hybrid {
                let (_, from) = samples.first()?;
                s.wave = from.wave.clone();
                s.c5 = from.c5;
                s.looped = from.looped;
            }
            let sound = SynthSound {
                waveforms,
                volumes,
                waves,
                vol_speed,
                wave_speed,
                hybrid,
            };
            s.program = Some(Arc::new(Program::new(sound, &s.wave)));
        }
        "puma" => {
            let mut r = Bytes(d, 0);
            let mut script = || -> Option<Vec<[u8; 4]>> {
                let k = usize::from(r.u16()?);
                Some(r.take(k * 4)?.chunks_exact(4).map(|b| [b[0], b[1], b[2], b[3]]).collect())
            };
            let volume = script()?;
            let pitch = script()?;
            let sources: Vec<u16> = samples.iter().map(|&(i, _)| i).collect();
            let mut all: Vec<Sample> = Vec::new();
            for &(i, from) in samples {
                let i = usize::from(i);
                if all.len() <= i {
                    all.resize(i + 1, Sample::empty());
                }
                all[i] = from.clone();
            }
            s.program = Some(Arc::new(Program::puma(PumaSound { volume, pitch }, sources, &all)));
        }
        _ => return None,
    }
    Some(s)
}

struct Bytes<'a>(&'a [u8], usize);

impl Bytes<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let b = self.0.get(self.1..self.1 + n)?;
        self.1 += n;
        Some(b)
    }

    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }

    fn table(&mut self) -> Option<Vec<u8>> {
        let n = usize::from(self.u16()?);
        Some(self.take(n)?.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patch_file_reads_back_and_is_checked_against_its_id() {
        let p = Patch {
            kind: "ahx".into(),
            samples: vec![Id([7; 32])],
            data: (0..300).map(|i| (i % 13) as u8).collect(),
        };
        let b = p.pack();
        assert_eq!(Patch::unpack(&b, Some(p.id())).unwrap(), p);
        assert!(Patch::unpack(&b, Some(Id([1; 32]))).is_err());
        let other = Patch { kind: "hvl".into(), ..p.clone() };
        assert_ne!(other.id(), p.id());
    }
}
