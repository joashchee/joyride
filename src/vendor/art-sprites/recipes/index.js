// The recipe registry: every way the system can draw something, by id. A recipe is `(params) => Art` where Art is
// `{ id, frames, fps, loop, clips?, layers?, anchors?, spec? }`. Add a recipe here and a kit subject can point at it.
import { drawItem, SHAPE_IDS, TIERS } from './items.js';
import { drawMaterial, drawCurrency, MATERIAL_KINDS, MATERIAL_IDS, DENOMINATIONS, KEY_LOOKS } from './material.js';
import { drawFace, EXPRESSIONS } from './faces.js';
import { drawCreature, PRESETS, RACES } from './creatures.js';
import { drawCharacter, drawPortrait, LOADOUTS, POSE_IDS, VIEWS, BACK_SLOTS, FACINGS } from './characters.js';
import { drawHand, GESTURE_IDS, STAGES } from './hands.js';
import { drawCloth, CLOTH_KINDS } from './cloth.js';
import { drawIcon, GLYPH_IDS } from './icons.js';
import { drawRoom, floorTile, dungeonTile, DUNGEON_BIOMES, FLOOR_KINDS, FURNITURE, FURNITURE_IDS, ROOM_IDS } from './interiors.js';
import { drawTopTile, TOP_PARTS, TOP_BIOMES } from './dungeon.js';
import { drawScene, drawProp, groundTile, wallTile, waterTile, transitionTile, BIOMES, PROP_IDS } from './tiles.js';
import { drawPattern, PATTERN_IDS } from './patterns.js';
import { drawBuilding, BUILDING_IDS } from './buildings.js';
import { drawExhibit, drawAward, EXHIBIT_STYLES, AWARD_KINDS, AWARD_TIERS } from './exhibit.js';
import { drawService, drawTown, SERVICE_IDS, BUILDING_STATES } from './town.js';
import { drawDress, TEMPLATE_IDS } from './dress.js';
import { drawMap } from './map.js';
import { drawTopProp, TOP_PROPS, TOP_PROP_IDS } from './topprops.js';
import { TOP_FURNITURE_IDS } from './topfurniture.js';
import { drawVehicle, VEHICLE_IDS } from './vehicles.js';
import { drawFarm, FARM_IDS } from './farm.js';
import { drawStrategy, STRATEGY_IDS } from './strategy.js';
import { drawEffect, EFFECT_IDS } from './effects.js';
import { drawTelegraph, drawTarget, TELEGRAPH_SHAPES, TELEGRAPH_KINDS, TARGET_KINDS, DIRECTIONS } from './telegraph.js';
import { drawSkillIcon, SKILL_CATEGORY_IDS, SKILL_TARGETS } from './skillicon.js';
import { drawOverlay, RARITY_IDS, MARK_IDS } from './overlay.js';
import { Pixels } from '../pixels.js';
import { withMotion } from '../motion.js';
import { resolveMotion } from '../motion-types.js';
import { foldArt } from '../cells.js';
import { textFromAnsiArt, textFromPixelArt } from '../text-art.js';
import { drawVista, TERRAINS, TIMES as VISTA_TIMES, WEATHER_KINDS } from './vista.js';
import { drawSigil, SIGIL_ORDINARIES, SIGIL_CHARGES } from './heraldry.js';
import { drawBar, drawMappane, drawSheet } from './ui.js';
import { drawLocale, LOCALE_KINDS } from './locales.js';
import { drawThing, THING_KINDS, THING_CLASSES, THING_AURAS } from './item-art.js';
import { drawCreation, RACE_NAMES, CLASS_NAMES } from './portrait.js';
import { drawCreatureFace, FACE_SHAPES, FACE_MOODS, FACE_LOOKS, HURT_STEPS } from './face.js';
import { drawEmblem, EMBLEM_LOADOUTS } from './emblem.js';
import { drawFight, FIGHT_LAYOUTS } from './fight.js';
import { decorate } from '../art-meta.js';
import { drawPlaceholder, PLACEHOLDER_FAMILIES, PLACEHOLDER_CATEGORIES, PLACEHOLDER_SIZES } from './placeholder.js';
import { drawOnPlace } from './onplace.js';
import { drawSplash, drawSplashAnsi, SPLASH_SCENES } from './splash.js';
import { drawBanner, drawLandscape, drawMenu, drawBust, drawArtpack, drawWeatherLoop, LOOP_WEATHERS, BANNER_FACES, TIMES, WEATHERS, SKIN_IDS, LIGHTS } from './ansi.js';

// Each smooth-colour recipe is registered at all three depths; 16 is the plain variant, so its golden hashes stay as they were.
const atDepths = (list) => () => list().flatMap((v) => [v, { ...v, colors: 256 }, { ...v, colors: 'true' }]);

