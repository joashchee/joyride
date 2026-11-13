// The fundamentals of writing a song, each as a parameter of a recipe rather than prose.
const r = (recipe, params = {}) => ({ recipe, params });
export default {
  id: 'song-craft',
  title: 'Song craft: melody, harmony, rhythm, form',
  doc: 'docs/composer-plan.md',
  workflow: ['Pick a key and scale', 'Choose a progression', 'Write a melody over it with a contour', 'Add a bassline and a groove', 'Arrange sections into a form'],
  axes: { contour: ['arch', 'rise', 'fall', 'wave', 'flat'], scale: ['major', 'minor', 'dorian', 'major-pentatonic', 'blues'], style: ['block', 'pad', 'arp', 'strum', 'stab'] },
  subjects: [
    { id: 'melodic-contour', label: 'The shape of a line: arch, rise, fall, wave', recipes: ['arch', 'rise', 'fall', 'wave'].map((contour) => r('melody', { contour })) },
    { id: 'motif-repetition', label: 'Repeat a rhythm, vary the next (A B A C)', recipes: [r('melody', { bars: 8 })] },
    { id: 'scales-and-modes', label: 'The same tune in different scales', recipes: ['minor', 'dorian', 'major-pentatonic', 'blues'].map((scale) => r('melody', { scale })) },
    { id: 'chord-progressions', label: 'Common progressions from the pop, rock and jazz habits', recipes: ['pop', 'axis', 'canon', 'jazz-251', 'minor-sad', 'andalusian'].map((progression) => r('chords', { progression, bars: 8 })) },
    { id: 'voice-leading', label: 'Chords that move by the smallest steps', recipes: [r('chords', { style: 'block', bars: 8, progression: 'canon' })] },
    { id: 'chord-textures', label: 'Block, pad, arpeggio, strum and stab accompaniment', recipes: ['block', 'pad', 'arp', 'strum', 'stab'].map((style) => r('chords', { style })) },
    { id: 'basslines', label: 'Root, octave, root-fifth, walking and sustained bass', recipes: ['root', 'octave', 'root-fifth', 'walking', 'sustain'].map((pattern) => r('bass', { pattern })) },
    { id: 'grooves', label: 'Rock, four-on-the-floor, breakbeat, half-time, march and waltz', recipes: ['rock', 'four', 'breakbeat', 'half', 'march'].map((pattern) => r('drums', { pattern })).concat(r('drums', { pattern: 'waltz', beats: 3 })) },
    { id: 'meter', label: 'Three, four, five and seven beats a bar', recipes: [r('melody', { beats: 3 }), r('melody', { beats: 5 }), r('melody', { beats: 7 }), r('drums', { pattern: 'waltz', beats: 3 }), r('drums', { pattern: 'quintuple', beats: 5 }), r('drums', { pattern: 'septuple', beats: 7 })], partial: 'whole beats only; no six-eight or other compound feel' },
    { id: 'song-forms', label: 'Verse-chorus, with a bridge, AABA, rondo and loops', recipes: ['verse-chorus', 'verse-chorus-bridge', 'AABA', 'rondo', 'loop'].map((form) => r('song', { form })) },
    { id: 'twelve-bar-blues', label: 'Three four-bar lines over I IV V', recipes: [r('blues'), r('blues', { scale: 'major' }), r('blues', { scale: 'major-pentatonic', key: 'A' })] },
    { id: 'phrase-structure', label: 'Period and sentence: antecedent and consequent, idea, repeat and continuation', recipes: [r('phrase', { type: 'period' }), r('phrase', { type: 'sentence' })] },
    { id: 'cadences', label: 'Authentic, half, plagal and deceptive endings', recipes: ['authentic', 'half', 'plagal', 'deceptive'].map((cadence) => r('phrase', { cadence })) },
    { id: 'dynamics-feel', label: 'Velocity accents, swing and a loosened grid', provides: 'accents come from the writers; swing() and humanize() in transform.js' },
    { id: 'modulation', label: 'Changing key partway through a piece', recipes: ['fifth', 'fourth', 'whole-step', 'semitone', 'relative', 'parallel'].map((to) => r('modulation', { to })), partial: 'a pivot dominant then a direct change; no common-chord pivots or chromatic modulation' },
    { id: 'counterpoint', label: 'Independent melodic lines that fit together', recipes: [1, 2, 3].map((species) => r('counterpoint', { species })).concat(r('counterpoint', { species: 2, position: 'below' })), partial: 'first to third species in two voices; no fourth (syncopation) or fifth (florid) species, no three voices' },
    { id: 'lyrics-vocals', label: 'Words and a singing voice', reference: 'not drawable: notes only' },
  ],
};
