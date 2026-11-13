// Models docs/cc0-faceart-reference-kit.md. Posture: study material (Duchenne plates, Rawpixel portraits, 3D models); drawn art is original.
const r = (recipe, params = {}) => ({ recipe, params });
export default {
  id: 'faceart',
  title: 'CC0 / Public-Domain Art Reference Kit: Faces, Expressions, Creatures',
  doc: 'docs/cc0-faceart-reference-kit.md',
  workflow: ['Duchenne: isolate the muscle mechanics of the expression', 'Add a modern portrait for age, likeness, lighting, skin', 'Translate the expression onto a creature, keep its emotional silhouette', 'Historical monster art for exaggeration', '3D assets to rotate, light and test volume'],
  // The kit's four axes of range, as the face recipe's parameters.
  axes: { age: ['child', 'adult', 'elder'], phenotype: ['skin-fair', 'skin-light', 'skin-medium', 'skin-deep'], angle: ['front', 'three-quarter'], intensity: [1, 2, 3] },
  subjects: [
    { id: 'expression-mechanics', label: 'Surprise, pain, fear, attention, smiling, anger as separate brow, eye and mouth moves', recipes: ['surprise', 'pain', 'fear', 'attention', 'joy', 'anger'].map((expression) => r('face', { expression })) },
    { id: 'range-age', label: 'Age range', recipes: ['child', 'adult', 'elder'].map((age) => r('face', { age })) },
    { id: 'range-phenotype', label: 'Ancestry / phenotype range', recipes: ['skin-fair', 'skin-light', 'skin-medium', 'skin-deep'].map((skin) => r('face', { skin })), partial: 'four skin ramps; no feature variation (nose, eye shape) beyond tone' },
    { id: 'range-angle', label: 'Head angle', recipes: [r('face', { angle: 'front' }), r('face', { angle: 'three-quarter' })], partial: 'no profile or back view' },
    { id: 'range-intensity', label: 'Emotional intensity', recipes: [1, 2, 3].map((intensity) => r('face', { expression: 'fear', intensity })) },
    { id: 'stylized-portrait-sets', label: 'Game-facing portrait sets and avatar variation', recipes: [r('face', { style: 'long' }), r('face', { style: 'hood' }), r('face', { style: 'bald' })] },
    { id: 'modular-people', label: 'Modular people: bodies, hair, clothing, poses (Open Peeps)', recipes: [r('character', { loadout: 'rogue' }), r('character', { loadout: 'healer' })], partial: 'pixel characters, not the hand-drawn Open Peeps style' },
    { id: 'creature-faces', label: 'Creature faces from animal anatomy, one system changed at a time', recipes: [r('creature', { preset: 'wolf' }), r('creature', { preset: 'bat' })], mutates: ['eyes', 'jaw', 'horns', 'skin', 'ears'] },
    { id: 'historical-monsters', label: 'Dragons, demons, chimeras, harpies', recipes: [r('creature', { preset: 'drake' }), r('creature', { preset: 'bat' })], partial: 'dragon and bat only; no demon, chimera or harpy body plan' },
    { id: 'reference-3d-lighting', label: '3D monster models, HDRIs, materials for lighting study', reference: 'Poly Haven, OpenGameArt 50+ Monsters Pack 3D: study material, not drawable' },
    { id: 'reference-portraits', label: 'Rawpixel and Wikimedia portrait and monster plates', reference: 'high-resolution photographs and engravings: study material, not drawable' },
  ],
};