// The small level the dress and map variants draw.
const DRESS_GRID = ['##########', '#..o.....#', '#..o..^..#', '+........>', '#....#...#', '##########'];
const DRESS_GATES = ['closed', 'open', 'locked'];
const GATE_GRID = ['#####=####', '#........#', '#........#', '+........>', '#........#', '##########'];
const MAP_CELLS = ['#####', '#...#', '#...#', '#...#', '#####'].flatMap((row, y) => [...row].map((ch, x) => ({ x, y, kind: ch === '#' ? 'wall' : 'floor', state: y === 0 || x === 4 ? 'unknown' : x === 3 ? 'remembered' : y === 3 ? 'explored' : 'visible' })));

/** A picture the game names as `{ recipe, params }` becomes that recipe's art; a Pixels or art passes through. */
const resolvePicture = (p) => (p && p.recipe ? draw(p.recipe, p.params ?? {}) : p);

const art = (id, px) => ({ id, frames: [px], fps: 0, loop: false });

export const RECIPES = {
  item: { describe: 'Item or equipment icon (16x16, or 24 or 32 with `iconSize`, drawn again from the same shape with wider shading, a longer glint, more grain and a bigger tier mark): shape, tier, material', variants: () => [...SHAPE_IDS.flatMap((shape) => Object.keys(TIERS).map((tier) => ({ shape, tier }))), ...[24, 32].flatMap((iconSize) => SHAPE_IDS.map((shape) => ({ shape, tier: 'crafted', iconSize }))), ...Object.keys(TIERS).map((tier) => ({ shape: 'sword', tier, iconSize: 32 }))], draw: drawItem },
  material: { describe: 'Crafting material (16x16): ore, herb, hide, monster part or crystal, with a `variant` index into its looks (ore: iron, copper, silver, gold, mithril; crystal: clear, ruby, amethyst, emerald, sun)', variants: () => MATERIAL_IDS.flatMap((kind) => MATERIAL_KINDS[kind].looks.map((_, variant) => ({ kind, variant }))), draw: drawMaterial },
  currency: { describe: 'Money and what stands for it (16x16): coins of four denominations (copper, silver, gold, platinum; a different mark as well as a colour) in a stack of 1 to 3, a purse, keys (bronze, silver, gold, skeleton), a fragment, a plan and a quest token', variants: () => [...Object.keys(DENOMINATIONS).flatMap((denomination) => [{ kind: 'coin', denomination }, { kind: 'coin', denomination, stack: 3 }, { kind: 'purse', denomination }]), ...KEY_LOOKS.map((look) => ({ kind: 'key', look })), { kind: 'fragment', denomination: 'silver' }, { kind: 'plan' }, { kind: 'token' }], draw: drawCurrency },
  face: { describe: 'Face portrait (16x16): expression, age, phenotype, angle, intensity', variants: () => Object.keys(EXPRESSIONS).map((expression) => ({ expression })), draw: drawFace },
  creature: { describe: 'Creature (32x32) from a body plan, a preset or a CoffeeMUD race (size table); mutate() changes one system', variants: () => [...Object.keys(PRESETS).map((preset) => ({ preset })), ...Object.keys(RACES).map((race) => ({ race }))], draw: drawCreature },
  character: { motion: 'native', describe: 'Modular paper-doll character (32x32): view (front, side, back, quarter, or top for the top-down world view with a north, east, south or west facing), pose, back slot (cape, quiver, pack), layers and clips', variants: () => [...Object.keys(LOADOUTS).map((loadout) => ({ loadout })), ...VIEWS.slice(1).flatMap((view) => Object.keys(LOADOUTS).map((loadout) => ({ loadout, view }))), ...POSE_IDS.map((pose) => ({ loadout: 'rogue', pose })), ...BACK_SLOTS.flatMap((back) => VIEWS.map((view) => ({ loadout: 'guardian', view, back }))), ...Object.keys(LOADOUTS).map((loadout) => ({ loadout, view: 'top' })), ...['guardian', 'berserker', 'rogue', 'scholar', 'healer'].flatMap((loadout) => FACINGS.map((facing) => ({ loadout, view: 'top', facing }))), ...BACK_SLOTS.flatMap((back) => FACINGS.map((facing) => ({ loadout: 'guardian', view: 'top', facing, back }))), ...POSE_IDS.map((pose) => ({ loadout: 'rogue', view: 'top', pose }))], draw: drawCharacter },
  emblem: { describe: 'A class emblem (16x16): the sigil shield in the loadout\'s colour with the icon glyph of its weapon, for portrait frames and party panels', variants: () => EMBLEM_LOADOUTS.map((loadout) => ({ loadout })), draw: drawEmblem },
  portrait: { describe: 'Portrait of a character: size party (16x16 head), status (20x20 bust, the default) or card (32x32 figure); the nine face expressions; normal, defeated or recalling; returns a mask and a safe box', variants: () => [...Object.keys(LOADOUTS).map((loadout) => ({ loadout })), ...['party', 'card'].map((size) => ({ loadout: 'healer', size })), ...['defeated', 'recalling'].map((state) => ({ loadout: 'healer', state })), { loadout: 'healer', expression: 'joy' }], draw: drawPortrait },
  icon: { describe: 'Pictogram icon (16x16): a symbol on a plate, for badges, emblems, skills, statuses and resources', variants: () => GLYPH_IDS.map((glyph) => ({ glyph })), draw: drawIcon },
  scene: { describe: 'Biome scene (12x8 tiles) with path, props and a landmark; layers: true adds the biome weather, inhabitants, story evidence and depth (each also a param: weather by name, inhabitants, evidence, depth)', variants: () => [...Object.keys(BIOMES).map((biome) => ({ biome })), ...Object.keys(BIOMES).map((biome) => ({ biome, layers: true }))], draw: drawScene },
  prop: {
    describe: 'Environment prop or building; view top draws 26 props from above on the 32 pixel grid, with a state, footprint, occlusion and loop clips (flicker, glow, swirl, open, trigger, reveal)',
    variants: () => [...PROP_IDS.map((kind) => ({ kind })), ...TOP_PROP_IDS.flatMap((kind) => (TOP_PROPS[kind].states?.length ? TOP_PROPS[kind].states : [undefined]).map((state) => ({ kind, view: 'top', ...(state ? { state } : {}) })))],
    draw: ({ kind = 'tree', view, ...rest } = {}) => { if (view === 'top') return drawTopProp({ kind, ...rest }); if (view !== undefined) throw new Error(`unknown prop view "${view}"; known: top`); return art(`prop.${kind}`, drawProp(kind)); },
  },
  tile: {
    describe: 'Tileable ground, wall, water, or a transition between two grounds; kind dungeon with view top is the top-down set: 8 biomes, 16 or 32 pixels, floor, flat wall tops with a 47-shape autotile (mask and notch), pillar, broken wall, doorway, stairs, pit, lava, toxic, bridge, rubble, cracks and a shade overlay',
    variants: () => [{ kind: 'ground', ramp: 'grass' }, { kind: 'wall', ramp: 'stone' }, { kind: 'water' }, ...FLOOR_KINDS.map((kind) => ({ kind })), ...DUNGEON_BIOMES.flatMap((biome) => ['floor', 'wall'].map((part) => ({ kind: 'dungeon', biome, part }))), { kind: 'transition', from: 'grass', to: 'dirt' }, { kind: 'transition', from: 'dirt', to: 'stone' },
      ...TOP_BIOMES.flatMap((biome) => [{ part: 'floor' }, { part: 'floor', mask: 3 }, { part: 'wall', mask: 15 }, { part: 'wall', mask: 6, notch: 0 }, { part: 'wall', mask: 15, notch: 2 }, { part: 'floor', size: 16 }, { part: 'wall', size: 16, mask: 10 }].map((v) => ({ kind: 'dungeon', view: 'top', biome, ...v }))),
      ...TOP_PARTS.filter((p) => !['floor', 'wall'].includes(p)).flatMap((part) => ['crypt', 'volcanic'].map((biome) => ({ kind: 'dungeon', view: 'top', biome, part, mask: part === 'pit' || part === 'lava' || part === 'toxic' ? 5 : 0 }))),
      ...['north', 'east'].flatMap((facing) => [{ part: 'doorway', facing, state: 'closed' }, { part: 'doorway', facing, state: 'locked' }, { part: 'stairs-down', facing }, { part: 'bridge', facing }].map((v) => ({ kind: 'dungeon', view: 'top', biome: 'ancient', ...v })))],
    draw: ({ kind = 'ground', ramp = 'grass', from = 'grass', to = 'dirt', seed = 'tile', biome = 'crypt', part = 'floor', view, ...top } = {}) => {
      if (view !== undefined && view !== 'top') throw new Error(`unknown tile view "${view}"; known: top`);
      if (view === 'top') { if (kind !== 'dungeon') throw new Error('the top-down tile set is kind "dungeon"'); return drawTopTile({ biome, part, seed, ...top }); }
      return art(`tile.${kind}`, kind === 'dungeon' ? dungeonTile(biome, part, seed) : FLOOR_KINDS.includes(kind) ? floorTile(kind, seed) : kind === 'wall' ? wallTile(ramp, seed) : kind === 'water' ? waterTile(seed) : kind === 'transition' ? transitionTile(from, to, seed) : groundTile(ramp, seed));
    },
  },
  pattern: { describe: '1-bit pattern tile (16x16, tileable): checker, stripes, diagonal, dots, grid, bricks, diamonds, waves, rings, weave, hatch, seeded noise; ink and paper ramps', variants: () => PATTERN_IDS.map((kind) => ({ kind })), draw: drawPattern },
  building: { describe: 'Building exterior (32x32, front view; view top is the roof plan seen from above): barn, windmill, lighthouse, tower, chapel, inn, watchtower; the seed picks the roof colour; a `service` (registry, atelier, inn, training, archive, blacksmith, alchemist, store, trading, bank, portal, chronicle, fame, research, museum, marker) draws from the grammar of shell, theme and icon with a state (locked, available, new, upgrade): front 32x48 with a hanging sign, top 32x32 with the icon on the roof', variants: () => [...BUILDING_IDS.map((kind) => ({ kind })), ...BUILDING_IDS.map((kind) => ({ kind, view: 'top' })), ...SERVICE_IDS.flatMap((service) => [{ service }, { service, view: 'top' }]), ...BUILDING_STATES.filter((state) => state !== 'available').flatMap((state) => [{ service: 'bank', state }, { service: 'store', state, view: 'top' }])], draw: ({ service, shell, theme, glyph, state, ...rest } = {}) => (service !== undefined || shell !== undefined || theme !== undefined || glyph !== undefined || state !== undefined ? drawService({ service, shell, theme, glyph, state, ...rest }) : drawBuilding(rest)) },
  town: { describe: 'The town from above: a list of services laid on a lattice of streets with lamps and trees; returns hotspots (rect and entry), the walkable grid and alt text', variants: () => [{ buildings: [{ service: 'registry' }, { service: 'bank', state: 'locked' }, { service: 'inn', state: 'new' }, { service: 'museum', state: 'upgrade' }, { service: 'portal', cell: [2, 1] }] }], draw: drawTown },
  exhibit: { describe: 'A Museum slot: frames the picture the game gives it (any art, or `{ recipe, params }`) in a case, on a plinth, as a framed picture, a manuscript page or a specimen board (48x48), empty, occupied or new; a category placeholder when there is no picture; no text is drawn', variants: () => EXHIBIT_STYLES.flatMap((style) => [{ style }, { style, picture: { recipe: 'item', params: { shape: 'sword', tier: 'legendary' } } }, { style, state: 'occupied', category: 'weapon' }, { style, state: 'new', picture: { recipe: 'icon', params: { glyph: 'trophy' } } }]), draw: (p = {}) => drawExhibit({ ...p, picture: resolvePicture(p.picture) }) },
  award: { describe: 'Hall of Fame pieces: a blank plaque with an emblem slot, a podium (place 1 to 3, counted in pips), a ribbon, a wax seal, a medal (bronze, silver, gold, platinum; the rim notches count the tier) and rank chevrons (1 to 5); no text is drawn', variants: () => [...AWARD_KINDS.filter((k) => k !== 'podium' && k !== 'rank').flatMap((kind) => Object.keys(AWARD_TIERS).map((tier) => ({ kind, tier }))), { kind: 'podium', place: 1 }, { kind: 'podium', place: 3 }, ...[1, 3, 5].map((level) => ({ kind: 'rank', level })), { kind: 'medal', emblem: { recipe: 'icon', params: { glyph: 'star' } } }], draw: (p = {}) => drawAward({ ...p, emblem: resolvePicture(p.emblem) }) },
  vehicle: { describe: 'Vehicle (32x32, side view): wagon, sled, car, locomotive, airship, raft, cannon; the seed picks a car or airship body colour', variants: () => VEHICLE_IDS.map((kind) => ({ kind })), draw: drawVehicle },
  farm: { describe: 'Farm and food (16x16): wheat, carrot, pumpkin, corn, turnip, grapes, egg, mushroom, hen, sheep; `ink` names the body ramp', variants: () => FARM_IDS.map((kind) => ({ kind })), draw: (p = {}) => drawFarm(p) },
  strategy: { describe: 'Strategy pieces (16x16): hex tiles (grass, forest, mountain, water, desert, snow) and towers (archer, cannon, mage, frost); `ink` names the body ramp', variants: () => STRATEGY_IDS.map((kind) => ({ kind })), draw: (p = {}) => drawStrategy(p) },
  sigil: { describe: 'Clan sigil (16x16) composed from parts: shield field, ordinary (pale, fess, bend, chevron, cross, saltire) and charge (star, moon, crown, tower, sword, flame)', variants: () => [...SIGIL_ORDINARIES.map((ordinary) => ({ ordinary })), ...SIGIL_CHARGES.slice(1).map((charge) => ({ field: 'red', ordinary: 'fess', charge })), { field: 'green', ordinary: 'saltire', charge: 'none', color: 'gold' }], draw: drawSigil },
  hand: { describe: 'Hand (24x24): gesture, and how far the study has got (mitten, blocks, detailed)', variants: () => STAGES.flatMap((stage) => GESTURE_IDS.map((gesture) => ({ gesture, stage }))), draw: drawHand },
  cloth: { describe: 'Cloth or hair in a looping wind: flag, cape, banner, curtain, hair, robe (a skirt through a turn)', variants: () => CLOTH_KINDS.map((kind) => ({ kind })), draw: drawCloth },
  furniture: { describe: 'Interior furniture, 3/4 view, bottom-aligned to its footprint; `view: "top"` draws it from above on the 32 pixel grid (a top-down prop)', variants: () => [...FURNITURE_IDS.map((kind) => ({ kind })), ...TOP_FURNITURE_IDS.map((kind) => ({ kind, view: 'top' }))], draw: ({ kind = 'table', view, ...rest } = {}) => { if (view === 'top') { if (!TOP_FURNITURE_IDS.includes(kind)) throw new Error(`no top-down furniture "${kind}"; known: ${TOP_FURNITURE_IDS.join(', ')}`); return drawTopProp({ kind, ...rest }); } if (view !== undefined) throw new Error(`unknown furniture view "${view}"; known: top`); if (!FURNITURE[kind]) throw new Error(`unknown furniture "${kind}"`); return art(`furniture.${kind}`, FURNITURE[kind].make()); } },
  room: { describe: 'Furnished interior (12x8 tiles): floor, wall, windows, door, furniture, clues, light by time of day', variants: () => ROOM_IDS.map((room) => ({ room })), draw: drawRoom },
  overlay: { needsAtlas: true, describe: 'The layer drawn over an item or slot icon: a rarity frame (each rarity its own shape), state markers (new, equipped, locked, bound, crafted, enchanted, compat, socket, better, worse, same), a durability pip row and, at size 32 with the VGA atlas, a stack count', variants: () => [...RARITY_IDS.map((rarity) => ({ rarity })), ...MARK_IDS.map((m) => ({ rarity: 'rare', marks: [m] })), { rarity: 'epic', marks: ['new', 'equipped', 'locked', 'enchanted'], durability: 0.6 }, { rarity: 'legendary', size: 32 }, { marks: ['bound'], durability: 0.1, size: 32 }], draw: drawOverlay },
  dress: { describe: "A level dressed from the game's grid, seen from above (`#` wall, `.` floor, `+` door, `>` exit, `o` pit, `^` hazard): every cell drawn as the grid says, props from a template or the game's own list placed only where the walkable cells stay joined; returns the props with footprints, the blocked and unreachable cells", variants: () => [{ grid: DRESS_GRID }, ...TEMPLATE_IDS.map((template) => ({ grid: DRESS_GRID, template, theme: template === 'boss' ? 'volcanic' : 'crypt' })), { grid: DRESS_GRID, theme: 'ancient', doors: 'closed', props: [{ kind: 'chest', x: 2, y: 1, state: 'closed' }] }, ...DRESS_GATES.map((gates) => ({ grid: GATE_GRID, theme: 'mechanical', gates }))], draw: drawDress },
  map: { describe: 'The level map with fog of war from the cells the game supplies and what is known of each (unknown, visible, remembered, explored): outlines, the party, objective, boss, safe, trade, exit and teleport markers, depth pips, progress and a frame, mini (4 pixels a cell) or expanded (8); an unknown cell is drawn the same whatever it holds', variants: () => [{ cells: MAP_CELLS, party: [2, 2] }, { cells: MAP_CELLS, party: [2, 2], size: 'expanded', depth: 3, progress: 0.4, markers: [{ kind: 'boss', x: 4, y: 2 }, { kind: 'safe', x: 1, y: 1 }, { kind: 'objective', x: 3, y: 3 }, { kind: 'trade', x: 3, y: 1 }, { kind: 'exit', x: 4, y: 1 }, { kind: 'teleport', x: 1, y: 3 }] }], draw: drawMap },
  telegraph: { describe: 'The area a blow will cover, from above: a circle, line, cone or rectangle with a pattern fill as well as a colour (hatch for danger, dots for friendly, checker for neutral), two frames, the caster at the centre', variants: () => [...TELEGRAPH_SHAPES.map((shape) => ({ shape })), ...Object.keys(TELEGRAPH_KINDS).map((kind) => ({ shape: 'cone', kind, direction: 'south' })), { shape: 'line', direction: 'north', size: 64 }, { shape: 'rect', size: 32 }], draw: drawTelegraph },
  target: { describe: 'A marker for a 40 by 40 world sprite: an ally, enemy or neutral ring (different shapes, not just colours), the turn brackets, a party seat diamond or a shadow blob', variants: () => [...TARGET_KINDS.map((kind) => ({ kind })), { kind: 'seat', seat: 3 }], draw: drawTarget },
  skillicon: { describe: 'A class skill icon (24x24): a category (attack, defence, mobility, control, heal, cleanse, buff, debuff, summon, utility) sets the plate, an approved motif is the picture, a target (self, ally, enemy, party, area, line, cone) and kind (active, passive, triggered) fill the right column, tier and cost the bottom row; locked or on cooldown it is dithered', variants: () => [...SKILL_CATEGORY_IDS.map((category) => ({ category })), ...SKILL_TARGETS.map((target) => ({ category: 'attack', motif: 'flame', target, tier: 2, cost: 1 })), { category: 'heal', kind: 'passive', locked: true }, { category: 'control', cooldown: 0.5, kind: 'triggered', cost: 3, tier: 3 }], draw: drawSkillIcon },
  placeholder: { describe: 'Fallback picture: a character, an enemy family, an item category, a building, an exhibit, or an unknown thing by size class only (it never shows what it stands for)', variants: () => [{ kind: 'character' }, ...Object.keys(PLACEHOLDER_FAMILIES).map((family) => ({ kind: 'family', family })), ...Object.keys(PLACEHOLDER_CATEGORIES).map((category) => ({ kind: 'item', category })), { kind: 'building' }, { kind: 'exhibit' }, ...PLACEHOLDER_SIZES.map((size) => ({ kind: 'unknown', size }))], draw: drawPlaceholder },
  effect: { describe: 'Combat or magic effect (4 frames); `flash: false` and `density: \'low\'` are the no-flash and low-effects settings', variants: () => EFFECT_IDS.map((effect) => ({ effect })), draw: drawEffect },
  // Native ANSI recipes: `ansiOnly` means there is no pixel drawing (draw them with style 'ansi').
  banner: { describe: 'Block lettering (ANSI only): half, block, outlined or shaded face', ansiOnly: true, variants: () => BANNER_FACES.map((face) => ({ face })), ansi: drawBanner },
  landscape: { describe: 'Landscape with a banded sky, mountains, lake and ground (ANSI only): time and weather', ansiOnly: true, variants: () => [...TIMES.map((time) => ({ time })), ...WEATHERS.slice(1).map((weather) => ({ weather }))], ansi: drawLandscape },
  menu: { describe: 'Menu screen, 80x24 (ANSI only): double frame, shaded title, boxed list with a selection', ansiOnly: true, variants: () => [{}, { title: 'QUEST', items: ['Start', 'Continue', 'Quit'], selected: 1 }], ansi: drawMenu },
  bust: { describe: 'Head and shoulders in half blocks (ANSI only): skin, hair, light side', ansiOnly: true, variants: () => [...SKIN_IDS.map((skin) => ({ skin })), ...LIGHTS.slice(1).map((light) => ({ light }))], ansi: drawBust },
  weatherloop: { describe: 'Landscape with rain or snow falling, a seamless loop (ANSI only): clip `loop`, first frame is the still', ansiOnly: true, variants: () => LOOP_WEATHERS.map((weather) => ({ weather })), ansi: drawWeatherLoop },
  vista: { describe: 'A picture of a place from facts (ANSI only): one of 24 terrains, time, weather, fantasy or high style, 16 or 256 colours', ansiOnly: true, variants: atDepths(() => [...TERRAINS.map((terrain) => ({ terrain })), ...VISTA_TIMES.map((time) => ({ time })), ...WEATHER_KINDS.slice(1).map((weather) => ({ weather })), { terrain: 'woods', style: 'high', time: 'night' }, { terrain: 'stone', style: 'high' }]), ansi: drawVista },
  locale: { describe: 'A CoffeeMUD locale vista does not cover (ANSI only): road lengths, ice plain, frozen peaks, tree surface, sewer, hot and ice rooms, maze, ship decks, starfield', ansiOnly: true, variants: () => [...LOCALE_KINDS.map((kind) => ({ kind })), { kind: 'road', length: 'short' }, { kind: 'gundeck', guns: 'light' }, { kind: 'gundeck', guns: 'heavy' }], ansi: drawLocale },
  bar: { describe: 'Client status bar, 80x5 (ANSI only): name and level, hit points, mana and movement as meters', ansiOnly: true, variants: () => [{}, { hp: 12, mana: 0, move: 5 }], ansi: drawBar },
  mappane: { describe: 'Client map pane (ANSI only): rooms joined by passages on a seeded walk, you marked @', ansiOnly: true, variants: () => [{}, { seed: 'two', columns: 41, rows: 19 }], ansi: drawMappane },
  sheet: { describe: 'Character sheet, 80x24 (ANSI only): name, race and class, the six abilities as meters, the equipment slots', ansiOnly: true, variants: () => [{}, { name: 'Brenna', race: 'dwarf', cls: 'cleric', stats: [12, 12, 18, 9, 15, 11] }], ansi: drawSheet },
  creatureface: { describe: "A creature's face from its race (ANSI only): 24 head shapes, colours and cues read from a description, six moods, five hurt steps, effect looks; full 25 by 12 cells or a small 16 by 8 drawn on its own strokes", ansiOnly: true, variants: atDepths(() => [...FACE_SHAPES.map((shape) => ({ shape })), ...FACE_SHAPES.map((shape) => ({ shape, size: 'small' })), ...FACE_MOODS.slice(1).map((mood) => ({ race: 'Orc', mood })), ...HURT_STEPS.slice(1).map((_, i) => ({ race: 'Wolf', hurt: i + 1 })), ...FACE_LOOKS.map((look) => ({ race: 'Dwarf', looks: [look] })), { name: 'an old dwarven smith', description: 'A grizzled old dwarf with a long grey beard and a scar.' }]), ansi: drawCreatureFace },
  thing: { describe: "An item picture from its name (ANSI only): 56 kinds by the name's first word, material class texture (cloth, leather, wood, metal, mithril, gems, glass, rock), wear (worn to broken, rusty, crude, fine), auras; 25 by 12 cells with its slot in words", ansiOnly: true, variants: atDepths(() => [...THING_KINDS.map((kind) => ({ kind })), ...['wool', 'glass', 'leather', 'iron', 'mithril', 'paper', 'ruby', 'granite', 'oak'].map((material) => ({ name: 'a longsword', material })), ...['worn', 'dented', 'battered', 'broken', 'rusty', 'crude', 'fine'].map((wear) => ({ name: 'a sword', material: 'iron', wear })), ...THING_AURAS.map((aura) => ({ name: 'an oak staff', auras: [aura] })), { name: 'a rusty iron longsword' }, { name: 'a fine mithril sword' }, { name: 'a glowing wooden staff' }, { name: 'a quux' }]), ansi: drawThing },
  creation: { describe: "A character-creation picture (ANSI only): a race beside a measuring stick marked every foot, or a class with its tools; 186 races and 49 classes, a name the tables lack made from the name, 8 by 4 to 80 by 40 cells", ansiOnly: true, variants: atDepths(() => [...['human', 'elf', 'dwarf', 'halfling', 'half ogre', 'pixie', 'faerie', 'kobold', 'dragon born', 'treant', 'a quux'].map((name) => ({ kind: 'race', name })), ...['fighter', 'mage', 'thief', 'cleric', 'druid', 'bard', 'monk', 'artisan', 'a quux'].map((name) => ({ kind: 'class', name }))]), ansi: drawCreation },
  onplace: { describe: "A face or an item on its place (ANSI only): the figure's parts laid on a darker picture of a terrain, lit by it with a rim light by day and darkened at night; an animal stands on woods, plains or hills when its place is a room or a town", ansiOnly: true, variants: atDepths(() => [{ face: { race: 'Orc' }, terrain: 'woods', time: 'dusk' }, { face: { race: 'Orc' }, terrain: 'stone' }, { face: { race: 'Human' }, terrain: 'plains', time: 'day' }, { face: { race: 'Elf' }, terrain: 'woods', time: 'night' }, { face: { race: 'Wolf' }, terrain: 'stone', key: 'a wolf' }, { face: { race: 'Dwarf', size: 'small' }, terrain: 'mountains', time: 'dawn' }, { thing: { name: 'a fine mithril sword' }, terrain: 'hills', time: 'day' }, { face: { race: 'Orc' }, terrain: 'plains', time: 'day', invisible: true }]), ansi: drawOnPlace },
  fight: { describe: 'The Fight panel (ANSI only): a room with two faces, the player mirrored; a big blow jolts and flashes the one hit, a new foe slides in; on the phone a strip of eight rows with the small faces and the words between; reducedMotion takes out the jolt and the slide', ansiOnly: true, variants: atDepths(() => [{}, { terrain: 'woods', time: 'dusk' }, { terrain: 'stone', you: { race: 'Elf', mood: 'angry' }, foe: { race: 'Troll', hurt: 2 } }, { terrain: 'city', time: 'night', columns: 80 }, { terrain: 'plains', hit: 'foe', fell: 20 }, { terrain: 'plains', hit: 'you', fell: 20 }, { terrain: 'plains', hit: 'foe', fell: 20, reducedMotion: true }, { terrain: 'plains', hit: 'foe', fell: 20, reducedMotion: true, flash: false }, { terrain: 'woods', slide: true }, { terrain: 'woods', slide: true, reducedMotion: true }, { layout: 'phone', terrain: 'woods', words: ['You hit the orc.', 'It staggers.'] }, { layout: 'phone', hit: 'foe', fell: 30, slide: true }, { layout: 'phone', pictures: false, words: ['You hit the orc.', 'It staggers.', 'The orc hits you.', 'You are hurt.'] }]), ansi: drawFight },
  splash: { needsAtlas: true, describe: 'A splash (title) screen from a title and a scene: 256 by 144 pixels with the title in the VGA font (needs the atlas) on a dark plate over a quiet scene, or 80 by 24 cells of block lettering over a vista; scenes meadow, forest, desert, mountains, sea, snow, night', variants: () => [...SPLASH_SCENES.map((scene) => ({ title: 'Hollow Keep', scene })), { title: 'Q', scene: 'night' }], draw: drawSplash, ansi: drawSplashAnsi },
  artpack: { describe: 'A whole piece with SAUCE (ANSI only): title, framed landscape, credit', ansiOnly: true, variants: () => [{}, { title: 'NIGHT', time: 'night', weather: 'snow' }], ansi: drawArtpack },
};
export const RECIPE_IDS = Object.keys(RECIPES);

