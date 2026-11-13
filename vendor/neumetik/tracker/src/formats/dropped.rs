//! The formats Neumetik doesn't play, why, and what plays them instead.
//! Each was planned and dropped because nothing licence-clear describes
//! or plays it: its only replayer is a rip or a disassembly of a
//! commercial program, is source with no licence, is GPL, or is a
//! program that's still sold. A file of one, which no format reads, is
//! named in the error with the reason and where it plays.
//!
//! Keep in step with `docs/dropped-formats.md`, which has each one's
//! sources and the research behind it: a test checks every name here is
//! there.

/// A format Neumetik doesn't play.
pub struct Dropped {
    pub name: &'static str,
    /// Its file names' extensions (`song.mdx`) and Amiga prefixes
    /// (`SDR.song`), lowercase.
    pub names: &'static [&'static str],
    /// Why it isn't played.
    pub reason: &'static str,
    /// What plays it.
    pub elsewhere: &'static str,
}

const RIPPED: &str = "its only replayers are rips of the original program, with no licence to use them";
const UADE: &str = "UADE plays it (on macOS and Linux; on Windows, foobar2000's UADE Input), and EaglePlayer with Wanted Team's players does on an Amiga, or in Floppy";

const fn dropped(name: &'static str, names: &'static [&'static str], reason: &'static str, elsewhere: &'static str) -> Dropped {
    Dropped { name, names, reason, elsewhere }
}

pub const DROPPED: &[Dropped] = &[
    // Amiga.
    dropped("Future Player", &["fp"], RIPPED, UADE),
    dropped("Synth Dream", &["sdr"], RIPPED, UADE),
    dropped("Synth Pack", &["osp"], RIPPED, UADE),
    dropped("Dynamic Synthesizer", &["dns"], RIPPED, UADE),
    dropped("Beathoven Synthesizer", &["bss"], "each song carries its game's own ripped replayer, with no licence to use it", UADE),
    dropped(
        "Mark II",
        &["mk2", "mkii"],
        RIPPED,
        "UADE plays it (on macOS and Linux; on Windows, foobar2000's UADE Input), and DeliTracker or EaglePlayer does on an Amiga, or in Floppy",
    ),
    dropped(
        "Medley",
        &["mso"],
        "its author's replayer is free to use but not to build into another program",
        "UADE plays it (on macOS and Linux; on Windows, foobar2000's UADE Input), and DeliTracker does on an Amiga, or in Floppy",
    ),
    dropped("The Musical Enlightenment", &["tme"], RIPPED, UADE),
    dropped(
        "Sound Programming Language",
        &["spl"],
        "its only replayer is ripped from the games it was written for, with no licence to use it",
        UADE,
    ),
    dropped(
        "Quartet",
        &["4v", "sqt"],
        "nothing describes the format, and its only replayers are rips of the Atari ST's, with no licence to use them",
        "UADE plays it (on macOS and Linux; on Windows, foobar2000's UADE Input), and EaglePlayer with Wanted Team's players does on an Amiga, or in Floppy",
    ),
    // PC.
    dropped(
        "Tunefish 2",
        &["tf2m"],
        "the source of its synth was never released (Tunefish 4's, a different synth, is GPL)",
        "the Tunefish 2 instrument plays it in a VST host on Windows",
    ),
    dropped(
        "JayTrax",
        &["jxs"],
        "its replayer's source was published with no licence",
        "rePlayer plays it on Windows, and Cog on macOS",
    ),
    dropped(
        "Ixalance",
        &["ixs"],
        "its only replayer source is a decompiled copy of the commercial player, for non-commercial use",
        "rePlayer plays it on Windows, and webIXS in a browser",
    ),
    dropped(
        "Buzzic",
        &["buz"],
        "neither its replayer's source nor a licence was ever released",
        "Buzzic itself plays it, on Windows",
    ),
    dropped("Buzzic 2", &["buz2"], "its source was published with no licence", "rePlayer plays it on Windows"),
    dropped(
        "DarkWave Studio",
        &["dwp"],
        "its synths' source was GPL, and the program is closed now",
        "DarkWave Studio itself plays it, on Windows",
    ),
    dropped(
        "Mario Paint (Shroom)",
        &["sho"],
        "it plays Mario Paint's own sounds, which are Nintendo's",
        "Mariopants plays it on Windows, macOS and Linux",
    ),
    // Commodore 64, NES and Game Boy.
    dropped(
        "Cybertracker (C64)",
        &["ct"],
        "its editor is closed, and nothing describes the format",
        "CyberTracker on a C64 or in VICE plays it, and its packer makes a .sid any SID player plays",
    ),
    dropped(
        "SID Duzz' It",
        &[],
        "neither a description of the format nor a licensed replayer exists",
        "its songs play as .sid rips (the HVSC collection, DeepSID in a browser)",
    ),
    dropped(
        "NerdTracker II",
        &["ned"],
        "its replayer is for non-commercial use only, and nothing describes the format",
        "NerdTracker II itself plays it, in DOSBox or Floppy",
    ),
    dropped(
        "LSDJ",
        &["lsdsng"],
        "its sound is the Game Boy program itself, which is sold",
        "LSDJ plays it, in a Game Boy emulator (SameBoy) with your own copy",
    ),
    // ZX Spectrum.
    dropped("AY STRC", &["strc"], "nothing describes the format, and nothing plays it today", "no player is known today"),
    dropped(
        "AY Amadeus",
        &["amad"],
        "nothing describes the format, and its only player may be used but not built on",
        "Ay_Emul plays it on Windows, and Vortex Tracker II imports it",
    ),
    dropped(
        "Beepola",
        &["bbsong"],
        "most of its songs play through beeper engines taken from commercial games",
        "Beepola itself plays it, on Windows (or in Wine)",
    ),
    dropped("TFM Music Maker", &["tfe"], "nothing describes the format, and its only open reader is LGPL", "ZXTune plays it"),
    // MSX and the X68000.
    dropped(
        "MoonBlaster",
        &["mbm"],
        "its replayer is a copyrighted MSX program, and nothing describes the format",
        "MSXplay (libkss) plays it, and MoonBlaster itself on an MSX or in openMSX",
    ),
    dropped(
        "SCC-Musixx",
        &[],
        "nothing describes the format, and nothing plays it today but the original on an MSX",
        "SCC-Musixx itself plays it, on an MSX or in openMSX",
    ),
    dropped("OPLLDRV", &["opx"], "it plays only through its copyrighted MSX driver", "MSXplay (libkss) plays it"),
    dropped("MuSICA", &["bgm"], "it plays only through its copyrighted MSX driver (Kinrou5)", "MSXplay (libkss) plays it"),
    dropped(
        "MDX",
        &["mdx"],
        "every player of it comes from a disassembly of the X68000's MXDRV, whose source was never released",
        "foobar2000's MDX input and portable_mdx play it, and mdxplay on an Amiga or in Floppy",
    ),
];

/// The dropped format a file is, when one is known by its contents
/// (Future Player's rips are executables saying "F.PLAYER") or its name.
pub fn recognize(data: &[u8], name: &str) -> Option<&'static Dropped> {
    if data.len() > 44 && data[..4] == [0, 0, 3, 0xF3] && &data[36..44] == b"F.PLAYER" {
        return DROPPED.first();
    }
    let file = name.rsplit(['/', '\\']).next().unwrap_or(name).to_ascii_lowercase();
    let ext = file.rsplit_once('.').map(|(_, e)| e);
    let prefix = file.split_once('.').map(|(p, _)| p);
    DROPPED.iter().find(|d| d.names.iter().any(|n| ext == Some(n) || prefix == Some(n)))
}

