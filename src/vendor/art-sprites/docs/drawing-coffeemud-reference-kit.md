# CoffeeMUD Drawing Reference Kit

A CC0-first visual reference and drawing-curriculum kit for depicting everything seen in **CoffeeMUD**: a high-fantasy text MUD and extensible game engine.

CoffeeMUD’s engine taxonomy includes races, classes, MOBs, items, armor, weapons, magic and technology items, clan items, areas, locales, exits, abilities, behaviors, and UI/web surfaces. Use this kit to build a modular drawing engine that can identify and fill visual gaps.

> **License note:** CoffeeMUD itself is Apache-2.0, not CC0. Use its source and wiki as a structural/content reference, but source finished visual assets from explicitly CC0 collections. Classic drawing books listed below are generally public domain rather than formally CC0.

---

## 1. Source basis

- **CoffeeMud official site:** https://www.coffeemud.org/
- **CoffeeMud wiki:** http://coffeemud.net:8080/wiki/index.php/Main_Page
- **JavaDocs / package taxonomy:** https://www.coffeemud.org/docs/overview-summary.html
- **Source repository:** https://github.com/bozimmerman/CoffeeMud

Key engine content packages:

- `Races`
- `CharClasses`
- `MOBS`
- `Items.Armor`
- `Items.Weapons`
- `Items.Basic`
- `Items.BasicTech`
- `Items.CompTech`
- `Items.MiscMagic`
- `Items.ClanItems`
- `Areas`
- `Locales`
- `Exits`
- `Abilities.*`
- `Behaviors`
- `Web`

---

## 2. Visual asset taxonomy

| CoffeeMUD category | Things to depict | CC0 asset sources |
|---|---|---|
| Races & characters | Humans, elves, dwarves, halflings, goblins, orcs, giants, undead, beasts, insectoids, aquatic and exotic races | Kenney character packs; OpenGameArt CC0 humanoid/monster art; Quaternius modular characters |
| Player classes | Fighter, mage, cleric, druid, thief, bard, paladin, ranger, archon, commoner, tech/superpowered archetypes | Kenney fantasy character sprites; OpenGameArt CC0 class sprites; Quaternius fantasy characters |
| MOBs / monsters | Animals, vermin, dragons, demons, undead, humanoids, constructs, elementals, sea creatures, planar beings | Kenney monster packs; OpenGameArt CC0 creature art; Poly Pizza CC0 creature models |
| Armor & clothing | Cloth, leather, chain, plate, shields, helms, robes, boots, gloves, jewelry, clan regalia | Kenney medieval RPG packs; OpenGameArt CC0 armor icons/sprites; Quaternius props |
| Weapons | Swords, daggers, axes, maces, spears, bows, crossbows, staves, wands, thrown and improvised weapons | Kenney weapon packs; OpenGameArt CC0 weapon icons; Quaternius fantasy weapons |
| Magic items | Potions, scrolls, wands, rings, amulets, spellbooks, keys, enchanted tools, cursed objects | Kenney fantasy/RPG icon packs; OpenGameArt CC0 potion/scroll/arcane art |
| Technology items | Computers, terminals, robots, cybernetics, vehicles, scanners, energy weapons, gadgets | Kenney sci-fi kits; OpenGameArt CC0 sci-fi props; Poly Pizza CC0 robot/tech models |
| Everyday items | Food, drink, tools, containers, books, maps, coins, lamps, rope, furniture, trade goods | Kenney generic/RPG packs; OpenGameArt CC0 inventory icons |
| Clan items | Banners, sigils, badges, trophies, clan halls, ceremonial objects | Kenney medieval/castle kits; OpenGameArt CC0 heraldry/banner art |
| Areas & locales | Cities, villages, dungeons, caves, forests, mountains, deserts, swamps, oceans, planes, underworlds, space | Kenney nature/city/dungeon/sci-fi kits; OpenGameArt CC0 tilesets; Poly Pizza CC0 environments |
| Rooms & architecture | Taverns, shops, temples, castles, prisons, homes, workshops, libraries, ships, ruins | Kenney medieval/castle/interior packs; OpenGameArt CC0 interior/building tilesets |
| Exits & transitions | Doors, gates, portals, stairs, ladders, tunnels, roads, bridges, ship decks, magical gateways | Kenney door/gate assets; OpenGameArt CC0 portal/door/dungeon art |
| Spells & abilities | Fire, ice, lightning, healing, necromancy, illusions, summons, blessings, curses, poisons, traps | Kenney particle/FX packs; OpenGameArt CC0 spell/VFX/icon art |
| Combat & status effects | Slashing, bludgeoning, bleeding, burning, freezing, poison, disease, buffs, debuffs | Kenney UI/FX packs; OpenGameArt CC0 combat VFX/status icons |
| Traps & hazards | Pit traps, dart traps, alarms, gas, spikes, magical wards, mechanical devices | Kenney mechanism assets; OpenGameArt CC0 dungeon hazard art |
| UI & world interface | Health/mana bars, inventory, maps, compass, chat, skills, quests, currencies, crafting menus | Kenney UI packs; OpenGameArt CC0 RPG UI kits |
| Web/client visuals | Browser client, maps, character sheets, chat panes, icons, logos, web pages | Kenney UI/icon packs; OpenGameArt CC0 interface art; CoffeeMud `web/` as structural reference only |

