// Copyright 2026 ansiapps. Neumetik MIDI, MIT licensed: see LICENSE.

//! Instrument names to General MIDI programs, for every text format.

/// General MIDI's 128 program names, in order.
pub const GM_NAMES: [&str; 128] = [
    "Acoustic Grand Piano", "Bright Acoustic Piano", "Electric Grand Piano", "Honky-tonk Piano", "Electric Piano 1", "Electric Piano 2", "Harpsichord", "Clavinet",
    "Celesta", "Glockenspiel", "Music Box", "Vibraphone", "Marimba", "Xylophone", "Tubular Bells", "Dulcimer",
    "Drawbar Organ", "Percussive Organ", "Rock Organ", "Church Organ", "Reed Organ", "Accordion", "Harmonica", "Tango Accordion",
    "Acoustic Guitar (nylon)", "Acoustic Guitar (steel)", "Electric Guitar (jazz)", "Electric Guitar (clean)", "Electric Guitar (muted)", "Overdriven Guitar", "Distortion Guitar", "Guitar Harmonics",
    "Acoustic Bass", "Electric Bass (finger)", "Electric Bass (pick)", "Fretless Bass", "Slap Bass 1", "Slap Bass 2", "Synth Bass 1", "Synth Bass 2",
    "Violin", "Viola", "Cello", "Contrabass", "Tremolo Strings", "Pizzicato Strings", "Orchestral Harp", "Timpani",
    "String Ensemble 1", "String Ensemble 2", "Synth Strings 1", "Synth Strings 2", "Choir Aahs", "Voice Oohs", "Synth Voice", "Orchestra Hit",
    "Trumpet", "Trombone", "Tuba", "Muted Trumpet", "French Horn", "Brass Section", "Synth Brass 1", "Synth Brass 2",
    "Soprano Sax", "Alto Sax", "Tenor Sax", "Baritone Sax", "Oboe", "English Horn", "Bassoon", "Clarinet",
    "Piccolo", "Flute", "Recorder", "Pan Flute", "Blown Bottle", "Shakuhachi", "Whistle", "Ocarina",
    "Lead 1 (square)", "Lead 2 (sawtooth)", "Lead 3 (calliope)", "Lead 4 (chiff)", "Lead 5 (charang)", "Lead 6 (voice)", "Lead 7 (fifths)", "Lead 8 (bass + lead)",
    "Pad 1 (new age)", "Pad 2 (warm)", "Pad 3 (polysynth)", "Pad 4 (choir)", "Pad 5 (bowed)", "Pad 6 (metallic)", "Pad 7 (halo)", "Pad 8 (sweep)",
    "FX 1 (rain)", "FX 2 (soundtrack)", "FX 3 (crystal)", "FX 4 (atmosphere)", "FX 5 (brightness)", "FX 6 (goblins)", "FX 7 (echoes)", "FX 8 (sci-fi)",
    "Sitar", "Banjo", "Shamisen", "Koto", "Kalimba", "Bagpipe", "Fiddle", "Shanai",
    "Tinkle Bell", "Agogo", "Steel Drums", "Woodblock", "Taiko Drum", "Melodic Tom", "Synth Drum", "Reverse Cymbal",
    "Guitar Fret Noise", "Breath Noise", "Seashore", "Bird Tweet", "Telephone Ring", "Helicopter", "Applause", "Gunshot",
];

