// Models docs/cc0-fantasy-biomes-environments-reference-kit.md. Posture: reference; scenes and tiles are original.
const r = (recipe, params = {}) => ({ recipe, params });
const scenes = (...biomes) => biomes.map((biome) => r('scene', { biome }));
export default {
  id: 'fantasy-biomes-environments',
  title: 'CC0 / Public-Domain Art Reference Kit: Fantasy Biomes, Environments & Settings',
  doc: 'docs/cc0-fantasy-biomes-environments-reference-kit.md',
  workflow: ['Landscape paintings for composition and atmosphere', 'Tilesets for readable terrain shorthand', 'HDRIs and materials for real lighting', 'Build the biome from layers', 'Design for gameplay readability'],
  // The kit's six biome layers; the BIOMES table in recipes/tiles.js carries geology (ground, wall), vegetation (props), water, architecture (landmark).
  layers: ['geology', 'vegetation', 'weather', 'architecture', 'inhabitants', 'story evidence'],
  subjects: [
    { id: 'forest', label: 'Forests and woodlands', recipes: scenes('forest') },
    { id: 'mountain-cave-dungeon', label: 'Mountains, caves, dungeons', recipes: [...scenes('cave', 'dungeon', 'mines', 'highlands'), r('prop', { kind: 'peak' }), r('prop', { kind: 'mineentrance' }), r('prop', { kind: 'ropebridge' })] },
    { id: 'desert-ruins', label: 'Deserts, wastelands, ruins', recipes: [...scenes('desert', 'ruins'), r('prop', { kind: 'ruinarch' }), r('prop', { kind: 'ruinwall' }), r('prop', { kind: 'dune' })] },
    { id: 'village-castle', label: 'Villages, towns, castles', recipes: [...scenes('village', 'castle', 'town'), ...['house', 'castle', 'gatehouse', 'battlement', 'stall', 'well', 'signpost'].map((kind) => r('prop', { kind }))] },
    { id: 'coast-river-swamp', label: 'Coastal, river and swamp settings', recipes: [...scenes('coast', 'swamp', 'harbor', 'river'), ...['dock', 'boat', 'ship', 'bridge', 'ropebridge'].map((kind) => r('prop', { kind }))] },
    { id: 'magical', label: 'Magical and otherworldly settings', recipes: [...scenes('magical', 'skyisle'), r('prop', { kind: 'floatisle' }), r('prop', { kind: 'portal' })] },
    { id: 'terrain-transitions', label: 'Terrain transitions', recipes: [r('tile', { kind: 'transition', from: 'grass', to: 'dirt' }), r('tile', { kind: 'transition', from: 'dirt', to: 'stone' })] },
    { id: 'biome-layers', label: 'Geology, vegetation, weather, architecture, inhabitants, story evidence', provides: 'BIOMES table in recipes/tiles.js (geology, vegetation, architecture); SCENE_LAYERS in recipes/scene-layers.js (weather, inhabitants, story evidence), drawn by scene with layers: true or weather, inhabitants and evidence on their own', recipes: [r('scene', { biome: 'forest', layers: true }), r('scene', { biome: 'ruins', layers: true }), r('scene', { biome: 'town', layers: true })] },
    { id: 'readability', label: 'Focal point, depth, pathing, scale, palette', provides: 'drawScene: a path and a landmark at its end; one ground ramp per biome; depth: true adds a hazed background of far props, the midground, and dark cropped foreground shapes under a shadow dither', recipes: [r('scene', { biome: 'highlands', depth: true }), r('scene', { biome: 'harbor', layers: true })] },
    { id: 'reference-paintings-hdri', label: 'Rawpixel landscape paintings, Poly Haven HDRIs', reference: 'study material, not drawable' },
  ],
};
