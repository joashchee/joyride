//! **Module tags**: a scanned module filed by what can be told of it,
//! in the faceted, namespaced tags of the music catalog system
//! (`music-catalog-system.md` at the repository root, §17.1: `genre:`,
//! `era:`, `inst:`, `use:`, `license:`...), lowercase and kebab-case
//! with `/` for a genre's path.
//!
//! What they're guessed from, nothing heard:
//!
//! - **genre:** words in its title, file names and SAUCE comments and
//!   the folders of the archive it was fetched from (Aminet files modules
//!   by genre: `mods/techn`, `mods/jungl`...), and, for chiptune, its
//!   samples (most of them single-cycle chip waveforms, little sample
//!   data). Whole words only, so "rocket" isn't rock. Not its message,
//!   which in many formats is only its samples' names ("acid bass",
//!   "rock kit"), and not folders on disk or Modland's, which are named
//!   for formats and artists ("Digital Symphony", "Jungle Jim").
//! - **platform:** the computer its format was made on (§13.6).
//! - **year:** and **era:** its SAUCE date, the year of the party it was
//!   entered at, else a year in its copyright lines or message.
//! - **length:** how long it plays once through, in the catalog's
//!   duration classes (§10).
//! - **inst:** its samples' categories (`analysis.rs`), as instruments (§7).
//! - **use:**, **party:** and **game:** a party's compo folder, a demo
//!   folder, a game's music format, or words in its names that say it's
//!   a game's ("level", "menu", "game over").
//! - **license:** its license's terms (`rights.rs`).
//!
//! They're kept apart from what people say (a module's `guessed` lines),
//! made again whenever [`VERSION`] says the rules have changed, and
//! searched with the samples (`search.rs`: `genre:techno` finds the
//! samples of modules guessed techno).

use std::collections::BTreeMap;

use super::db::{Db, ModuleRec};
use super::rights::{module_license, Terms};

/// The rules' version: a module tagged by older rules is tagged again
/// when the repository opens.
pub const VERSION: u32 = 1;

/// Genre words: whole words, and the genre path each points to (§4).
const GENRE_WORDS: &[(&[&str], &str)] = &[
    (&["chiptune", "chiptunes", "chip", "chipmusic", "8bit", "bitpop"], "electronic/chiptune"),
    (&["techno", "techn", "tekno", "technoid"], "electronic/techno"),
    (&["acid"], "electronic/techno/acid-techno"),
    (&["trance", "tranc", "goa", "psytrance"], "electronic/trance"),
    (&["house"], "electronic/house"),
    (&["jungle", "jungl", "ragga"], "electronic/drum-and-bass/jungle"),
    (&["dnb", "drumandbass", "drumnbass", "neurofunk"], "electronic/drum-and-bass"),
    (&["hardcore", "hardc", "gabber", "gabba", "speedcore", "breakcore"], "electronic/hardcore"),
    (&["happyhardcore"], "electronic/hardcore/happy-hardcore"),
    (&["breakbeat", "breaks", "bigbeat"], "electronic/breakbeat"),
    (&["ambient", "atmos", "atmosphere", "atmospheric", "chillout", "chill"], "electronic/ambient"),
    (&["downtempo", "triphop", "lounge"], "electronic/downtempo"),
    (&["industrial", "ebm"], "electronic/industrial"),
    (&["electro"], "electronic/breakbeat/electro"),
    (&["eurodance"], "electronic/eurodance"),
    (&["dance", "rave", "electronica"], "electronic"),
    (&["synthpop", "synthwave"], "pop/synth-pop"),
    (&["disco", "italo"], "r-and-b/disco"),
    (&["funk", "funky"], "r-and-b/funk"),
    (&["soul"], "r-and-b/soul"),
    (&["hiphop", "rap"], "hip-hop"),
    (&["pop"], "pop"),
    (&["rock"], "rock"),
    (&["metal", "thrash"], "metal"),
    (&["punk"], "punk"),
    (&["jazz", "jazzc", "swing", "bebop", "fusion"], "jazz"),
    (&["blues"], "blues"),
    (&["reggae", "ska", "dub"], "reggae"),
    (&["latin", "salsa", "samba", "bossa", "bossanova", "tango"], "latin"),
    (&["country", "bluegrass"], "country"),
    (&["folk", "celtic"], "folk"),
    (
        &[
            "classical",
            "symph",
            "symphony",
            "symphonic",
            "sonata",
            "concerto",
            "fugue",
            "prelude",
            "toccata",
            "etude",
            "nocturne",
            "requiem",
            "baroque",
            "bach",
            "mozart",
            "beethoven",
            "chopin",
            "vivaldi",
            "handel",
            "grieg",
            "debussy",
            "satie",
            "tchaikovsky",
            "pachelbel",
        ],
        "classical",
    ),
    (&["orchestral", "orchestra"], "classical/orchestral"),
    (&["soundtrack", "ost", "score", "cinematic"], "soundtrack"),
    (&["ingame", "videogame", "gamemusic"], "soundtrack/game"),
    (&["newage", "meditation", "relax", "relaxing"], "new-age"),
    (&["xmas", "christmas"], "seasonal/christmas"),
    (&["experimental", "noise", "avantgarde"], "experimental"),
];