/// Short names and other formats' names, to programs. Kern's `*I` codes
/// (Humdrum's instrument list), Alda's names, and the common words.
const ALIASES: &[(&str, u8)] = &[
    ("piano", 0), ("grandpiano", 0), ("acousticpiano", 0), ("pno", 0), ("klavier", 0), ("pianoforte", 0), ("fortepiano", 0),
    ("brightpiano", 1), ("electricpiano", 4), ("epiano", 4), ("rhodes", 4), ("honkytonk", 3), ("cembalo", 6), ("clavichord", 7), ("clavi", 7), ("cemba", 6), ("hpschd", 6), ("harpsi", 6),
    ("celeste", 8), ("glock", 9), ("glockenspiel", 9), ("musicbox", 10), ("vibes", 11), ("vibraphone", 11), ("vibra", 11), ("marimba", 12), ("marim", 12), ("xylophone", 13), ("xylo", 13), ("chimes", 14), ("tubularbells", 14), ("bells", 14), ("dulcimer", 15), ("cimbalom", 15),
    ("organ", 19), ("pipeorgan", 19), ("organo", 19), ("hammond", 16), ("drawbar", 16), ("harmonium", 20), ("accordion", 21), ("accor", 21), ("bandoneon", 23), ("concertina", 21), ("harmonica", 22), ("armon", 22),
    ("guitar", 24), ("acousticguitar", 25), ("classicalguitar", 24), ("nylonguitar", 24), ("steelguitar", 25), ("guitr", 24), ("guitarra", 24), ("chitarra", 24), ("gitarre", 24), ("lute", 24), ("liuto", 24), ("mandolin", 25), ("mando", 25), ("ukulele", 24), ("ukule", 24), ("jazzguitar", 26), ("electricguitar", 27), ("eguitar", 27), ("cleanguitar", 27), ("mutedguitar", 28), ("overdrive", 29), ("distortion", 30), ("distortedguitar", 30),
    ("bass", 32), ("acousticbass", 32), ("doublebass", 43), ("stringbass", 43), ("electricbass", 33), ("bassguitar", 33), ("ebass", 33), ("fretless", 35), ("slapbass", 36), ("synthbass", 38),
    ("violin", 40), ("violn", 40), ("violino", 40), ("vln", 40), ("geige", 40), ("viola", 41), ("viole", 41), ("vla", 41), ("bratsche", 41), ("cello", 42), ("violoncello", 42), ("vc", 42), ("cell", 42), ("contrabass", 43), ("kontrabass", 43), ("cb", 43), ("contrabbasso", 43), ("violone", 43), ("viol", 42), ("gamba", 42), ("vdgamba", 42),
    ("tremolostrings", 44), ("pizzicato", 45), ("pizz", 45), ("harp", 46), ("arpa", 46), ("harfe", 46), ("timpani", 47), ("timp", 47), ("kettledrum", 47), ("pauken", 47),
    ("strings", 48), ("stringensemble", 48), ("stringorchestra", 48), ("synthstrings", 50), ("choir", 52), ("chorus", 52), ("aahs", 52), ("voice", 53), ("vox", 53), ("oohs", 53), ("soprano", 52), ("alto", 52), ("tenor", 52), ("baritone", 52), ("contralto", 52), ("mezzo", 52), ("basso", 52), ("cant", 52), ("sopran", 52), ("vocals", 53), ("vocal", 53), ("synthvoice", 54), ("orchestrahit", 55), ("orchhit", 55),
    ("trumpet", 56), ("trump", 56), ("tromba", 56), ("trompete", 56), ("cornet", 56), ("cornett", 56), ("flugelhorn", 56), ("trombone", 57), ("trbon", 57), ("posaune", 57), ("sackbut", 57), ("tuba", 58), ("euphonium", 58), ("mutedtrumpet", 59), ("horn", 60), ("frenchhorn", 60), ("corno", 60), ("cor", 60), ("waldhorn", 60), ("brass", 61), ("brasssection", 61), ("synthbrass", 62),
    ("sopranosax", 64), ("saxophone", 65), ("sax", 65), ("altosax", 65), ("saxa", 65), ("tenorsax", 66), ("saxt", 66), ("baritonesax", 67), ("saxr", 67), ("saxs", 64),
    ("oboe", 68), ("oboi", 68), ("hautbois", 68), ("shawm", 68), ("englishhorn", 69), ("cora", 69), ("coranglais", 69), ("bassoon", 70), ("fagot", 70), ("fagotto", 70), ("contrabassoon", 70), ("clarinet", 71), ("clars", 71), ("clarinetto", 71), ("klarinette", 71), ("bassclarinet", 71),
    ("piccolo", 72), ("picco", 72), ("flute", 73), ("flt", 73), ("flauto", 73), ("flote", 73), ("flöte", 73), ("traverso", 73), ("recorder", 74), ("recrd", 74), ("blockflote", 74), ("panflute", 75), ("panpipes", 75), ("bottle", 76), ("shakuhachi", 77), ("whistle", 78), ("tinwhistle", 78), ("pennywhistle", 78), ("ocarina", 79),
    ("squarewave", 80), ("square", 80), ("lead", 80), ("sawtooth", 81), ("saw", 81), ("calliope", 82), ("pad", 89), ("warmpad", 89), ("synth", 81), ("synthesizer", 81),
    ("sitar", 104), ("banjo", 105), ("shamisen", 106), ("koto", 107), ("kalimba", 108), ("mbira", 108), ("bagpipe", 109), ("bagpipes", 109), ("fiddle", 110), ("shanai", 111), ("shehnai", 111),
    ("steeldrums", 114), ("steeldrum", 114), ("woodblock", 115), ("taiko", 116), ("tom", 117), ("melodictom", 117), ("synthdrum", 118),
    ("seashore", 122), ("birdtweet", 123), ("helicopter", 125), ("applause", 126), ("gunshot", 127),
];

