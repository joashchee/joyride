// Models docs/free-asset-sources-study.md. Posture: study material; none of the listed art is bundled, traced or drawn, and each row is
// a category Painter has no recipe for yet. Take a row off this list by drawing an original recipe for it.
export default {
  id: 'free-asset-sources',
  title: 'Free Asset Sources: Catalog Gaps',
  doc: 'docs/free-asset-sources-study.md',
  workflow: ['Read the category as a list of kinds, not as art to copy', 'Draw an original recipe from the DB32 ramps', 'Register it with variants() and move the row to a real subject'],
  subjects: [
    { id: 'vehicles', label: 'Vehicles: cars, ships, carts, planes', recipes: ['wagon', 'sled', 'car', 'locomotive', 'airship', 'raft', 'cannon'].map((kind) => ({ recipe: 'vehicle', params: { kind } })).concat(['boat', 'cart', 'ship', 'caravan', 'spaceship'].map((kind) => ({ recipe: 'prop', params: { kind } }))), partial: 'side-view vehicles at 32x32 only; no planes, top-down or isometric views' },
    { id: 'buildings', label: 'Buildings and city props: exteriors, roads, street furniture', recipes: ['barn', 'windmill', 'lighthouse', 'tower', 'chapel', 'inn', 'watchtower'].map((kind) => ({ recipe: 'building', params: { kind } })).concat(['house', 'clanhall', 'shrine', 'tent'].map((kind) => ({ recipe: 'prop', params: { kind } }))), partial: 'front-view exteriors at 32x32; no roads, street furniture or modern city blocks' },
    { id: 'weapon-family', label: 'Weapons as a tracked family (kinds, not single items)', recipes: ['sword', 'dagger', 'rapier', 'sabre', 'scimitar', 'katana', 'greatsword', 'axe', 'axe2', 'axegreat', 'mace', 'hammer', 'staff', 'wand', 'spear', 'halberd', 'trident', 'javelin', 'bow', 'sling', 'whip', 'net', 'lasso', 'arquebus', 'cannon'].map((shape) => ({ recipe: 'item', params: { shape } })), partial: 'melee, ranged, thrown, cord and firearm icons at 16x16; no sci-fi or energy weapons, crossbow or ammunition' },
    { id: 'farm-food', label: 'Farm and food: crops, produce, meals, animals', recipes: ['wheat', 'carrot', 'pumpkin', 'corn', 'turnip', 'grapes', 'egg', 'mushroom', 'hen', 'sheep'].map((kind) => ({ recipe: 'farm', params: { kind } })), partial: '16x16 crops, produce and two animals; no fields, tilled rows or a wider herd (`thing` draws bread, fruit, meat, cheese and fish in ANSI)' },
    { id: 'tower-strategy', label: 'Tower-defense and strategy pieces: towers, hex tiles', recipes: ['hex-grass', 'hex-forest', 'hex-mountain', 'hex-water', 'hex-desert', 'hex-snow', 'tower-archer', 'tower-cannon', 'tower-mage', 'tower-frost'].map((kind) => ({ recipe: 'strategy', params: { kind } })), partial: '16x16 pointy-top hex tiles (six terrains) and four towers; no enemies, projectiles, paths or square-grid tiles' },
    { id: 'one-bit-patterns', label: '1-bit patterns and tiles', recipes: ['checker', 'checker4', 'stripes', 'hstripes', 'diagonal', 'dots', 'grid', 'bricks', 'diamonds', 'waves', 'rings', 'weave', 'hatch', 'noise'].map((kind) => ({ recipe: 'pattern', params: { kind } })) },
    { id: 'reference-sources', label: 'The source lists themselves, after the licence check', reference: 'verified CC0: Kenney, Quaternius (3D). Excluded: assethoard (no licence of its own), jam-ready RUN-licence packs (RUN projects only), Tiddybub (blanket CC0 claim unverified). Gists: pointers only. Art never bundled or used as a fixture' },
  ],
};
