// Models docs/cc0-fantasy-items-equipment-reference-kit.md. Posture: reference; the drawn icons are original.
const r = (recipe, params = {}) => ({ recipe, params });
const thing = (params) => ({ recipe: 'thing', params, style: 'ansi', lint: { skip: ['outline', 'contrast', 'ramp'] } });
const items = (...shapes) => shapes.map((shape) => r('item', { shape }));
export default {
  id: 'fantasy-items-equipment',
  title: 'CC0 / Public-Domain Art Reference Kit: Fantasy Items & Equipment',
  doc: 'docs/cc0-fantasy-items-equipment-reference-kit.md',
  workflow: ['Historical armor and weapons for construction and wear', 'Game-asset collections for silhouette and icon design', '3D models to rotate and light', 'Poly Haven for materials', 'Design in tiers'],
  // The kit's equipment tiers are the recipe's `tier` parameter.
  tiers: ['mundane', 'crafted', 'enchanted', 'legendary'],
  subjects: [
    { id: 'weapons', label: 'Swords, axes, hammers, spears, bows, daggers, staves', recipes: [...items('sword', 'rapier', 'sabre', 'greatsword', 'axe', 'axe2', 'axegreat', 'mace', 'hammer', 'bow', 'dagger', 'staff'), ...['a spear', 'a trident', 'a halberd', 'a lance', 'a scythe'].map((name) => thing({ name }))], provides: 'spears and polearms (spear, trident, halberd, glaive, lance, scythe) in the `thing` recipe, in ANSI' },
    { id: 'armor-and-shields', label: 'Armor, helmets, boots, shields', recipes: [...items('armor', 'spikemail', 'helm', 'visor', 'crownhelm', 'hornhelm', 'hat', 'hood', 'headband', 'boots', 'shield', 'buckler', 'targe'), thing({ name: 'iron gauntlets' }), thing({ name: 'steel greaves' })], provides: 'gauntlets and greaves in the `thing` recipe, in ANSI' },
    { id: 'inventory-icons', label: 'Compact inventory icons at 16x16', recipes: items('sword', 'ring', 'potion', 'shield', 'helm') },
    { id: 'potions-alchemy', label: 'Potion bottles, flasks, ingredient jars, herbs, scrolls', recipes: [...items('potion', 'vial', 'goblet', 'scroll', 'parchment'), thing({ name: 'a clay jar' }), thing({ name: 'a sprig of sage' })], provides: 'jars and herbs in the `thing` recipe, in ANSI' },
    { id: 'treasure-currency', label: 'Chests, coins, gems, jewellery, keys, relics', recipes: [...items('chest', 'coin', 'gem', 'ring', 'key', 'amulet', 'starcharm', 'fang', 'bonecharm', 'heart'), thing({ name: 'a treasure map' }), thing({ name: 'a golden idol' }), thing({ name: 'an old skull' })], provides: 'maps and relics (an idol, a skull, an orb) in the `thing` recipe, in ANSI' },
    { id: 'camping-travel', label: 'Tents, bedrolls, lanterns, torches, ropes, canteens, backpacks', recipes: [...items('torch'), thing({ name: 'a torch' }), thing({ name: 'a lantern' }), thing({ name: 'a bag' })], partial: 'a torch, a lantern, bags, barrels and cups; no tents, bedrolls or ropes' },
    { id: 'books-arcane', label: 'Spellbooks, wands, staves, crystals, runes', recipes: [...items('book', 'staff', 'gem', 'scroll'), r('prop', { kind: 'crystal' }), thing({ name: 'a wand' }), thing({ name: 'a rune' })], provides: 'wands and rune stones in the `thing` recipe, in ANSI' },
    { id: 'equipment-tiers', label: 'Mundane, crafted, enchanted, legendary visual language', recipes: ['mundane', 'crafted', 'enchanted', 'legendary'].map((tier) => r('item', { shape: 'sword', tier })) },
    { id: 'worn-equipment', label: 'Equipment visible on a character', recipes: [r('character', { loadout: 'guardian', tier: 'legendary' }), r('character', { loadout: 'berserker', tier: 'enchanted' })] },
    { id: 'reference-3d-materials', label: 'Rotatable 3D items, Poly Haven metal, leather, wood, cloth, stone, glass', reference: 'study material, not drawable' },
    { id: 'reference-historical', label: 'Rawpixel armor and medieval weapon plates', reference: 'museum photographs and illustrations: study material, not drawable' },
  ],
};
