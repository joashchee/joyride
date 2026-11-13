// Sakura MML: a port of sakuramml-rust 0.2.3 (https://github.com/kujirahand/sakuramml-rust),
// Copyright (c) 2022 kujirahand, MIT licensed (see LICENSE in this folder). The compiler's own
// source is kept as it was, but for `crate::` paths and this file, which replaces its `lib.rs`
// (the WebAssembly glue and command line left out).

//! **Sakura**: the Japanese MML that the Sakura compiler (TextSakura,
//! sakuramml) turns into MIDI. Its compiler is kept whole, so a song is
//! the MIDI file Sakura itself makes, read back as a score.

#![allow(clippy::all, dead_code, unused_variables, unused_assignments, unused_mut, unused_imports, unreachable_code, non_snake_case, non_camel_case_types, non_upper_case_globals)]

pub mod lexer;
pub mod midi;
pub mod mml_def;
pub mod note_length;
pub mod runner;
pub mod sakura_functions;
pub mod sakura_message;
pub mod sakura_version;
pub mod song;
pub mod song_test;
pub mod source_cursor;
pub mod sutoton;
pub mod svalue;
pub mod token;

#[cfg(test)]
mod lexer_test;
#[cfg(test)]
mod runner_test;

use crate::score::Score;
use crate::Error;

/// The longest source Sakura compiles, as its own compiler sets.
pub const SAKURA_MAX_INPUT_SIZE: usize = 3 * 1024 * 1024;

/// A compile: the MIDI file and Sakura's log.
#[derive(Debug)]
pub struct SakuraResult {
    pub bin: Vec<u8>,
    pub log: String,
}

/// Sakura MML compiled to a MIDI file, with its log (messages in
/// English).
pub fn compile(source: &str) -> SakuraResult {
    let mut song = song::Song::new();
    song.set_language("en");
    if source.len() > SAKURA_MAX_INPUT_SIZE {
        return SakuraResult { bin: Vec::new(), log: format!("[ERROR](0) Input size exceeds max_input_size ({} > {})", source.len(), SAKURA_MAX_INPUT_SIZE) };
    }
    let source_mml = sutoton::convert(source);
    let tokens = lexer::lex(&mut song, &source_mml, 0);
    runner::exec(&mut song, &tokens);
    let bin = midi::generate(&mut song);
    SakuraResult { bin, log: song.get_logs_str() }
}

/// The line of the first error in a Sakura log.
fn first_error(log: &str) -> Option<u32> {
    let at = log.find("[ERROR](")? + 8;
    let n: String = log[at..].chars().take_while(|c| c.is_ascii_digit()).collect();
    Some(n.parse().unwrap_or(0))
}

/// Sakura MML as a score and its title (`TrackName`).
pub fn read(text: &str) -> Result<(Score, String), Error> {
    let r = compile(text);
    if let Some(line) = first_error(&r.log) {
        return Err(Error::BadText { line: line.max(1), column: 1, why: "Sakura can't compile this line" });
    }
    if r.bin.is_empty() {
        return Err(Error::BadText { line: 1, column: 1, why: "Sakura made no music from it" });
    }
    let score = Score::from_midi(&r.bin)?;
    Ok((score, title(text)))
}

/// A song's `TrackName = {"..."}` (or `曲名`).
fn title(text: &str) -> String {
    for line in text.lines() {
        let l = line.trim_start();
        if l.starts_with("TrackName") || l.starts_with("曲名") {
            if let Some(a) = l.find('"') {
                let rest = &l[a + 1..];
                return rest[..rest.find('"').unwrap_or(rest.len())].to_string();
            }
        }
    }
    String::new()
}

/// Whether a text looks like Sakura's MML: its `TR(n)`/`Track(n)`
/// tracks, `Tempo =`, `Str`/`Int` variables, `System.` settings, or its
/// Japanese commands.
pub fn looks_like(text: &str) -> bool {
    let has = |w: &str| text.contains(w);
    let upper = text.to_ascii_uppercase();
    upper.contains("TR(") || upper.contains("TRACK(") || upper.contains("TRACK=") || upper.contains("TR=") || has("System.") || has("TrackName") || has("TimeSignature") || has("Tempo=") || has("Tempo =") || has("Tempo(") || has("Str ") || has("Int ") || has("トラック") || has("テンポ") || has("音色") || has("ドレミ") || has("Include(") || has("CH(") || has("KeyFlag") || has("TimeBase")
}
