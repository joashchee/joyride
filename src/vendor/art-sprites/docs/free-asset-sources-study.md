# Free asset sources: study note (2026-10-30)

Five links a search tool suggested for expanding the catalog, read on 2026-10-30. **Study material only.** Under rules 2, 4, 8 and 10
in `CLAUDE.md`, nothing from them is bundled, traced, converted or used as a fixture; Painter draws its own art from `rng(seed)` and
the DB32 palette. Their licenses were checked on 2026-10-30 (below); only Kenney and Quaternius are verified CC0.

## What each is, and the license check (2026-10-30)

Checked on each source's own page, not on the aggregators' summaries.

| Source | What it is | License, as checked | Verdict |
| --- | --- | --- | --- |
| assethoard.com | Catalogue of ~180 links to Kenney, Quaternius, OpenGameArt, 0x72, itch.io creators | Its terms license only the site's software and say "You are responsible for ensuring you have the rights and licences for the assets you import and use." The CC0 badges are its own, unchecked | **Dropped as a source.** Use the upstream creators instead |
| jam-ready-assets | 380 packs; no repo-wide license | Per pack `License.txt`: 347 CC0, 4 MIT (keep the notice), 29 RUN License ("RUN projects only" until it converts to MIT on 2028-01-01) | **Filtered.** CC0 packs pass; MIT needs the notice; the 29 RUN packs are excluded |
| Tiddybub/2d-assets | 1,101 packs, mostly OpenGameArt and Kenney | `LICENSE` and README declare the whole repo CC0, but give no way it was checked, and OpenGameArt hosts CC-BY and GPL art too. Real terms are in each pack's `SOURCE.md` | **Unverified.** Excluded until a pack's `SOURCE.md` and original page are checked one by one |
| gist UkoeHB/9991c1a6... | Link list (fork of the next) | None stated | **Pointers only** |
| gist benfrankel/5332a90d... | Link list, 2024-01-25 | None stated | **Pointers only** |
| Kenney (upstream) | 2D and 3D packs | CC0, "Attribution is not required" (kenney.nl/support); the Kenney logo is reserved | **Verified CC0** |
| Quaternius (upstream) | Low-poly 3D models | "All models are under the CC0 License" (quaternius.com/faq.html) | **Verified CC0**, 3D only |

Not checked: 0x72, KayKit, OpenGameArt and itch.io creators individually. Each would need its own page read first.

Even a verified CC0 pack is still not used: rule 10 keeps these as category lists and Painter draws its own art.

None has ANSI or CP437 art, and none describes a drawing technique.

## Where Painter already covers their categories

Their common themes (jam-ready's 19 and Tiddybub's folders) against Painter's recipes:

| Their theme | Painter |
| --- | --- |
| characters, NPCs, enemies | `character`, `creature`, `portrait`, `creation`, `bust` |
| fantasy, dungeon, RPG | `item`, `thing`, `room`, `scene`, `vista` |
| nature, terrain, tiles | `tile`, `landscape`, `vista`, `locale` |
| effects, particles | `effect`, `weatherloop` |
| UI, icons | `icon`, `bar`, `mappane`, `sheet`, `menu`, `sigil`, `banner` |
| props, furniture, interiors | `prop`, `furniture`, `room` |

## Gaps worth a recipe (candidates, none started)

Ranked by how often they show up and how little Painter has:

1. **Vehicles** (cars, ships, carts, planes; Tiddybub 14 packs, jam-ready "vehicles-racing"). **Drawn since:** `vehicle` (wagon, sled, car, locomotive, airship, raft, cannon) beside `prop`'s boat, cart, ship, caravan and spaceship. Still open: planes and non-side views.
2. **Buildings and city props** (Tiddybub "modern-urban" 18 packs, jam-ready "city"). **Drawn since:** `building` (barn, windmill, lighthouse, tower, chapel, inn, watchtower) beside `prop`'s house, clanhall, shrine and tent. Still open: roads, street furniture, modern blocks.
3. **Weapons as a family.** `item` and `thing` draw them one at a time; no kit row tracks which kinds exist.
4. **Farm and food** (jam-ready "farm", "food"; Tiddybub "nature"). `thing` already draws bread, fruit, meat, cheese and fish in ANSI; crops, fields and farm animals are open.
5. **Tower-defense and strategy pieces** (towers, hex tiles; jam-ready "tower-defense", "strategy-hex").
6. **1-bit patterns and tiles** (listed in both gists). **Drawn since:** `pattern`, 14 kinds, written from scratch.

Sci-fi is the largest folder in Tiddybub (529 of 1,101 packs). Painter has no sci-fi look; that is an art-direction choice, so rule 6
leaves it to a project to ask for.

## Not taken

- **Palettes (Lospec).** Rule 4 fixes DB32. Adding another palette is the user's decision, not a catalog task.
- **Pixel fonts.** Rule 8 fixes IBM VGA 8x16; any other font needs its license checked on the real project page first.
- **The art itself**, in any form (see the top of this note).

## Next steps if wanted

- Add the gaps above as `reference-only` subjects in a new kit (`kits/`), then `node cli.js kits --strict`.
- Draw one or two as real recipes, registered with `variants()` in `recipes/index.js`, then `npm test`.