fn squash(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// A General MIDI program (0 to 127) for an instrument named in any of the
/// text formats: GM's own name (`Violin`, `acoustic-guitar-nylon`,
/// `midi-electric-piano-1`), a common word (`piano`, `strings`), another
/// language's (`violino`, `Geige`) or Humdrum's `*I` code (`*Iflt`).
/// Numbers and trailing digits (`Violin I`, `Flute 2`) are ignored.
pub fn program(name: &str) -> Option<u8> {
    let name = name.trim().trim_start_matches("*I").trim_start_matches("midi-");
    let full = squash(name);
    if full.is_empty() {
        return None;
    }
    if let Some(i) = GM_NAMES.iter().position(|n| squash(n) == full) {
        return Some(i as u8);
    }
    // Without a part's number or roman numeral at the end.
    let words: Vec<String> = name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect();
    let trimmed: Vec<&String> = {
        let mut w: Vec<&String> = words.iter().collect();
        while w.len() > 1 && w.last().is_some_and(|l| l.chars().all(|c| c.is_ascii_digit()) || matches!(l.as_str(), "i" | "ii" | "iii" | "iv" | "v" | "vi" | "1st" | "2nd" | "3rd" | "4th" | "in" | "solo" | "primo" | "secondo")) {
            w.pop();
        }
        // "Clarinet in Bb": what's before "in".
        if let Some(i) = w.iter().position(|x| x.as_str() == "in") {
            w.truncate(i.max(1));
        }
        w
    };
    let joined: String = trimmed.iter().map(|s| s.as_str()).collect();
    for key in [full.as_str(), joined.as_str()] {
        if let Some(i) = GM_NAMES.iter().position(|n| squash(n) == key) {
            return Some(i as u8);
        }
        if let Some(&(_, p)) = ALIASES.iter().find(|a| a.0 == key) {
            return Some(p);
        }
        // Digits glued on: "violin1".
        let bare = key.trim_end_matches(|c: char| c.is_ascii_digit());
        if let Some(&(_, p)) = ALIASES.iter().find(|a| a.0 == bare) {
            return Some(p);
        }
    }
    // A word of it that's an alias, the last first ("Solo Violin", "Bass Clarinet" is a clarinet).
    for w in trimmed.iter().rev() {
        if let Some(&(_, p)) = ALIASES.iter().find(|a| a.0 == w.as_str()) {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(program("Acoustic Grand Piano"), Some(0));
        assert_eq!(program("acoustic-guitar-nylon"), Some(24));
        assert_eq!(program("midi-electric-piano-1"), Some(4));
        assert_eq!(program("Violin I"), Some(40));
        assert_eq!(program("Flute 2"), Some(73));
        assert_eq!(program("Clarinet in B♭"), Some(71));
        assert_eq!(program("*Iflt"), Some(73));
        assert_eq!(program("Violoncello"), Some(42));
        assert_eq!(program("Solo Violin"), Some(40));
        assert_eq!(program("Bass Clarinet"), Some(71));
        assert_eq!(program("Contrabass"), Some(43));
        assert_eq!(program("Kazoo-o-matic"), None);
    }
}