---

## 3. Recommended CC0 asset libraries

| Source | Best for | Notes |
|---|---|---|
| Kenney.nl | Sprites, tilesets, 3D models, UI, props | Large, consistently CC0 catalog; excellent baseline library |
| OpenGameArt.org | Specific fantasy props, monsters, spells, armor, weapons, tilesets | Filter per asset; verify each item is CC0/CC-PDDC |
| Poly Pizza | Low-poly 3D characters, props, buildings, vehicles | Useful for 3D dioramas; check each model’s license |
| Quaternius | Stylized low-poly characters, nature, buildings, weapons, props | Good for cohesive 3D mockups |
| AssetHoard | Curated CC0 game-asset discovery | Use as a discovery layer, then verify original license |

---

## 4. Modular drawing curriculum

Each module represents a capability the drawing engine can invoke when an asset category is missing or weak.

| Module | Capability | Covers | Core studies | Learning references |
|---|---|---|---|---|
| M1 | Construction & form | Every asset | Primitives, proportion, overlap, value, silhouette | *Drawing Basics* OER textbook — CC BY-SA 4.0, not CC0 |
| M2 | Perspective & space | Areas, rooms, dungeons, cities, interiors, roads, bridges, ships, space | 1/2/3-point perspective, horizon, vanishing points, grids, curves | *Perspective Made Easy* — Ernest Norling, public domain |
| M3 | Humanoid anatomy | Races, classes, player characters, humanoid MOBs, armor fitting | Gesture, skeleton, muscles, pose, weight, hands | *Figure Drawing for All It’s Worth* — Andrew Loomis; *Constructive Anatomy* — George Bridgman; public domain |
| M4 | Heads, faces & expression | Portraits, emotions, races, undead, masks, helmets | Skull construction, facial planes, expression, stylization | *Drawing the Head and Hands* — Andrew Loomis, public domain |
| M5 | Creature anatomy | Animals, monsters, dragons, beasts, insects, sea creatures, aberrations | Comparative anatomy, quadrupeds, wings, horns, tails | Public-domain artistic-anatomy references; OpenGameArt CC0 creature references |
| M6 | Costume & cloth | Robes, armor, cloaks, banners, flags, sails, regalia | Fabric behavior, folds, tension, drape, heraldry | *Costume Design and Illustration* — Ethel B. Traphagen; public domain |
| M7 | Props & manufactured objects | Weapons, tools, potions, books, keys, containers, furniture, tech | Hard-surface construction, symmetry, materials, icon readability | *Sketching and Rendering in Pencil* — Arthur L. Guptill, public domain |
| M8 | Architecture & interiors | Taverns, temples, castles, shops, prisons, workshops, ruins | Plans, elevations, ornament, scale, interior lighting | *The Essentials of Perspective* — Leslie W. Miller; *The Theory and Practice of Perspective* — G. A. Storey; public domain |
| M9 | Nature & terrain | Forests, mountains, caves, deserts, swamps, oceans, weather, plants | Organic shapes, silhouettes, rocks, trees, atmospheric depth | Public-domain landscape texts; OpenGameArt CC0 environment tilesets |
| M10 | Magic, FX & motion | Spells, elemental effects, auras, portals, combat effects | Motion lines, energy shapes, particles, frame sequences | *Animated Cartoons* — Edwin G. Lutz, public domain |
| M11 | Icon & UI design | Inventory, skills, spells, currencies, maps, bars, buttons | Shape language, clarity, grids, color coding, symbols | *The Art of Composition* — Michel Jacobs; OpenGameArt CC0 UI packs |
| M12 | Color, light & rendering | Final rendering for all assets | Value structure, local color, light, materials, palettes | *The Practice and Science of Drawing* — Harold Speed, public domain |

