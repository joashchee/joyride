// Models docs/cc0-hands-poses-turnarounds-motion-interiors-kit.md. Posture: study material (pose plates, turnaround sheets, flag packs,
// tilesets, HDRIs); the drawn hands, stances, views, cloth and rooms are original recipes.
const r = (recipe, params = {}) => ({ recipe, params });
const hands = (stage) => ['open', 'fist', 'point', 'cast', 'thumbsup'].map((gesture) => r('hand', { gesture, stage }));
const pose = (...poses) => poses.map((p) => r('character', { loadout: 'rogue', pose: p }));
export default {
  id: 'hands-poses-motion-interiors',
  title: 'CC0 / Public-Domain Art Reference Kit: Hands, Poses, Turnarounds, Secondary Motion & Interiors',
  doc: 'docs/cc0-hands-poses-turnarounds-motion-interiors-kit.md',
  workflow: ['Hand and pose references for gesture, weight and anatomy', 'Turnaround sheets to keep a character consistent across views', 'Flag and cloth references for follow-through, overlap and drag', 'Interior tilesets and references for readable rooms, furniture and light', 'Study gesture first, then construction, then pixels'],
  // The kit's workflows as parameters: the hand workflow is `stage`, the turnaround is `view`, the interior layers are a room's data.
  axes: { handStage: ['mitten', 'blocks', 'detailed'], view: ['front', 'side', 'back'], wind: ['calm', 'steady', 'gust'], time: ['day', 'dusk', 'night'] },
  layers: ['architecture', 'furniture', 'props', 'lighting', 'storytelling'],
  subjects: [
    { id: 'hand-workflow', label: 'Hand study workflow: mitten, blocks, then detail', recipes: [...hands('mitten'), ...hands('blocks'), ...hands('detailed')] },
    { id: 'hand-gestures', label: 'Gestures: open, grip, point, cast, ward', recipes: ['open', 'ward', 'fist', 'point', 'thumbsup', 'cast', 'grip'].map((gesture) => r('hand', { gesture })), partial: 'right hand from the back only; no palm view, no foreshortening, no hand holding an item' },
    { id: 'pose-stances', label: 'Standing, walking, running, sitting, crouching, reaching', recipes: [r('character', { loadout: 'guardian' }), ...pose('run', 'sit', 'crouch', 'reach')] },
    { id: 'pose-action', label: 'Combat, casting, climbing, carrying, falling', recipes: [r('character', { loadout: 'berserker' }), ...pose('cast', 'carry', 'fall', 'brace')], partial: 'no climbing; combat is the attack clip, not a held pose' },
    { id: 'pose-posture', label: 'Character posture: confident, injured, sneaky, exhausted, regal', recipes: pose('confident', 'injured', 'sneak', 'exhausted', 'regal') },
    { id: 'pose-weight', label: 'Weight distribution and balance line', provides: 'POSES in recipes/characters.js: lean, squat, lift and foot offsets are joints, not new art', partial: 'balance is set by hand; nothing checks the balance line' },
    { id: 'pose-foreshortening', label: 'Foreshortening in arms, legs and hands', recipes: ['guardian', 'rogue', 'scholar'].flatMap((loadout) => ['point', 'kick'].map((pose) => r('character', { loadout, pose }))), checks: ['value', 'palette', 'outline'], partial: 'front view only: an arm and a leg toward the viewer; no foreshortened hand gesture or side-view limb' },
    { id: 'reference-poses', label: 'Art in Context pose plates, Rawpixel and Wikimedia hand studies, OpenGameArt sprites', reference: 'Muybridge studies, old-master hands, photographs: study material, not drawable' },

    { id: 'turn-front', label: 'Front view: face, torso, limbs, costume, proportions', recipes: ['guardian', 'berserker', 'rogue'].map((loadout) => r('character', { loadout })) },
    { id: 'turn-side', label: 'Side view: head profile, spine, chest depth, limb overlap', recipes: ['guardian', 'berserker', 'rogue', 'scholar', 'healer'].map((loadout) => r('character', { loadout, view: 'side' })) },
    { id: 'turn-back', label: 'Back view: hair, cloak, backpack, armour plates, heels', recipes: [...['guardian', 'rogue', 'scholar'].map((loadout) => r('character', { loadout, view: 'back' })), ...['cape', 'quiver', 'pack'].map((back) => r('character', { loadout: 'guardian', view: 'back', back }))] },
    { id: 'turn-three-quarter', label: 'Three-quarter view connecting front and side', recipes: ['guardian', 'rogue', 'scholar'].map((loadout) => r('character', { loadout, view: 'quarter' })), checks: ['value', 'palette', 'outline'] },
    { id: 'turn-consistency', label: 'Same proportions, density and palette in every view; accessories placed logically', recipes: ['front', 'side', 'back'].map((view) => r('character', { loadout: 'rogue', view, back: 'quiver' })), checks: ['value', 'palette', 'outline'] },
    { id: 'turn-walk', label: 'Walk cycle in side and back views', recipes: [r('character', { loadout: 'guardian', view: 'side' }), r('character', { loadout: 'guardian', view: 'back', back: 'cape' })], checks: ['frames', 'boil', 'length'] },
    { id: 'reference-turnarounds', label: 'OpenGameArt turnaround sheet, 64x64 four-direction character, RPG sprite templates', reference: 'CC0 sheets: study the construction, never copied' },

    { id: 'motion-flags', label: 'Flags: a 16-frame wave becomes a short loop', recipes: ['calm', 'steady', 'gust'].map((wind) => r('cloth', { kind: 'flag', wind })) },
    { id: 'motion-capes', label: 'Capes and cloaks: follow-through and drag', recipes: [r('cloth', { kind: 'cape', ramp: 'blue' }), r('cloth', { kind: 'cape', ramp: 'blue', drag: 3 }), r('character', { loadout: 'guardian', view: 'side', back: 'cape' }), r('character', { loadout: 'guardian', view: 'back', back: 'cape' })] },
    { id: 'motion-hair', label: 'Hair blowing in wind', recipes: [r('cloth', { kind: 'hair', ramp: 'wood' }), r('cloth', { kind: 'hair', ramp: 'sand', wind: 'gust' })] },
    { id: 'motion-hangings', label: 'Banners and curtains in an interior', recipes: [r('cloth', { kind: 'banner', emblem: 'diamond' }), r('cloth', { kind: 'curtain', ramp: 'purple', tie: true })] },
    { id: 'motion-robes', label: 'Skirt or robe during a turn', recipes: ['calm', 'steady', 'gust'].map((wind) => r('cloth', { kind: 'robe', wind })), checks: ['frames', 'length'] },
    { id: 'motion-principles', label: 'Follow-through, overlap, drag, anchor points, fewer frames', provides: 'cloth.js: a pinned anchor, a wave that grows and lags with distance from it, a `drag` offset, 4 or 6 frame loops from one phase', checks: ['frames', 'boil', 'length'] },
    { id: 'reference-motion', label: 'Animated Flags Pack, Lospec fabric tutorials, OPP2017 sprites', reference: '458 CC0 flags and tutorials: study timing, never copied' },

    { id: 'interior-architecture', label: 'Walls, floors, doorways and windows', recipes: [r('tile', { kind: 'planks' }), r('tile', { kind: 'flagstone' }), r('tile', { kind: 'checker' }), r('furniture', { kind: 'window' }), r('furniture', { kind: 'doorway' })], partial: 'no stairs, ceilings or beams' },
    { id: 'interior-furniture', label: 'Beds, tables, chairs, shelves, altars, workbenches, storage', recipes: ['bed', 'table', 'chair', 'shelf', 'hearth', 'altar', 'workbench', 'anvil', 'barrel', 'chest'].map((kind) => r('furniture', { kind })) },
    { id: 'interior-props', label: 'Books, candles, food, tools, rugs, pottery and personal objects', recipes: ['candle', 'rug', 'torch', 'bones'].map((kind) => r('furniture', { kind })), partial: 'clues are books, mugs, scrolls, skulls, coins, ingots and potions; no food or pottery' },
    { id: 'interior-lighting', label: 'Windows, fireplaces, lanterns, torches: time of day and mood', recipes: [r('room', { room: 'cottage', time: 'day' }), r('room', { room: 'cottage', time: 'night' }), r('room', { room: 'cell' })] },
    { id: 'interior-storytelling', label: 'Ownership, profession, wealth, recent events', recipes: ['cottage', 'smithy', 'library', 'shrine', 'cell'].map((room) => r('room', { room })), partial: 'five rooms, each with a one-line story; no tavern, throne room or ruin' },
    { id: 'interior-readability', label: 'Focal point, entrances and walkable space, clean wall/floor transition, scale', provides: 'layoutOf, walkable and unreachable in recipes/interiors.js: the door, the focal item and every piece of furniture reachable on foot', partial: 'scale to the character is by eye; lighting is checked by the pictures, not a rule' },
    { id: 'reference-interiors', label: 'OpenGameArt tilesets, Poly Haven HDRIs and materials', reference: 'tilesets, HDRIs and materials: study material, not drawable' },
  ],
};
