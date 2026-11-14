// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! **Hydrogen** drum patterns and songs (`.h2pattern`, `.h2song`: XML).
//!
//! Written from Hydrogen's published documents (the manual's file types and
//! the "Song format" wiki page) and from what its files look like; Hydrogen's
//! source (GPL) was not read and Hydrogen was not run, so this is untested
//! against it. Ticks are Hydrogen's: 192 to a whole note (48 to a quarter);
//! a pattern's `size` is its length in ticks and `denominator` (4 when
//! absent) its meter's lower number, so a size of 144 over 8 is 6/8.
//!
//! **Reading:** a pattern file is its one pattern; a song is its pattern
//! sequence (each group's patterns together, one group after another), or
//! its first pattern if it has no sequence. A drum is named by the song's
//! instrument list (Kick, Snare Rock, Hat Closed, Tom Low ...); a pattern
//! file names none, so its instrument ids are read as the places in
//! Hydrogen's default kit ([`DEFAULT_KIT`], GMRockKit's list). A note's `type` ("Hi-hat Closed", "Snare Rimshot"...), which newer
//! pattern files give each note, names its drum first. A song may not
//! change meter (a group not a whole number of the first bar long is
//! refused: a [`Pattern`] has one meter), and an instrument whose name is
//! no drum Neumetik knows is refused by name. A note's velocity (0 to 1) is
//! a soft hit, a hit or an accent; probability, lead and lag, pitch and note
//! offs are left out (a note off is skipped).
//!
//! **Writing:** a `.h2pattern` of one pattern for the default kit. Hydrogen's
//! kit has 15 different sounds, so a drum within 3 keys of one of them
//! takes it and any other is refused.

use super::names;
use super::read::Opts;
use super::{level_of, Pattern, REST};
use crate::text::xml::{self, Element};

type R<T> = Result<T, String>;

/// Hydrogen's default kit (GMRockKit, `data/drumkits/GMRockKit/drumkit.xml` in Hydrogen's repository: read as data on 2026-11-14) as
/// General MIDI keys, by instrument id: Kick, Stick, Snare, Hand Clap, Snare Rimshot, Floor Tom, Hat Closed, Tom 2, Hat Pedal, Tom 1,
/// Hat Open, Cowbell, Ride, Crash, Ride 2, Splash, Hat Semi-Open, Bell.
pub const DEFAULT_KIT: [u8; 18] = [36, 37, 38, 39, 38, 43, 42, 47, 44, 50, 46, 56, 51, 49, 51, 55, 46, 53];