---

## 5. Public-domain core library

Use these as the engine’s foundational “study modules”:

- **Construction:** *Fun with a Pencil* and *Successful Drawing* — Andrew Loomis
- **Figure and anatomy:** *Figure Drawing for All It’s Worth* — Andrew Loomis; *Constructive Anatomy* — George Bridgman
- **Heads:** *Drawing the Head and Hands* — Andrew Loomis
- **Perspective:** *Perspective Made Easy* — Ernest Norling; *The Essentials of Perspective* — Leslie William Miller
- **Rendering and craft:** *The Practice and Science of Drawing* — Harold Speed; *Sketching and Rendering in Pencil* — Arthur Leighton Guptill; *Line: An Art Study* — Edmund J. Sullivan
- **Motion and FX:** *Animated Cartoons* — Edwin George Lutz
- **Composition:** *The Art of Composition* — Michel Jacobs

---

## 6. Drawing-engine gap-fill pipeline

For any missing CoffeeMUD visual asset:

1. **Classify the asset** — character, creature, item, environment, exit, effect, UI, or web surface.
2. **Define silhouette** — establish readability before detail.
3. **Construct from primitives** — boxes for architecture/tech; cylinders for limbs/weapons; spheres for heads/orbs.
4. **Apply anatomy module** — M3 for humanoids, M5 for creatures, M7 for objects.
5. **Place in perspective** — M2 for environments; M8 for built spaces.
6. **Assign material logic** — cloth, metal, stone, wood, flesh, energy, and water require distinct edge and value treatment.
7. **Render light and color** — use M12 for final readability and mood.
8. **Validate at target scale** — icons, map markers, and UI elements must remain identifiable when small.
9. **Cross-check against CC0 asset packs** — use Kenney, OpenGameArt CC0, Poly Pizza, and Quaternius references for proportion, style, and completeness.

---

## 7. Completeness checklist

- [ ] One silhouette per major race
- [ ] One costume/equipment sheet per major class
- [ ] Bestiary categories: animal, humanoid, undead, construct, elemental, dragon, aberration, tech enemy
- [ ] Weapon families: blade, blunt, polearm, ranged, thrown, magical, improvised
- [ ] Armor tiers: cloth, leather, chain, plate, shield, helm, accessory
- [ ] Magic items: potion, scroll, wand, ring, amulet, book, key, cursed object
- [ ] Tech items: terminal, robot, vehicle, scanner, energy weapon, gadget
- [ ] Environment biomes: forest, mountain, desert, swamp, ocean, cave, dungeon, city, plane, space
- [ ] Built spaces: tavern, shop, temple, castle, prison, home, workshop, library, ship, ruin
- [ ] Exits: door, gate, portal, stair, ladder, tunnel, road, bridge, deck
- [ ] Spell families: fire, ice, lightning, healing, necromancy, illusion, summoning, blessing, curse, poison, trap
- [ ] Status effects: buff, debuff, bleed, burn, freeze, poison, disease, stun
- [ ] UI: inventory, skills, spells, currency, map, compass, health, mana, chat, quest, crafting
- [ ] Clan visuals: banner, sigil, badge, trophy, hall, ceremonial object

