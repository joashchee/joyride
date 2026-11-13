// Models docs/cc0-pixel-art-learning-reference-kit.md. Posture: study material and method; nothing shipped is copied from it.
const r = (recipe, params = {}) => ({ recipe, params });
export default {
  id: 'pixel-art-learning',
  title: 'CC0 / Public-Domain Pixel Art Learning Kit',
  doc: 'docs/cc0-pixel-art-learning-reference-kit.md',
  // The kit's study workflow. `sprites study` runs the compare step; the rest is the person's, and the recipes are the "apply the lesson" step.
  workflow: ['Choose one reference', 'Reduce it to major shapes, values and palette', 'Recreate a small section at the same resolution', 'Compare side by side (sprites study)', 'Redraw from memory', 'Apply the lesson to an original subject'],
  subjects: [
    { id: 'line-and-shape', label: 'Clean lines, clusters, strong silhouettes at 16/24/32', recipes: [r('item', { shape: 'sword' }), r('item', { shape: 'shield' })], checks: ['noise', 'margin'] },
    { id: 'color-and-value', label: 'Limited palette (4 to 16), hue-shifted ramps, one light direction', provides: 'palette.js ramps; shade.js light from the top left', checks: ['palette', 'value'] },
    { id: 'shading-and-rendering', label: 'Cluster shading, sparing anti-aliasing, dither as texture', recipes: [r('item', { shape: 'potion' }), r('creature', { preset: 'slime' })] },
    { id: 'readability', label: 'Readable at 100/200/400% and at game scale', provides: 'sheet.js scaleStrip; lint rules outline and value', checks: ['outline', 'value'] },
    { id: 'faces-small', label: 'Faces at 8x8 / 16x16: eyes, hair silhouette, expression', recipes: [r('face', { expression: 'joy' }), r('face', { expression: 'anger' }), { recipe: 'creatureface', params: { size: 'small', mood: 'angry' }, style: 'ansi', lint: { skip: ['outline', 'contrast', 'ramp'] } }], partial: 'the pixel face is 16x16 only; the creature face draws a small 16 by 8 (16 by 16 pixels) in its own strokes, by shape, mood and hurt' },
    { id: 'hands-and-poses', label: 'Hands and poses: mitten to detailed', recipes: [r('hand', { gesture: 'open', stage: 'mitten' }), r('hand', { gesture: 'open', stage: 'blocks' }), r('hand', { gesture: 'open', stage: 'detailed' }), r('character', { loadout: 'rogue', pose: 'crouch' })], partial: 'right hand from the back only; see the hands/poses kit for the rest' },
    { id: 'animals', label: 'Real proportions reduced to readable pixel shapes', recipes: [r('creature', { preset: 'rat' }), r('creature', { preset: 'wolf' })] },
    { id: 'monsters', label: 'Animal anatomy, exaggerated silhouette, limited palette', recipes: [r('creature', { preset: 'goblin' }), r('creature', { preset: 'skeleton' }), r('creature', { preset: 'drake' })] },
    { id: 'item-workflow', label: 'Silhouette, three-value shading, material cue, one accent, icon-size check', recipes: [r('item', { shape: 'axe' }), r('item', { shape: 'chest' })] },
    { id: 'character-front', label: '32x32 character, front view', recipes: [r('character', { loadout: 'guardian' }), r('character', { loadout: 'scholar' })] },
    { id: 'character-side-back', label: 'Character side, back and three-quarter views', recipes: [r('character', { loadout: 'guardian', view: 'side' }), r('character', { loadout: 'guardian', view: 'back' }), r('character', { loadout: 'guardian', view: 'quarter' })] },
    { id: 'walk-cycle', label: '4 to 6 frame walk cycle, key poses, largest masses first', recipes: [r('character', { loadout: 'rogue' })], checks: ['frames', 'boil', 'length'] },
    { id: 'secondary-motion', label: 'Cloth, flags and other secondary animation', recipes: [r('cloth', { kind: 'flag' }), r('cloth', { kind: 'cape', ramp: 'blue' }), r('cloth', { kind: 'hair', ramp: 'wood' })], checks: ['frames', 'boil', 'length'] },
    { id: 'tile-transitions', label: 'Grass to dirt, dirt to stone, water edge', recipes: [r('tile', { kind: 'transition', from: 'grass', to: 'dirt' }), r('tile', { kind: 'transition', from: 'dirt', to: 'stone' }), r('tile', { kind: 'water' })] },
    { id: 'tree-bush-rock', label: 'Tree, bush, rock and cliff clusters', recipes: [r('prop', { kind: 'tree' }), r('prop', { kind: 'bush' }), r('prop', { kind: 'rock' })], partial: 'no cliffs' },
    { id: 'buildings', label: 'Roof, wall, door and window construction', recipes: [r('prop', { kind: 'house' }), r('prop', { kind: 'door' })] },
    { id: 'interiors', label: 'Interior floors, walls, furniture and lighting', recipes: [r('room', { room: 'cottage' }), r('room', { room: 'library', time: 'night' })] },
    { id: 'dungeon-layout', label: 'Corridors, treasure rooms, hazard placement', recipes: [r('scene', { biome: 'dungeon' })], partial: 'one scene layout; no rooms or hazards' },
  ],
};
