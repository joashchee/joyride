// What a game asks of its music, and which recipes make it. Posture: a drafting engine for placeholder and prototype music;
// polished themes are a composer's job.
const r = (recipe, params = {}) => ({ recipe, params });
export default {
  id: 'game-music',
  title: 'Game music: stingers, loops, scene beds',
  doc: 'docs/composer-plan.md',
  workflow: ['Stingers first (pickup, win, lose, level up, alert): they are short and tell the player something', 'A looping bed per scene or mood', 'One verse-chorus or looping song for the title and the main fight', 'Layer, transpose and stretch to make variants from what exists'],
  axes: { key: ['C', 'F', 'G', 'A', 'D'], mood: ['calm', 'sad', 'tense', 'bright'] },
  subjects: [
    { id: 'stingers', engine: 'opl', label: 'Short reward and failure stingers (pickup, win, lose, level up)', recipes: ['pickup', 'win', 'lose', 'levelup'].map((kind) => r('jingle', { kind })) },
    { id: 'alert-loop', engine: 'opl', label: 'A warning that loops until dismissed', recipes: [r('jingle', { kind: 'alert' })] },
    { id: 'scene-beds', engine: 'midi', label: 'Looping ambience by mood: exploration, grief, danger, wonder', recipes: ['calm', 'sad', 'tense', 'bright'].map((mood) => r('ambient', { mood })) },
    { id: 'title-theme', engine: 'midi', label: 'A title or menu song with an intro, verses and choruses', recipes: [r('song', { form: 'verse-chorus' })] },
    { id: 'battle-loop', engine: 'midi', label: 'A driving loop for a fight', recipes: [r('song', { form: 'loop', drums: 'breakbeat', bpm: 140, scale: 'minor' })] },
    { id: 'waltz-scene', label: 'A town or ballroom in three-four time', recipes: [r('song', { form: 'AABA', beats: 3 })] },
    { id: 'march', label: 'A march for armies and parades', recipes: [r('drums', { pattern: 'march' })], partial: 'the drums only; no brass or fife voice' },
    { id: 'keyed-variants', label: 'The same piece in another key for a character or region', recipes: ['F', 'G', 'A'].map((key) => r('song', { form: 'loop', key })) },
    { id: 'adaptive-layers', label: 'Stems that fade in and out with the action (vertical layering)', provides: 'toModule({ layers, on }) groups a song\'s channels as named stems and starts some muted; the .ntt keeps the groups, and cues.json lists them', partial: 'the engine writes the stems and `compose cues --audio` bakes them; `createCuePlayer` fades them on the bar in a page' },
    { id: 'horizontal-sections', label: 'Sections a host switches between at a phrase boundary (horizontal resequencing)', recipes: [r('song', { form: 'verse-chorus-bridge' }), r('song', { form: 'rondo' })], provides: 'toModule({ lists }) writes each named section as its own order list that loops, and cues.json lists the bars where a switch lands', partial: 'the switch rules are data (transitions in the cue sheet); `createCuePlayer` applies them in a page' },
    { id: 'cue-sheet', label: 'Every scene, state and beat that needs music, as data', provides: 'composeCues(sheet) / compose cues sheet.json --dir d: each cue as .ntt and .mid plus a cues.json manifest, with loops linted for their seam and stingers ending after a tail' },
    { id: 'leitmotifs', label: 'A short theme per character that returns in different arrangements', recipes: ['calm', 'tense', 'triumph', 'lament', 'chase'].map((state) => r('leitmotif', { state })), partial: 'one theme in five states (scale, tempo, instrument, octave, speed, accompaniment); not yet stated inside a longer song' },
    { id: 'sound-effects', label: 'Hits, footsteps, spells and other non-musical effects', reference: 'sound design, not music: a different engine' },
    { id: 'recorded-references', label: 'Recorded game soundtracks as study material', reference: 'copyrighted recordings: listened to, never sampled, traced or shipped' },
  ],
};