/** The drawing languages. `pixel` is every recipe above. `ansi` is a recipe's native drawing when it names one in its `ansi` field
 *  (preferred: it is made for cells), and otherwise the pixel drawing folded to cells (cells.js `foldArt`), so every recipe has both. */
export const STYLES = ['pixel', 'ansi', 'text'];
export const DEFAULT_STYLE = 'pixel';

/** The styles a recipe can draw in: all of them, since a pixel drawing folds. */
export function recipeStyles(id) {
  if (!RECIPES[id]) throw new Error(`unknown recipe "${id}"`);
  return RECIPES[id].ansiOnly ? ['ansi', 'text'] : [...STYLES];
}

/** Whether a recipe draws a style natively rather than by folding its pixel drawing. */
export const isNative = (id, style) => (style === DEFAULT_STYLE && !RECIPES[id]?.ansiOnly) || typeof RECIPES[id]?.[style] === 'function';

/** Draw by recipe id. Throws on an unknown recipe so a bad kit reference fails loudly, and on an unknown style.
 *  `options.style` defaults to `pixel`, so a call without it is exactly what it always was. An `ansi` drawing has Cells for frames;
 *  a folded one is made legal for `options.bgLimit` (8 background colours by default, the classic target; 16 for iCE and UTF-8), and a native one that takes a `bgLimit` param (`vista`) is handed it. */
