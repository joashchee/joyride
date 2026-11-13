// The browser-safe entry point: everything that composes, with no Node built-ins anywhere behind it, so it runs in a page, a
// worker, Node or Deno unchanged. MIDI and WAV are plain bytes (Uint8Array), so downloads need no Node either.
//
//   import { compose, toMidi, render, toWav, lint, rng } from 'music-composer';
//   const art = compose('song', { form: 'verse-chorus', key: 'G', seed: 'town' });   // { id, score, loop, sections, params }
//   const midi = toMidi(art.score);                                                    // Uint8Array, the exact artifact
//   const wav = toWav(render(art.score));                                              // a preview you can hear
export { Score, PPQ, STEP } from './score.js';
export { rng, hashSeed } from './rng.js';
export { NOTE_NAMES, SCALES, SCALE_IDS, PARENT, PROGRESSIONS, PROGRESSION_IDS, DRUM, tonic, pitch, pitchName, degree, inScale, scaleNotes, snap, chordOf, chordName, progression } from './scales.js';
export { arp, voiceLead, CONSONANT, PERFECT, parallelPerfect } from './voicing.js';
export { transpose, invert, retrograde, stretch, repeat, slice, concat, layer, swing, humanize } from './transform.js';
export { lint } from './lint.js';
export { toMidi, parseMidi, fromMidi } from './midi.js';
export { sha256, hex } from './sha256.js';
export { ByteWriter, ByteReader } from './bytes.js';
export { nlzStore, nlzUnstore } from './nlz.js';
export { Module, NOTE_FADE, NOTE_CUT, NOTE_OFF, ORDER_SKIP, ORDER_END, CHANNEL_MUTED, KINDS, LOOPS, EFFECTS, EFFECT_KINDS } from './module.js';
export { fmPatch, oplPatch, OPL_PATCHES, OPL_PATCH_IDS } from './opl.js';
export { cycle, drumShot, CYCLE, CYCLE_RATE, DRUM_RATE } from './samples.js';
export { MANIFEST_VERSION, MANIFEST_ENGINES, NOTE_FLOOR, UNPITCHED, toManifest, bake, parseWav } from './bake.js';
export { toNtt, parseNtt } from './ntt.js';
export { CUE_KINDS, SWITCH_AT, buildCue, composeCues, nextSwitch, playScript, audioFiles, renderCue, cueAudio } from './cues.js';
export { createCuePlayer, loadCueAudio } from './cueplayer.js';
export { toModule } from './tracker.js';
export { render, WAVES } from './synth.js';
export { toWav } from './wav.js';
export { pianoRoll } from './text.js';
export { toNotation, parseNotation } from './notation.js';
export { RECIPES, RECIPE_IDS, compose } from './recipes/index.js';
export { addMelody, addChords, addBass, addDrums, CONTOURS, CHORD_STYLES, BASS_PATTERNS, DRUM_PATTERNS, DRUM_BEATS } from './recipes/parts.js';
export { writeBlues, writeModulation, writeCounterpoint, writePhrase, BLUES_12, BLUES_SCALES, MODULATIONS, MODULATION_IDS, SPECIES, CADENCES, CADENCE_IDS, PHRASE_TYPES } from './recipes/theory.js';
export { FORMS } from './recipes/forms.js';
export { writeLeitmotif, motifOf, STATES, STATE_IDS } from './recipes/motif.js';
export { JINGLES, MOODS } from './recipes/game.js';
export { KITS, registerKit, coverage, subjectStatus } from './kits/index.js';
export { toAudioBuffer, play } from './render.js';