/// A Hydrogen instrument's name as a General MIDI drum key.
pub fn key_of(name: &str) -> Option<u8> {
    let n: String = name.to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ').collect();
    let has = |s: &str| n.contains(s);
    let k = if has("kick") || has("bass drum") || has("bassdrum") || n.trim() == "bd" {
        36
    } else if has("snare") && !has("side") {
        38
    } else if has("stick") || has("rim") {
        37
    } else if has("clap") {
        39
    } else if has("hat") || has("hihat") || n.starts_with("hh") {
        if has("open") {
            46
        } else if has("pedal") || has("foot") {
            44
        } else {
            42
        }
    } else if has("tom") {
        if has("floor") {
            if has("hi") {
                43
            } else {
                41
            }
        } else if has("hi") || n.ends_with("1") {
            50
        } else if has("mid") || n.ends_with("2") {
            47
        } else {
            45
        }
    } else if has("crash") || has("cymbal") {
        49
    } else if n.trim() == "bell" {
        53
    } else if has("china") {
        52
    } else if has("splash") {
        55
    } else if has("ride") {
        if has("bell") {
            53
        } else {
            51
        }
    } else if has("cowbell") {
        56
    } else if has("tamb") {
        54
    } else if has("shaker") || has("maraca") {
        70
    } else if has("cabasa") {
        69
    } else if has("clave") {
        75
    } else {
        return names::key_from_name(name);
    };
    Some(k)
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// A pattern element's tick length and denominator.
fn size_of(p: &Element) -> R<(u32, u32)> {
    let size = p.num("size").map_or(192.0, |s| s);
    let den = p.num("denominator").map_or(4.0, |s| s);
    if size < 1.0 || size.fract() != 0.0 || !matches!(den as u32, 1 | 2 | 4 | 8 | 16 | 32) || den.fract() != 0.0 {
        return Err(format!("a pattern of size {size} over {den} is not a meter"));
    }
    Ok((size as u32, den as u32))
}

/// Reads a `.h2pattern` or `.h2song`.
pub fn read(text: &str, o: &Opts) -> R<Pattern> {
    let root = xml::parse(text).map_err(|e| format!("not Hydrogen's XML: {e}"))?;
    // Instrument names by id (a song's list).
    let mut inst: Vec<(u32, String)> = Vec::new();
    root.walk(&mut |e| {
        if e.name == "instrument" {
            if let (Some(id), Some(name)) = (e.num("id"), e.get("name")) {
                inst.push((id as u32, name));
            }
        }
    });
    let mut patterns: Vec<&Element> = Vec::new();
    // `pattern` elements that hold notes; they sit in a patternList or are the file's own.
    fn collect<'a>(e: &'a Element, out: &mut Vec<&'a Element>) {
        if e.name == "pattern" && e.child("noteList").is_some() {
            out.push(e);
        }
        for c in e.elements() {
            collect(c, out);
        }
    }
    collect(&root, &mut patterns);
    if patterns.is_empty() {
        return Err("no pattern in it".into());
    }
    let name_of = |p: &Element| p.get("name").or_else(|| p.get("pattern_name")).unwrap_or_default();
    // The order they play in.
    let mut groups: Vec<Vec<&Element>> = Vec::new();
    let mut seq_el: Option<&Element> = None;
    fn find_seq<'a>(e: &'a Element, out: &mut Option<&'a Element>) {
        if e.name == "patternSequence" && out.is_none() {
            *out = Some(e);
        }
        for c in e.elements() {
            find_seq(c, out);
        }
    }
    find_seq(&root, &mut seq_el);
    if let Some(s) = seq_el {
        for g in s.all("group") {
            let mut members = Vec::new();
            for id in g.all("patternID") {
                let t = id.text();
                let found = patterns.iter().find(|p| name_of(p) == t).or_else(|| t.parse::<usize>().ok().and_then(|i| patterns.get(i)));
                members.push(*found.ok_or_else(|| format!("the sequence plays a pattern '{t}' the song doesn't have"))?);
            }
            if !members.is_empty() {
                groups.push(members);
            }
        }
    }
    if groups.is_empty() {
        groups.push(vec![patterns[0]]);
    }
    let (first, den) = size_of(groups[0][0])?;
    if first * den % 192 != 0 {
        return Err(format!("a pattern of size {first} over {den} is not a whole number of beats"));
    }
    let total_beats = first * den / 192;
    // Hydrogen stores a pattern's length, not its bars: the meter is said, or
    // eight quarters are two bars of 4/4, or the whole length is a bar.
    let beats = if o.meter_set() && u32::from(o.unit) == den && total_beats % u32::from(o.beats) == 0 {
        u32::from(o.beats)
    } else if den == 4 && total_beats > 4 && total_beats % 4 == 0 {
        4
    } else {
        total_beats
    };
    if beats == 0 || beats > 64 {
        return Err(format!("{beats} beats to a bar is not a meter"));
    }
    let bar = beats * 192 / den;
    // Every note as (tick in the song, key, level).
    let mut notes: Vec<(u32, u8, u8)> = Vec::new();
    let mut at = 0u32;
    for g in &groups {
        let mut len = 0;
        for p in g {
            let (size, d) = size_of(p)?;
            if d != den {
                return Err(format!("this changes meter ({}/{} then {}/{}): a pattern has one meter", beats, den, size * d / 192, d));
            }
            len = len.max(size);
            for n in p.all("noteList").flat_map(|l| l.all("note")) {
                if n.get("note_off").is_some_and(|t| t == "true") {
                    continue;
                }
                let (Some(pos), Some(id)) = (n.num("position"), n.num("instrument")) else { continue };
                if pos < 0.0 || pos.fract() != 0.0 || id < 0.0 {
                    continue;
                }
                let id = id as u32;
                // A pattern file's instrument ids index the kit it names, which the file doesn't carry; each note's `type` says what it is.
                let typed = n.get("type").filter(|t| !t.trim().is_empty()).and_then(|t| key_of(&t));
                let key = match inst.iter().find(|i| i.0 == id) {
                    _ if typed.is_some() && inst.is_empty() => typed.unwrap(),
                    Some((_, name)) => key_of(name).ok_or_else(|| format!("'{name}' is not a drum Neumetik knows"))?,
                    None if inst.is_empty() => *DEFAULT_KIT.get(id as usize).ok_or_else(|| format!("instrument {id} is past Hydrogen's default kit, and the file names none"))?,
                    None => return Err(format!("a note plays instrument {id}, which the song doesn't list")),
                };
                let vel = n.num("velocity").unwrap_or(0.8).clamp(0.0, 1.0);
                if pos as u32 >= size {
                    continue;
                }
                notes.push((at + pos as u32, key, level_of((vel * 127.0).round() as u8).max(1)));
            }
        }
        if len % bar != 0 {
            return Err(format!("this changes meter: a part {len} ticks long is not whole bars of {bar}"));
        }
        at += len;
    }
    let bars = at / bar;
    if bars == 0 || bars > 256 {
        return Err(format!("{bars} bars is more than a pattern holds"));
    }
    let g = notes.iter().fold(bar, |g, n| gcd(g, n.0));
    let steps = bar / g;
    let mut pat = Pattern::new(beats as u8, den as u8, bars, steps);
    for (t, key, level) in notes {
        pat.put(key, i64::from(t / g), level);
    }
    if let Some(n) = patterns.first().map(|p| name_of(p)).filter(|n| !n.is_empty()) {
        pat.title = n;
    }
    Ok(pat)
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The nearest sound of Hydrogen's default kit to a key, as its instrument id.
fn instrument_for(key: u8) -> Option<usize> {
    let best = DEFAULT_KIT.iter().enumerate().min_by_key(|(i, k)| (k.abs_diff(key), *i))?;
    (best.1.abs_diff(key) <= 3).then_some(best.0)
}