/// Words in a module's names that say it's a game's music: its level,
/// menu or game-over tune ("obsession - level 2", "tronic-gameover").
const GAME_WORDS: &[&str] = &["game", "ingame", "videogame", "gamemusic", "gameover", "highscore", "hiscore", "level", "menu", "boss", "ost"];

/// The format's computer: what it was made on (its own computer, not
/// every one that later played it).
fn platform(format: &str) -> Option<&'static str> {
    Some(match format {
        "mod" | "stk" | "okt" | "med" | "digi" | "dbm" | "mtn" | "sfx" | "emod" | "kris" | "ims" | "gmc" | "pt36" | "dss" | "symmod" | "ftm" | "puma" | "stp" | "ex" | "am" | "packed" => "amiga",
        "s3m" | "stm" | "mtm" | "669" | "ptm" | "gdm" | "dsm" | "dsm1" | "ult" | "far" | "imf" | "amf" | "rtm" | "plm" | "fnk" | "c67" | "xmf" | "dstudio" | "667" | "dsmi" | "psm" | "liq" | "uni"
        | "ams" | "ams2" | "mdl" | "dmf" | "kdm" | "it" | "xm" => "pc-dos",
        "mptm" | "mt2" | "j2b" => "windows",
        "dtm" | "gtk" | "gt2" | "mgt" => "atari",
        "musx" => "acorn",
        "mad" => "mac",
        _ => return None,
    })
}

/// A sample category (`analysis::CATEGORIES`) as an instrument (§7).
fn instrument(category: &str) -> Option<&'static str> {
    Some(match category {
        "Kick" | "Snare" | "Clap" | "Hi-hat" | "Cymbal" | "Tom" | "Percussion" => "drums",
        "Drum loop" => "breakbeats",
        "Bass" => "bass",
        "Piano" => "piano",
        "Electric piano" => "electric-piano",
        "Organ" => "organ",
        "Mallets & bells" => "mallets",
        "Strings" => "strings",
        "Brass" => "brass",
        "Woodwind" => "woodwinds",
        "Guitar" => "guitar",
        "Plucked" => "plucked-strings",
        "Choir & voice" => "voice",
        "Pad" => "pad",
        "Lead" => "lead-synth",
        "Synth" => "synth",
        "Chip waveform" => "chip",
        "Orchestral hit" => "orchestra-hit",
        "Sound effect" => "sound-effects",
        _ => return None,
    })
}

/// The catalog's duration class for a length in seconds (§10).
fn length_class(seconds: f32) -> &'static str {
    match seconds {
        s if s < 5.0 => "micro",
        s if s < 15.0 => "sting",
        s if s < 60.0 => "short",
        s if s < 120.0 => "bed",
        s if s < 300.0 => "standard",
        s if s < 600.0 => "extended",
        _ => "long-form",
    }
}