function drawRaw(id, params = {}, options = {}) {
  const r = RECIPES[id]; if (!r) throw new Error(`unknown recipe "${id}"`);
  // `motion` (a clip of poses, see motion.js): a recipe with `motion: 'native'` (`character`) draws poses with its own joints; any other recipe, in any style, gets the generic move of its still.
  // A motion may be a preset of the recipe's motion type (a name or `{ preset, ...params }`, see motion-types.js); presets are made for the art's size.
  if (params.motion !== undefined && !r.motion) { const { motion, ...rest } = params, art = draw(id, rest, options), f = art.frames[0]; return withMotion(art, resolveMotion(id, motion, { w: f.w, h: f.h })); }
  if (params.motion !== undefined) params = { ...params, motion: resolveMotion(id, params.motion, { w: 32, h: 32 }) };
  const style = options.style ?? DEFAULT_STYLE;
  if (!STYLES.includes(style)) throw new Error(`unknown style "${style}"; known: ${STYLES.join(', ')}`);
  if (style === DEFAULT_STYLE) {
    if (r.ansiOnly) throw new Error(`recipe "${id}" draws in ansi only; pass style 'ansi'`);
    return r.draw(params, r.needsAtlas ? { atlas: options.atlas } : undefined);
  }
  if (style === 'text') {
    // Plain ASCII: the recipe's own `text` drawing, else its ANSI drawing (native or folded) read as ink, else its pixel drawing read as light and dark.
    if (typeof r.text === 'function') return { style, ...r.text(params) };
    return r.ansiOnly || typeof r.ansi === 'function' ? textFromAnsiArt(draw(id, params, { style: 'ansi', bgLimit: 16 })) : textFromPixelArt(r.draw(params));
  }
  return typeof r[style] === 'function' ? { style, ...r[style](options.bgLimit ? { ...params, bgLimit: options.bgLimit } : params) } : foldArt(r.draw(params), { bgLimit: options.bgLimit });
}
/** Draw by recipe id (see `drawRaw` above for the rules), then fill in what every drawing carries: `alt`, anchors, `reduced`, `projection` (art-meta.js). */
export function draw(id, params = {}, options = {}) { return decorate(id, params, drawRaw(id, params, options)); }
export { Pixels };