/// A `.h2pattern` for Hydrogen's default kit.
pub fn write(p: &Pattern) -> R<String> {
    let tpb = 192 / u32::from(p.unit);
    let bar = u32::from(p.beats) * tpb;
    let mut notes = String::new();
    let mut count = 0;
    for l in p.shown() {
        let id = instrument_for(l.key).ok_or_else(|| format!("Hydrogen's default kit has nothing near {}", names::name(l.key)))?;
        for (c, level) in l.cells.iter().enumerate() {
            if *level == REST {
                continue;
            }
            let (b, s) = (c as u32 / p.steps, c as u32 % p.steps);
            let pos = b * bar + s * bar / p.steps;
            let vel = f64::from(super::velocity(*level)) / 127.0;
            notes.push_str(&format!("   <note>\n    <position>{pos}</position>\n    <leadlag>0</leadlag>\n    <velocity>{vel:.3}</velocity>\n    <pan>0</pan>\n    <pitch>0</pitch>\n    <key>C</key>\n    <length>-1</length>\n    <instrument>{id}</instrument>\n    <note_off>false</note_off>\n    <probability>1</probability>\n   </note>\n"));
            count += 1;
        }
    }
    if count == 0 {
        return Err("there is nothing to write".into());
    }
    let title = if p.title.is_empty() { "Rhythm".to_string() } else { escape(&p.title) };
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<drumkit_pattern xmlns=\"http://www.hydrogen-music.org/drumkit_pattern\">\n <drumkit_name>GMRockKit</drumkit_name>\n <author>Neumetik</author>\n <info></info>\n <license>undefined license</license>\n <pattern>\n  <name>{title}</name>\n  <category>unknown_category</category>\n  <size>{}</size>\n  <denominator>{}</denominator>\n  <noteList>\n{notes}  </noteList>\n </pattern>\n</drumkit_pattern>\n",
        bar * p.bars,
        p.unit
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::rhythm::read::parse;
    use crate::text::rhythm::Notation;

    fn note(pos: u32, vel: f32, inst: u32) -> String {
        format!("<note><position>{pos}</position><leadlag>0</leadlag><velocity>{vel}</velocity><pan>0</pan><pitch>0</pitch><key>C</key><length>-1</length><instrument>{inst}</instrument><note_off>false</note_off></note>")
    }

    #[test]
    fn a_song_reads_by_its_instrument_names_and_sequence() {
        let a: String = [note(0, 0.9, 0), note(96, 0.8, 1), note(0, 0.5, 2), note(48, 0.5, 2), note(96, 0.5, 2), note(144, 0.5, 2)].concat();
        let b: String = note(0, 0.8, 0);
        let song = format!(
            "<?xml version=\"1.0\"?><song><version>1.2</version><bpm>90</bpm><instrumentList>\
             <instrument><id>0</id><name>Kick</name></instrument><instrument><id>1</id><name>Snare Rock</name></instrument>\
             <instrument><id>2</id><name>Hat Closed</name></instrument></instrumentList>\
             <patternList><pattern><name>groove</name><category/><size>192</size><noteList>{a}</noteList></pattern>\
             <pattern><name>end</name><size>192</size><noteList>{b}</noteList></pattern></patternList>\
             <patternSequence><group><patternID>groove</patternID></group><group><patternID>groove</patternID></group><group><patternID>end</patternID></group></patternSequence></song>"
        );
        let p = parse(&song, None, &Opts::default()).expect("song");
        assert_eq!((p.beats, p.unit, p.bars, p.steps), (4, 4, 3, 4));
        let cells = |k: u8| -> Vec<u8> { p.lanes.iter().find(|l| l.key == k).map(|l| l.cells.clone()).unwrap_or_default() };
        assert_eq!(cells(42), [1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0]);
        assert_eq!(cells(38), [0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 0, 0]);
        assert_eq!(cells(36), [3, 0, 0, 0, 3, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(p.title, "groove");
    }

    #[test]
    fn the_two_shipped_kits_name_known_drums() {
        // The instrument names of Hydrogen's GMRockKit and TR808EmulationKit (their drumkit.xml, read as data), as GM keys.
        let gm = [("Kick", 36), ("Stick", 37), ("Snare", 38), ("Hand Clap", 39), ("Snare Rimshot", 38), ("Floor Tom", 41), ("Hat Closed", 42), ("Tom 2", 47), ("Hat Pedal", 44), ("Tom 1", 50), ("Hat Open", 46), ("Cowbell", 56), ("Ride", 51), ("Crash", 49), ("Ride 2", 51), ("Splash", 55), ("Hat Semi-Open", 46), ("Bell", 53)];
        for (name, key) in gm {
            assert_eq!(key_of(name), Some(key), "{name}");
        }
        for name in ["Kick Long", "Kick Short", "Snare 1", "Snare 2", "Clap", "Tom Low", "Tom Mid", "Tom Hi", "Closed Hat", "Pedal Hat", "Open Hat", "Cymbal", "Shaker", "Conga", "Clave", "Cowbell"] {
            assert!(key_of(name).is_some(), "{name}");
        }
    }

    #[test]
    fn a_pattern_notes_type_names_its_drum() {
        // Hydrogen 1.2's pattern files: the instrument id indexes the kit the file names (not in it), `type` says what it is.
        let n = |pos: u32, inst: u32, ty: &str| format!("<note><position>{pos}</position><velocity>0.8</velocity><instrument>{inst}</instrument><type>{ty}</type><note_off>false</note_off></note>");
        let notes: String = [n(0, 0, "Kick"), n(48, 4, "Snare Rimshot"), n(96, 16, "Hi-hat Semi-Open"), n(144, 12, "Ride Bow"), n(0, 99, "Tom Floor"), n(96, 9, "Ride Bell")].concat();
        let f = format!("<drumkit_pattern><drumkit_name>GMRockKit</drumkit_name><pattern><name>t</name><size>192</size><denominator>4</denominator><noteList>{notes}</noteList></pattern></drumkit_pattern>");
        let p = parse(&f, None, &Opts::default()).expect("pattern");
        let keys: Vec<u8> = p.lanes.iter().map(|l| l.key).collect();
        for k in [36, 38, 46, 51, 41, 53] {
            assert!(keys.contains(&k), "{k} in {keys:?}");
        }
    }

    /// Real files: H2_DIR a folder of `.h2pattern` and `.h2song` (nothing is checked in: their licences vary).
    #[test]
    #[ignore]
    fn reads_real_hydrogen_files() {
        let Ok(dir) = std::env::var("H2_DIR") else { return };
        let mut paths: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "h2pattern" || e == "h2song")).collect();
        paths.sort();
        let (mut ok, mut bad) = (0, 0);
        for path in paths {
            let text = std::fs::read_to_string(&path).unwrap();
            match parse(&text, None, &Opts::default()) {
                Ok(p) => {
                    ok += 1;
                    eprintln!("{}: {}/{} x {} bars, {} hits, {} drums", path.file_name().unwrap().to_string_lossy(), p.beats, p.unit, p.bars, p.hits(), p.lanes.len());
                }
                Err(e) => {
                    bad += 1;
                    eprintln!("{}: {e}", path.file_name().unwrap().to_string_lossy());
                }
            }
        }
        eprintln!("{ok} read, {bad} refused");
    }

    #[test]
    fn a_pattern_file_uses_the_default_kit_and_its_meter() {
        let n: String = [note(0, 0.8, 0), note(24, 0.8, 6), note(72, 0.8, 4)].concat();
        let f = format!("<drumkit_pattern><drumkit_name>x</drumkit_name><pattern><pattern_name>six</pattern_name><size>144</size><denominator>8</denominator><noteList>{n}</noteList></pattern></drumkit_pattern>");
        assert_eq!(crate::text::rhythm::read::detect(&f), Some(Notation::Hydrogen));
        let p = parse(&f, None, &Opts::default()).expect("pattern");
        assert_eq!((p.beats, p.unit, p.bars, p.title.as_str()), (6, 8, 1, "six"));
        assert_eq!(p.hits(), 3);
        assert!(p.lanes.iter().any(|l| l.key == 38) && p.lanes.iter().any(|l| l.key == 42));
        // Two bars' length is two bars of 4/4, unless the meter is said.
        let two = f.replace("144", "384").replace("<denominator>8", "<denominator>4");
        assert_eq!(parse(&two, None, &Opts::default()).map(|p| (p.beats, p.bars)), Ok((4, 2)));
        let said = Opts { beats: 8, unit: 4, ..Opts::default() };
        assert_eq!(parse(&two, None, &said).map(|p| (p.beats, p.bars)), Ok((8, 1)));
    }

    #[test]
    fn what_it_cannot_read_it_says() {
        let n = note(0, 0.8, 0);
        let bad = format!("<song><instrumentList><instrument><id>0</id><name>Weird Sample</name></instrument></instrumentList><patternList><pattern><name>a</name><size>192</size><noteList>{n}</noteList></pattern></patternList></song>");
        assert!(parse(&bad, Some(Notation::Hydrogen), &Opts::default()).unwrap_err().contains("Weird Sample"));
        let meter = format!("<song><patternList><pattern><name>a</name><size>192</size><noteList>{n}</noteList></pattern><pattern><name>b</name><size>144</size><noteList>{n}</noteList></pattern></patternList><patternSequence><group><patternID>a</patternID></group><group><patternID>b</patternID></group></patternSequence></song>");
        assert!(parse(&meter, Some(Notation::Hydrogen), &Opts::default()).unwrap_err().contains("meter"));
        assert!(parse("<song></song>", Some(Notation::Hydrogen), &Opts::default()).is_err());
    }

    #[test]
    fn a_written_pattern_reads_back() {
        let p = parse("kick: x...x...x...x...\nsnare: ....x.......x...\nhat: x-x-x-x-x-x-x-x-", None, &Opts::default()).expect("rock");
        let h = write(&p).expect("writes");
        assert!(h.contains("<size>192</size>") && h.contains("<denominator>4</denominator>"), "{h}");
        let q = parse(&h, None, &Opts::default()).unwrap_or_else(|e| panic!("{e}\n{h}"));
        let wh = |x: &Pattern| x.notes().into_iter().map(|n| (n.0, n.2, n.3 / 40)).collect::<Vec<_>>();
        assert_eq!(wh(&q), wh(&p));
        // A drum the kit has nothing near is refused.
        let conga = parse("conga: x...", None, &Opts::default()).expect("conga");
        assert!(write(&conga).is_err());
    }
}