/// Lowercase words of letters and digits; a word with digits stuck to
/// its end also gives the word without them ("techno2" is techno).
fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for w in text.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
        if w.is_empty() {
            continue;
        }
        out.push(w.to_string());
        let core = w.trim_end_matches(|c: char| c.is_ascii_digit());
        if !core.is_empty() && core.len() != w.len() {
            out.push(core.to_string());
        }
    }
    out
}

/// Two words run together the way a genre is often written: "drum and
/// bass", "drum'n'bass", "hip hop", "happy hardcore", "big beat".
fn joined(words: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for i in 0..words.len() {
        if let (Some(a), Some(b)) = (words.get(i), words.get(i + 1)) {
            out.push(format!("{a}{b}"));
            if let Some(c) = words.get(i + 2) {
                out.push(format!("{a}{b}{c}"));
            }
        }
    }
    out
}

/// A slug: lowercase letters and digits, the rest dashes, none doubled.
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

/// A plausible year in a text: four digits from 1980 to 2039 standing alone.
fn year_in(text: &str) -> Option<u16> {
    let b = text.as_bytes();
    (0..b.len().saturating_sub(3)).find_map(|i| {
        let four = &b[i..i + 4];
        let alone = (i == 0 || !b[i - 1].is_ascii_digit()) && b.get(i + 4).is_none_or(|c| !c.is_ascii_digit());
        if !alone || !four.iter().all(u8::is_ascii_digit) {
            return None;
        }
        let y: u16 = std::str::from_utf8(four).ok()?.parse().ok()?;
        (1980..2040).contains(&y).then_some(y)
    })
}

/// A scene.org party folder in a path or URL: its year and its name
/// (`…/parties/1995/assembly95/music/…`).
fn party(path: &str) -> Option<(u16, String)> {
    let lower = path.to_lowercase();
    let at = lower.find("/parties/")?;
    let mut parts = lower[at + 9..].split('/');
    let year: u16 = parts.next()?.parse().ok()?;
    let name = parts.next().filter(|n| !n.is_empty() && !n.contains('.'))?;
    Some((year, slug(name)))
}

/// The folders a path or URL passes through, as words.
fn folder_words(path: &str) -> Vec<String> {
    let p = path.replace('\\', "/");
    let dirs = p.rsplit_once('/').map_or("", |(d, _)| d);
    // A URL's host and scheme say nothing of the music.
    let dirs = dirs.split_once("://").map_or(dirs, |(_, rest)| rest.split_once('/').map_or("", |(_, d)| d));
    let decoded = dirs.replace("%20", " ");
    words(&decoded)
}

