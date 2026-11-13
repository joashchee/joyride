# Painter to Tag Team: decisions and what to expect

**Written 2026-10-30**, in answer to `Tag Team Visuals 2.3` (sections 8.4,
Appendix A, 1.2). The full gap list and build order are in
`docs/tag-team-gaps-plan.md`. Carry this note into the Tag Team project.

## 1. Characters stay 32 by 32

Your document calls 32 by 48 and 48 by 64 "prototype starting points to
validate on real phones, not fixed requirements" (8.4). Painter keeps
**32 by 32** (40 wide while a weapon swings) for these reasons:

- **Everything is built on it.** 26 loadouts, 4 views, 16 poses, 3 back
  slots, the layer slots, the clips and the `motion` presets all share one
  body. A taller canvas changes every existing character and every golden
  hash, for a size nobody has yet tested on a phone.
- **Five seats must fit a portrait phone.** Five characters across a square
  viewport at a whole-number scale (8.2 forbids fractional scaling) leave
  more room at 32 wide than at 48. A smaller sprite scaled by 3 or 4 stays
  crisp and readable; a larger one forces a lower scale or a smaller party.
- **Read the shape, not the detail.** Your principle 6 says silhouettes and
  contrast matter more than decoration. 32 by 32 forces strong shapes,
  which suits class identity without colour (2.1B).
- **Detail comes from other places.** Portraits are a separate, larger
  canvas (2.1D, 8.4), and items and icons can go to 24 or 32 (plan T1; those sizes arrive as the shapes are redrawn with more detail, not scaled up).
- **It can still grow.** If your phone test says 32 is too small, Painter
  adds a taller `bodyProfile` beside the default. The default does not
  change, so saved characters stay put.

**What to do on your side:** set `fantasy_pixel_v1` to a 32 by 32 character
canvas and a 16-pixel tile unit for now, leave the canvas a manifest field
(Appendix A says it is configurable anyway), and run the phone test with
the real sprites.

## 2. The world is true top-down

Your document asks for one projection and no mixing (8, first profile).
Painter will draw the world looking straight down: characters in four
facings (`north` `east` `south` `west`), creatures, props, walls as flat
tops, buildings as roof plans. Every art returned carries
`projection: 'top-down'` (plan T23, T4).

- The old front, side, back and quarter views stay for **portraits, the
  Looks Atelier mannequin and Museum displays**, which you already put on
  separate canvases. Set the profile's world projection to `top_down` and
  those to `portrait`.
- **Readability risk:** from above, a class shows by headgear, weapon,
  shield and back slot, not body shape. Painter adds a lint that the five
  classes differ by silhouette alone before the top-down characters ship.
  Judge them on a phone before building much on them.

## 3. What Painter will not draw

Panels, buttons, health bars, seat frames, the turn strip and the viewport
frame are CSS in your game, with the IBM VGA 8x16 font. Painter supplies the
pixel digits, rarity and state frames, skill and panel icons, and badges.

## 4. Take Painter as a pinned dependency

Until launch, a local dependency (`npm install ../painter`) lets you follow
changes. **At launch, pin an exact released version** (no `^` or `~`),
commit the lockfile, and upgrade on purpose:

1. Read the CHANGELOG lines that say "changes existing output".
2. Store each saved character's `resolveLook` spec (plan T4), not just a
   loadout name, so a new default cannot alter it.
3. Bake Museum and historical exhibit images into your asset bundle
   instead of redrawing them (invariant 7).
4. Rerun your golden hashes, which `sprites pack` will emit, before moving
   the pin.

Painter's rule is same recipe, params and seed gives the same pixels *within
one version*. The pin and the stored spec are what carry that across
versions.

## 5. Order and releases

1.21 contract (anchors, dye, pack export, placeholders, alt text; built, awaiting a checkpoint),
1.22 top-down party, 1.23 dungeon and enemies, 1.24 town and community,
1.25 breadth. Each is suggested, not scheduled; Painter tells you in its
CHANGELOG when one lands.