impl Dropped {
    /// What a file of it says when opened.
    pub fn explain(&self) -> String {
        format!("it's {}, which Neumetik doesn't play: {}; {}", self.name, self.reason, self.elsewhere)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_by_contents_extension_or_prefix() {
        let mut fp = vec![0u8; 64];
        fp[..4].copy_from_slice(&[0, 0, 3, 0xF3]);
        fp[36..44].copy_from_slice(b"F.PLAYER");
        assert_eq!(recognize(&fp, "whatever").unwrap().name, "Future Player");
        assert_eq!(recognize(b"", "music/Song.MDX").unwrap().name, "MDX");
        assert_eq!(recognize(b"", "SDR.jigsaw").unwrap().name, "Synth Dream");
        assert!(recognize(b"", "song.mod").is_none());
        assert!(recognize(b"", "README").is_none());
    }

    #[test]
    fn every_one_is_in_its_document() {
        let doc = include_str!("../../docs/dropped-formats.md");
        for d in DROPPED {
            assert!(doc.contains(&format!("| {} |", d.name)), "{} is missing from docs/dropped-formats.md", d.name);
        }
    }

    #[test]
    fn says_why_and_where() {
        let e = recognize(b"", "a.jxs").unwrap().explain();
        assert!(e.starts_with("it's JayTrax, which Neumetik doesn't play: "));
        assert!(e.ends_with("; rePlayer plays it on Windows, and Cog on macOS"));
    }
}