/// The tags guessed for a module, from its record and its samples'.
pub fn module_tags(db: &Db, m: &ModuleRec) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    let mut add = |t: String| {
        if !tags.contains(&t) {
            tags.push(t);
        }
    };

    // Genre: words where they're said, folders where it was filed.
    let mut text = Vec::new();
    for t in std::iter::once(&m.title).chain(m.files.iter()).chain(m.comments.iter()) {
        text.extend(words(t));
    }
    for p in m.origins.iter().filter(|u| !u.contains("/pub/modules/")) {
        text.extend(folder_words(p));
    }
    let mut all = text.clone();
    all.extend(joined(&text));
    let mut genres: Vec<&str> = Vec::new();
    for (keys, genre) in GENRE_WORDS {
        if all.iter().any(|w| keys.contains(&w.as_str())) && !genres.contains(genre) {
            genres.push(genre);
        }
    }
    // Chiptune by its samples: mostly tiny looped chip waveforms.
    let cats: Vec<&str> = m.samples.iter().filter_map(|(_, id)| db.samples.get(id)).filter_map(|s| s.categories().first().copied()).collect();
    let chips = cats.iter().filter(|c| **c == "Chip waveform").count();
    if !cats.is_empty() && chips * 2 >= cats.len() && m.sample_bytes < 32 * 1024 && !genres.contains(&"electronic/chiptune") {
        genres.insert(0, "electronic/chiptune");
    }
    // A genre and one under it: the narrower says both.
    let narrow: Vec<&str> = genres.iter().copied().filter(|g| !genres.iter().any(|o| o != g && o.starts_with(&format!("{g}/")))).collect();
    for g in narrow.iter().take(3) {
        add(format!("genre:{g}"));
    }

    // Platform, and games' music formats.
    if let Some(p) = platform(&m.format) {
        add(format!("platform:{p}"));
    }
    let ext = |e: &str| m.files.iter().any(|f| f.to_lowercase().ends_with(&format!(".{e}")));
    if ext("umx") {
        add("platform:windows".into());
        add("use:game".into());
        add("game:unreal-engine".into());
    } else if m.format == "j2b" {
        add("use:game".into());
        add("game:jazz-jackrabbit-2".into());
    } else if m.format == "psm" {
        add("use:game".into());
        add("game:epic-megagames".into());
    } else if all.iter().any(|w| GAME_WORDS.contains(&w.as_str())) {
        add("use:game".into());
    }

    // Where it was entered or filed: a party's compo, a demo.
    let places: Vec<&String> = m.paths.iter().chain(m.origins.iter()).collect();
    let mut party_year = None;
    if let Some((y, name)) = places.iter().find_map(|p| party(p)) {
        add("use:demo-compo".into());
        add(format!("party:{name}"));
        party_year = Some(y);
    } else if text.iter().any(|w| matches!(w.as_str(), "demo" | "intro" | "compo" | "demoscene")) {
        add("use:demoscene".into());
    }

    // Year: SAUCE's date, the party's, else a year it states.
    let year = year_in(&m.date).or(party_year).or_else(|| m.rights.iter().chain(m.comments.iter()).find_map(|t| year_in(t))).or_else(|| year_in(&m.message)).or_else(|| year_in(&m.title));
    if let Some(y) = year {
        add(format!("year:{y}"));
        add(format!("era:{}s", y / 10 * 10));
    }

    if m.seconds > 0.0 {
        add(format!("length:{}", length_class(m.seconds)));
    }

    // Instruments, by how many of its samples are each, most first.
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for c in &cats {
        if let Some(i) = instrument(c) {
            *count.entry(i).or_default() += 1;
        }
    }
    // Any noisy sample with no pitch is guessed a sound effect: they're
    // the song's only when they're a third of it.
    count.retain(|i, n| *i != "sound-effects" || *n * 3 >= cats.len());
    let mut inst: Vec<(&str, usize)> = count.into_iter().collect();
    inst.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    for (i, _) in inst {
        add(format!("inst:{i}"));
    }

    if let Some((terms, _)) = module_license(m) {
        add(format!(
            "license:{}",
            match terms {
                Terms::Free => "free",
                Terms::Credit => "free-with-credit",
                Terms::Restricted => "restricted",
            }
        ));
    }
    tags
}

/// A module's genre, as guessed, for showing: its first `genre:` tag's
/// narrowest part, in words ("electronic/drum-and-bass/jungle": "Jungle").
pub fn genre_name(tags: &[String]) -> String {
    let Some(g) = tags.iter().find_map(|t| t.strip_prefix("genre:")) else { return String::new() };
    let last = g.rsplit('/').next().unwrap_or(g).replace('-', " ");
    let mut c = last.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str()).replace("R and b", "R&B").replace("Hip hop", "Hip-hop")
}

/// Whether a search term `ns:value` matches one of `tags`: the same
/// namespace, and `value` the start of its value or of any part of a
/// genre's path (`genre:jungle` matches `genre:electronic/drum-and-bass/jungle`).
pub fn term_matches(tags: &[String], ns: &str, value: &str) -> bool {
    tags.iter().any(|t| {
        let Some((n, v)) = t.split_once(':') else { return false };
        n == ns && (v.starts_with(value) || v.split('/').any(|part| part.starts_with(value)))
    })
}