---

## 8. License rules

- **CC0 / CC-PDDC:** No attribution required; safest for finished game assets.
- **Public domain:** Usually no permission needed, but verify the specific edition and jurisdiction.
- **CC BY / CC BY-SA:** Requires attribution; CC BY-SA also requires share-alike for adaptations.
- **CoffeeMud source/wiki:** Apache-2.0; use for structure, naming, and content taxonomy, not as a CC0 art library.
- **Never assume:** Check the license on every individual asset, especially on OpenGameArt and Poly Pizza.

---

## 9. Findings from the linked sources (read 2026-10-29)

What the JavaDocs package pages show, and which of it Painter draws. The kit's data is `kits/coffeemud.js`; `node cli.js kits` is the live report.

**Reachability.** The official site, JavaDocs and GitHub pages read fine. The wiki (`coffeemud.net:8080`) fails TLS from here, and the site's Features, Reference and Builder's guides were not found at guessed paths, so the wiki and guides are still unread. The repo and site hold no art to reuse: only an ASCII banner and logo images, which stay uncopied.

**Real counts, by package:** Races 249 (about half animals), MOBS about 150 (ten dragon colours, giants, drow, shopkeepers), Items.Armor 51, Items.Weapons 62, Items.MiscMagic 47, Items.Basic about 140 (including boats, tents, carts and space bodies), Locales 115 (grid, thin-grid and maze variants of a few terrains), Exits 31, CharClasses 56.

**Draw families, not classes.** Most Locales are a terrain plus a layout (`Grid`, `ThinGrid`, `Maze`), which is a map concern, not a new picture; the picture families are fewer than 40.

**Gaps: all closed in 1.7.0.** The kit has no `partial` and no `gap` row (`node cli.js kits` is the live report). What closed each:

1. ~~Creature plans~~ horse, insect, ghost, elemental, fey, treant, griffon, naga, a four-armed biped, centaur and merfolk, with presets for the undead, elementals, mephit, fey, mounts and the ten dragon colours. `RACES` (25 races) names a plan, a size and the anatomy that sets a race apart; `draw('creature', { race })`.
2. ~~Class loadouts~~ shaman, archon, pirate, sailor, jester, six mage schools, and the shop trades (shopkeeper, banker, librarian, smith).
3. ~~Item shapes~~ javelin, sling, net, lasso, scimitar, katana, arquebus, cannon, dress, pants, shirt, earring, pill, powder, bag, cards, portable hole, lute, flute, drum, horn, backpack, mirror, pipe, cigar, perfume, scale, food, apron, ledger.
4. ~~Vehicles~~ boat, ship, cart, caravan, tent (props) and a saddled `mount`.
5. ~~Locales~~ the `locale` recipe (ANSI): road lengths, ice plain, frozen peaks, tree surface, sewer, hot room, ice room, maze corridor, ship deck, hold, quarters, light, medium and heavy gun decks, magazine, starfield. `vista` keeps Coupler's 24 terrains.
6. ~~Exits~~ gate, grate, curtain, stairs, ladder, pit, locked, trapped and hidden doors, gap, skyway, airlock.
7. ~~Effects and statuses~~ ten per-school spells plus necromancy, illusion, blessing, disease; statuses plus sleep, blind, silence, slow. Trap pictures: needle, dart, gas, spikes, alarm, ward.
8. ~~Built spaces~~ tavern, shop, castle hall, ruin, sewer, stable, inn (rooms); biomes mountain, snow, jungle, road, plains, canopy.
9. ~~Space and tech~~ terminal, robot, scanner, spaceship, star, planet, gas giant, moonlet, asteroid, starfield.
10. ~~UI and clan~~ `bar`, `mappane` and `sheet` (ANSI); clan hall prop and the `sigil` composer (field, ordinary, charge).

What stays out of scope: every Locale class (a layout, not a picture), per-class armour tiers beyond the four item tiers, and the original CoffeeMUD banner art (copyright, never copied).