/// The namespaces the guessed tags use, which search reads as terms.
pub const NAMESPACES: &[&str] = &["genre", "platform", "year", "era", "length", "inst", "use", "party", "game", "license"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::db::{Catalog, SampleRec};

    fn module(title: &str, file: &str) -> ModuleRec {
        ModuleRec { title: title.into(), files: vec![file.into()], format: "mod".into(), ..Default::default() }
    }

    #[test]
    fn genres_come_from_whole_words_and_folders() {
        let db = Db::default();
        let t = module_tags(&db, &module("Rocket Science", "rocket.mod"));
        assert!(!t.iter().any(|t| t.starts_with("genre:")), "{t:?}");
        let t = module_tags(&db, &module("Jungle Fever", "jf.mod"));
        assert_eq!(t[0], "genre:electronic/drum-and-bass/jungle");
        let mut m = module("x", "x.mod");
        m.origins = vec!["https://ftp.fau.de/aminet/mods/techn/foo.lha".into()];
        assert!(module_tags(&db, &m).contains(&"genre:electronic/techno".to_string()));
        assert!(module_tags(&db, &module("drum'n'bass session", "a.mod")).contains(&"genre:electronic/drum-and-bass".to_string()));
        assert!(module_tags(&db, &module("x", "bach - fugue in g.mod")).contains(&"genre:classical".to_string()));
        assert!(module_tags(&db, &module("x", "x.mod")).contains(&"platform:amiga".to_string()));
    }

    #[test]
    fn parties_years_and_lengths() {
        let db = Db::default();
        let mut m = module("x", "x.s3m");
        m.format = "s3m".into();
        m.origins = vec!["https://ftp.scene.org/pub/parties/1995/assembly95/music/x.zip".into()];
        m.seconds = 200.0;
        let t = module_tags(&db, &m);
        for want in ["platform:pc-dos", "use:demo-compo", "party:assembly95", "year:1995", "era:1990s", "length:standard"] {
            assert!(t.contains(&want.to_string()), "{want} in {t:?}");
        }
        let mut m = module("x", "x.mod");
        m.rights = vec!["its message: (c) 1992 Someone".into()];
        assert!(module_tags(&db, &m).contains(&"era:1990s".to_string()));
        assert_eq!(year_in("v12345 and 1989"), Some(1989));
        assert_eq!(year_in("2101"), None);
    }

    #[test]
    fn older_modules_are_tagged_again_and_kept() {
        let mut db = Db::default();
        let mut m = module("Techno Mania", "tm.mod");
        m.id = crate::repo::canon::Id([9; 32]);
        m.seconds = 30.0;
        db.modules.insert(m.id, m);
        assert_eq!(db.retag_modules(false), 1);
        assert_eq!(db.retag_modules(false), 0);
        let back = Db::read(&db.write()).unwrap();
        let m = &back.modules[&crate::repo::canon::Id([9; 32])];
        assert_eq!((m.tagged, m.seconds), (VERSION, 30.0));
        assert_eq!(m.guessed, ["genre:electronic/techno", "platform:amiga", "length:short"]);
        assert_eq!(back.module_tags()[0], ("genre:electronic/techno".to_string(), 1));
    }

    #[test]
    fn samples_say_instruments_and_chiptune() {
        let mut db = Db::default();
        let mut m = module("x", "x.mod");
        for (i, cat) in ["Chip waveform", "Chip waveform", "Kick"].iter().enumerate() {
            let id = crate::repo::canon::Id([i as u8 + 1; 32]);
            db.samples.insert(id, SampleRec { id, guessed: cat.to_string(), catalog: Catalog::default(), ..Default::default() });
            m.samples.push((i as u16 + 1, id));
        }
        m.sample_bytes = 2000;
        let t = module_tags(&db, &m);
        assert_eq!(t[0], "genre:electronic/chiptune");
        assert!(t.contains(&"inst:chip".to_string()) && t.contains(&"inst:drums".to_string()));
        assert_eq!(genre_name(&t), "Chiptune");
        assert!(term_matches(&t, "genre", "chip") && term_matches(&t, "genre", "electronic") && !term_matches(&t, "genre", "rock"));
    }
}
