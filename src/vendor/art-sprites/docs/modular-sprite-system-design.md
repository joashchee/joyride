# Modular Sprite System --- Design & Implementation Specification

**Project:** Tag Team\
**Audience:** Claude Code / implementation agent\
**Status:** Proposed implementation specification\
**Primary objective:** Build a small, engine-independent, data-driven
sprite composition system that starts with Tag Team's retro fantasy
dungeon and can later support other game styles without rewriting its
core.

------------------------------------------------------------------------

## 1. Executive summary

Build a lightweight sprite pipeline and runtime compositor for 2D games.
It must assemble characters and objects from pre-drawn, aligned layers;
support animation frames and visible equipment changes; keep asset
metadata and rendering rules in an expandable manifest; and allow new
assets to be ingested only when their provenance and usage rights are
recorded and acceptable.

The first profile is **Tag Team Fantasy Dungeon**: readable retro pixel
art for a portrait-first spectator/player UI, with five party archetypes
(Guardian, Berserker, Rogue, Scholar, Healer), monsters, weapons, armor,
loot, effects, and basic combat/exploration animation.

The system must not require AI, an LLM, a particular game engine, or a
network connection at runtime. It must not affect deterministic game
rules, authoritative outcomes, persistence, or the trusted runner.
Rendering is a presentation concern driven by already-committed
state/events.

**Key design choice:** define a small engine-independent data model and
adapter interface, not a full game engine. Keep the first implementation
deliberately narrow, but make manifests extensible.

## 2. Goals

1.  Compose a character from interchangeable, pre-drawn sprite layers.
2.  Make equipment changes visually apparent: helmet, chest armor,
    boots, weapon, shield, cape, accessories, dyes and cosmetic effects.
3.  Support animation clips made from frame sequences, including
    layer-aware animation where needed.
4.  Use a typed, versioned manifest as the source of truth for assets,
    styles, animation clips, layer order, compatibility, attribution and
    license status.
5.  Add assets through a repeatable offline ingestion/validation
    workflow.
6.  Support multiple art styles and game profiles without hard-coding
    fantasy-specific assumptions into the core.
7.  Render through a small engine adapter, initially for the
    repository's existing browser UI/renderer.
8.  Provide fallbacks for missing or incompatible assets and maintain
    accessible, reduced-motion presentation.
9.  Keep output reproducible: the same manifest, asset bytes,
    composition definition and frame must produce the same visual
    result.
10. Keep licensing information available to developers and operators and
    generate attribution notices where required.

## 3. Non-goals

-   No AI-generated art or LLM dependency.
-   No replacement for the game engine, authoritative runner, combat
    system, persistence, or event schema.
-   No full-featured pixel-art editor.
-   No mandatory online asset marketplace or runtime asset downloads.
-   No promise that an asset is lawful merely because a scraper or file
    extension accepts it.
-   No requirement to support every art style in the first release.
-   No procedural pixel drawing beyond simple deterministic compositing,
    palette substitution, tinting, and effects.
-   No bundling or redistribution of raw source packs unless their terms
    explicitly permit it.

## 4. Architecture and boundaries

Use four separable parts:

1.  **Core compositor** --- loads normalized asset metadata, resolves
    layers, selects animation frames, and produces a render plan or
    composite image. No game rules, DOM assumptions, network access, or
    persistence.
2.  **Manifest and style profiles** --- describe asset IDs, dimensions,
    frame geometry, layer names, anchors, palette groups, animation
    clips, compatibility, provenance and licensing.
3.  **Ingestion/validation tools** --- import local files, record
    source/creator/license evidence, validate image properties,
    optionally slice sprite sheets, generate thumbnails/contact sheets,
    and report errors. These tools run during development/build, not in
    the game loop.
4.  **Renderer adapter** --- translates render plans into the existing
    frontend renderer (e.g. canvas or WebGL) and receives presentation
    state. Keep the core independent of the adapter.

Suggested conceptual flow:

`committed game state/event -> presentation model -> composition request -> manifest resolver -> render plan/compositor -> renderer adapter`

A presentation event may trigger an animation, but it must never mutate
authoritative state. Do not refactor the deterministic runner to
accommodate visual effects.

### Suggested module boundaries

Adapt names to the existing repository rather than creating duplicate
infrastructure:

-   `sprite-core/`: composition resolver, frame selection, validation
    types, palette mapping
-   `sprite-manifest/`: schemas, style profiles, manifest loader
-   `sprite-tools/`: import, inspect, validate, contact-sheet and
    license-report commands
-   `sprite-adapters/`: current app renderer adapter
-   `assets/`: licensed asset files, grouped by pack/style
-   `docs/assets/`: asset register, ingestion instructions, generated
    attribution/license reports

Do not create all folders blindly. Inspect the existing repository and
follow its language, build tooling, naming conventions and tests first.

## 5. Core data model

Use stable string IDs rather than filenames as public identifiers. Use a
typed schema appropriate to the existing stack (TypeScript types plus
JSON Schema, if TypeScript is already used). Include a `schemaVersion`
and provide validation errors with asset IDs and JSON paths.

### 5.1 Asset record

Each asset should describe:

-   `id`: globally unique stable ID
-   `kind`: `layer`, `standalone-sprite`, `tileset`, `effect`, `icon`,
    or other explicit supported kind
-   `styleId`: art-style profile
-   `path`: local project-relative path; never an arbitrary remote URL
    at runtime
-   `source`: source URL, creator, pack name, download date, original
    filename
-   `license`: exact license identifier/name, license URL, attribution
    text, evidence/notes, status
-   `dimensions`: source width/height and expected pixel dimensions
-   `frame`: frame width/height, frame count, columns/rows or explicit
    frame rectangles where applicable
-   `anchors`: stable alignment points such as feet, hand, head, weapon
    grip, center
-   `layer`: semantic layer name and draw order
-   `paletteGroup`: optional palette mapping family
-   `tags`: searchable descriptive tags
-   `compatibility`: body types, classes, equipment slots, orientations
    and styles supported
-   `variants`: optional color, rarity, direction or cosmetic variants
-   `integrity`: optional SHA-256 checksum
-   `status`: `approved`, `review-needed`, `rejected`, or `deprecated`
-   `notes`: limitations and adaptation notes

Avoid duplicating derived values when they can be validated from the
file. Do not treat `license.status = approved` as sufficient unless
required source/license fields and review evidence are present.

### 5.2 Style profile

A style profile defines assumptions that assets must share or explicitly
convert between:

-   `id`, `name`, `version`
-   coordinate/pixel scale and nominal character canvas
-   projection: `top-down`, `three-quarter`, `side-view`, `isometric`,
    etc.
-   pixel-art palette rules and optional palette families
-   default frame size and animation naming conventions
-   supported directions and animation states
-   layer stack and default anchor locations
-   nearest-neighbor scaling policy
-   optional allowed resampling/normalization rules
-   default missing-asset fallbacks
-   accessibility/reduced-motion behavior

Do not combine incompatible perspectives or radically different pixel
scales silently. Validation should flag style mismatches; an explicit
conversion/normalization step may be allowed with recorded provenance.

### 5.3 Character composition

A composition references a style profile, body/base asset, palette and
optional layers by slot:

-   `body`
-   `head` / `hair`
-   `torso`
-   `legs`
-   `feet`
-   `headgear`
-   `chestArmor`
-   `cape`
-   `weaponMain`
-   `weaponOffhand`
-   `accessory`
-   `cosmeticEffect`

Use semantic slots, but do not assume every game style needs the same
slots. Profiles can declare additional slots and layer ordering. An
asset may provide multiple layers or a full-body replacement if
explicitly marked.

A character's gameplay equipment is authoritative elsewhere. The
compositor receives a read-only visual description resolved from the
character/equipment state. Cosmetic appearance and combat stats must
remain separate.

### 5.4 Animation clip

An animation clip should support:

-   `id`, `styleId`, `entityType`
-   frame rate or per-frame duration
-   loop policy
-   direction(s)
-   frame sequence or frame-index mapping
-   layer-specific offsets/visibility overrides if necessary
-   event markers for presentation synchronization (e.g. weapon impact
    frame)
-   fallback clip
-   reduced-motion alternative

Do not assume every layer has an identical frame layout. The manifest
must state how frame rectangles and anchors map to the same logical
animation frame. If a pack uses incompatible frame layouts, normalize it
during ingestion or use an explicit adapter mapping.

### 5.5 License/provenance record

At minimum:

-   `creator`
-   `sourceUrl`
-   `assetPageUrl`
-   `licenseName`
-   `licenseUrl`
-   `attributionRequired`
-   `attributionText`
-   `commercialUseAllowed`
-   `modificationAllowed`
-   `redistributionAllowed`
-   `shareAlike`
-   `nonCommercial`
-   `noDerivatives`
-   `evidenceNotes`
-   `reviewedAt`
-   `reviewedBy` (person/role, not a private account identifier)
-   `status`

Represent unknown rights as **unknown**, never as allowed. These fields
are a practical checklist, not a substitute for the actual license text.
For custom licenses, preserve the exact terms and a human-readable
review note. A source website's general policy does not override a
specific asset's license.

## 6. Composition and rendering behavior

### 6.1 Deterministic resolution

Given the same manifest version, asset files, composition, animation ID
and frame index, resolution must produce the same layer order and render
result.

-   Use explicit draw order and anchors.
-   Use nearest-neighbor scaling for pixel art.
-   Do not use random variation unless a seed is explicit and supplied
    by the caller.
-   Do not fetch remote assets while rendering.
-   Avoid time-dependent asset selection.
-   Cache composed frames using a stable key that includes
    manifest/version, asset IDs, palette, equipment, animation and
    frame.
-   Invalidate cache entries when any relevant input changes.

### 6.2 Layer order and occlusion

The style profile defines base order. Some assets need conditional
foreground/background placement (e.g. a weapon behind the body during
one frame and in front during another). Support a minimal per-frame
override rather than building a complicated scene graph. A layer can
declare visibility and ordering rules, but these rules must be explicit
and testable.

### 6.3 Anchors and alignment

Every modular layer must align to a common logical canvas or declare an
anchor/offset transformation. At ingestion time, validate expected
dimensions and anchors. Provide a contact sheet preview showing all
frames/layers over a checkerboard or base body. Flag likely
misalignment; do not silently stretch or distort pixels.

### 6.4 Palette changes

Allow palette substitutions only where the asset declares a compatible
palette group or indexed-color mapping. Never recolor arbitrary RGBA
artwork blindly. Preserve outline contrast and transparency. Palettes
must be versioned and deterministic.

### 6.5 Equipment changes

The UI sends the updated visual loadout after authoritative equipment
state changes. The compositor resolves changed slots and refreshes only
affected layers/cache keys. A weapon or armor change should be visible
immediately in the viewport and character panel, with an optional short
transition. A visual failure must not affect the actual equipment state.

### 6.6 Rendering targets

The core should return a render plan or pixel buffer independent of the
game engine. Prefer the least complex approach that fits the current
repository:

-   If the frontend already has a canvas renderer, provide a canvas
    adapter.
-   If it uses WebGL, provide an adapter that uses atlas textures and
    nearest-neighbor sampling.
-   If it uses DOM images initially, use precomposed local PNGs where
    reasonable.
-   Do not introduce a new engine or a heavyweight dependency merely to
    render sprites.

Provide a static composite export mode for debugging and environments
that do not support live layering.

## 7. Asset ingestion pipeline

All ingestion happens in development/build tooling, not on user devices.

### Step 1 --- Source selection

-   Accept a local downloaded pack or a documented source URL.
-   Identify the exact asset page and creator, not only the hosting
    website.
-   Prefer CC0/public-domain assets, then clearly compatible CC BY
    assets with attribution.
-   Custom royalty-free licenses may be accepted only after their terms
    are reviewed for proprietary game use, modification, in-game
    distribution, and redistribution restrictions.
-   Do not accept an asset based only on a "free download" label.

### Step 2 --- License gate

-   Store the license text/URL or a durable evidence note where legally
    appropriate.
-   Mark the asset `approved` only after review.
-   Reject or quarantine assets with unclear provenance, contradictory
    terms, or incompatible restrictions.
-   Avoid CC BY-NC for the project; do not use CC BY-ND for assets that
    must be modified; do not use CC BY-SA or GPL art unless an explicit
    review confirms the obligations are acceptable for the intended
    distribution.
-   Do not redistribute the raw asset pack unless its license explicitly
    permits that.
-   Maintain attribution in a generated `ASSET-LICENSES.md` and
    machine-readable `asset-licenses.json`.

### Step 3 --- Technical inspection

-   Detect supported image formats.
-   Record dimensions, alpha channel, color mode, and file size.
-   For sprite sheets, infer candidate frame grids only as suggestions;
    require explicit confirmation or strong validation before
    registering them.
-   Validate frame dimensions, row/column bounds, transparency, and
    integer pixel scaling.
-   Do not modify the source file in place.

### Step 4 --- Normalize

-   Copy the original into a source/archive location only if license
    terms permit it.
-   Produce normalized working copies for the chosen style profile.
-   Preserve original files and record transformation steps.
-   Use nearest-neighbor scaling only when scale conversion is approved.
-   Record crops, offsets, palette transformations, and generated
    atlases.
-   Never strip embedded credits or license metadata intentionally.

### Step 5 --- Register and preview

-   Create or update manifest records.
-   Generate thumbnails and contact sheets.
-   Preview layered equipment on the base body in idle and at least one
    action animation.
-   Flag gaps such as missing directions, mismatched frames,
    inconsistent outlines or clipping.

### Step 6 --- Validate and report

-   Run schema and file validation.
-   Run license completeness checks.
-   Run style compatibility checks.
-   Generate a human-readable report listing accepted, rejected, and
    review-needed assets.
-   Fail CI/build for broken paths, duplicate IDs, invalid schema, or
    unapproved assets referenced by production manifests.
-   Do not fail merely because optional content is absent; use a clear
    fallback.

### Suggested tool commands

Use the repository's existing command style; names below are
illustrative:

-   `sprites inspect <path>`
-   `sprites ingest <path> --profile tag-team-fantasy`
-   `sprites validate`
-   `sprites preview <composition-id>`
-   `sprites contact-sheet <asset-or-pack-id>`
-   `sprites licenses`
-   `sprites build-atlas --profile tag-team-fantasy`

Commands must report what was actually checked and avoid claiming legal
approval automatically.

## 8. Initial Tag Team Fantasy Dungeon profile

Start small and cohesive. Use assets already identified in the project's
asset research, verifying the license on the exact asset page before
importing.

Potential source candidates: - Kenney Roguelike/RPG Pack --- CC0,
subject to checking the current asset page. - Kenney RPG Base --- CC0,
subject to checking the current asset page. - Kenney RPG Audio --- CC0,
for a future audio mapping phase; audio is not part of the sprite
compositor itself. - OpenGameArt Tiny Dungeon --- verify the exact
asset's license. - OpenGameArt Buch Dungeon Tileset --- verify the exact
asset's license. - OpenGameArt top-down dungeon tileset and RPG
character animations --- verify the exact asset's license. - Other
itch.io packs only when the individual creator's license and permitted
use are clear.

Known discovery pages are not blanket license grants. Review each chosen
asset and record its own source and license.

### First profile requirements

-   Retro fantasy dungeon pixel art.
-   Consistent top-down or chosen top-down/three-quarter perspective; do
    not mix perspectives in the first profile.
-   Base bodies and modular equipment for the five Tag Team archetypes.
-   One or two monsters to prove generic non-player entities work.
-   At minimum: idle, move, attack, hit, and defeat/death or vanish
    where assets permit.
-   Item/loot sprites and basic combat/status effects.
-   A fallback silhouette or placeholder for missing art.
-   Character and equipment preview on the portrait-first square
    viewport.
-   All assets local; no runtime remote downloads.

If a candidate pack is not modular, use it for standalone sprites/tiles
while sourcing a modular character pack separately. Do not force every
asset into the paper-doll model.

## 9. Future game-style expansion

The core must not hard-code fantasy roles, equipment names, projection
or a fixed layer list.

Future profiles might include: - sci-fi crew and armor; - modern or
historical characters; - side-view platformer characters; - isometric
units; - creatures with body-part variants; - vehicles and modular
machines; - UI icons, portraits and non-character sprites.

A new profile should mostly be data: canvas dimensions, projection,
anchors, palette groups, animation conventions, layer/slot declarations
and fallback behavior. Core code changes should be needed only when a
genuinely new rendering capability is required.

Never combine assets from different profiles by default. Cross-style
conversion must be explicit and tested.

## 10. Accessibility and mobile performance

-   Preserve crisp pixel edges using nearest-neighbor scaling.
-   Avoid flashing effects and rapid high-contrast patterns.
-   Respect reduced-motion preference: replace long movement/zoom/shake
    sequences with brief, restrained or static feedback.
-   Important information must also be available in text/chronicle and
    not be communicated by animation or color alone.
-   Keep animation optional; do not block game interaction while an
    animation plays.
-   Bound texture/image cache size and release unused composites.
-   Avoid recomposing every character every frame; cache by stable
    composition key.
-   Lazy-load only local approved assets relevant to the current
    scene/profile.
-   Ensure a missing sprite does not break layout or the simulation.
-   Test on a low-memory mobile browser and the intended Mac mini M1/8
    GB operational setup, without requiring that the server render every
    client frame.

## 11. Security and supply-chain requirements

-   Treat imported files, metadata and archive paths as untrusted.
-   Prevent path traversal when unpacking archives; reject absolute
    paths and `..` paths.
-   Apply reasonable file size, image dimension and archive expansion
    limits.
-   Do not execute scripts included in asset packs.
-   Do not hotlink or dynamically execute content from source URLs.
-   Avoid remote runtime loading; production manifests reference local
    packaged assets only.
-   Keep the license register and provenance notes under version
    control.
-   Use checksums to detect unexpected changes.
-   Keep asset ingestion separate from the authoritative runner and
    server process.
-   Do not add third-party dependencies without documenting size,
    license, maintenance and reason.

## 12. Testing and acceptance criteria

### Unit tests

-   Manifest schema validation and helpful error paths.
-   Duplicate asset IDs and missing local paths are rejected.
-   Unsupported or unknown license state cannot be approved
    automatically.
-   Layer ordering is stable and explicit.
-   Anchor/offset resolution is deterministic.
-   Frame rectangles remain within source image bounds.
-   Palette substitution works only for compatible palette groups.
-   Missing optional layers produce a fallback rather than a crash.
-   Cache keys change when equipment, palette, frame, asset
    bytes/version or manifest changes.
-   Same inputs produce the same render plan/output.

### Integration tests

-   Compose a base character with and without headgear.
-   Change chest armor and weapon and confirm the resulting composition
    differs as expected.
-   Play idle and attack clips with multiple layers.
-   Preview a monster and a standalone loot sprite.
-   Load a second mock style profile with different canvas dimensions
    without changing core logic.
-   Confirm an invalid asset is excluded from production manifests.
-   Generate attribution and license reports from manifest data.
-   Confirm presentation effects cannot mutate game state or
    authoritative events.

### Acceptance criteria for Phase 1

1.  One working style profile: `tag-team-fantasy`.
2.  At least one fully composed character using multiple layers.
3.  At least three visibly distinct equipment changes.
4.  At least idle and attack animations, plus a static fallback.
5.  One standalone monster and one loot/item sprite.
6.  Manifest-driven loading; no asset-specific hard-coded rendering
    branches.
7.  A documented local ingestion and validation workflow.
8.  License/provenance fields for every included third-party asset.
9.  Generated `ASSET-LICENSES.md` and `asset-licenses.json`.
10. Tests pass and the current game simulation remains unchanged.

Do not make the acceptance criteria depend on having five complete,
polished characters if the chosen licensed pack cannot supply them.
Report art gaps honestly and use placeholders until suitable assets are
approved.

## 13. Phased implementation plan

### Phase 0 --- Repository reconnaissance

-   Read the current Game Bible, UI design, and game asset roadmap.
-   Identify language, frontend renderer, build/test commands, current
    asset conventions, event adapter, and deployment constraints.
-   Propose a concise file plan and report risks before editing.
-   Do not refactor unrelated systems.

### Phase 1 --- Minimal manifest and validator

-   Define schema/types and one `tag-team-fantasy` profile.
-   Implement path, ID, dimensions, frame-bounds, and
    license-completeness checks.
-   Add sample records and tests. Use placeholders if assets are not yet
    approved.

### Phase 2 --- Static composition

-   Implement deterministic layer resolution, anchors, draw order and
    nearest-neighbor output.
-   Add a debug preview/contact sheet.
-   Prove base body + helmet + armor + weapon.

### Phase 3 --- Animation

-   Add frame selection, clip metadata, direction mapping and event
    markers.
-   Support idle and attack first.
-   Add caching and reduced-motion fallback.

### Phase 4 --- Ingestion workflow

-   Add local pack inspection, safe extraction, normalization metadata,
    manifest generation and license report.
-   Import only a small set of verified assets.
-   Ensure unknown/unclear license records are quarantined.

### Phase 5 --- Tag Team integration

-   Add read-only adapter from the existing presentation model to
    composition requests.
-   Reflect committed equipment changes in the viewport and character
    panel.
-   Add combat feedback without changing game rules or event semantics.
-   Keep a static fallback for missing assets.

### Phase 6 --- Expansion

-   Add further archetypes, enemies, equipment, effects and optional
    style profiles.
-   Only add core features when new asset requirements justify them.
-   Maintain coverage and licensing reports as assets grow.

Claude Code should implement one phase at a time, run relevant tests
after each phase, and report changed files, commands/tests actually run,
verified results, known gaps, and assumptions. Do not attempt to ingest
a large asset catalog or build every animation in one task.

## 14. Example manifest sketch

This is illustrative rather than a final schema; refine it after
repository inspection.

``` json
{
  "schemaVersion": 1,
  "styles": [
    {
      "id": "tag-team-fantasy",
      "name": "Tag Team Fantasy Dungeon",
      "projection": "top-down",
      "nominalCanvas": { "width": 32, "height": 32 },
      "scaleMode": "nearest-neighbor",
      "layerOrder": [
        "shadow",
        "body",
        "legs",
        "torso",
        "cape-back",
        "headgear",
        "weapon-back",
        "weapon-front",
        "effects"
      ],
      "fallbackAssetId": "placeholder-adventurer"
    }
  ],
  "assets": [
    {
      "id": "placeholder-adventurer",
      "kind": "standalone-sprite",
      "styleId": "tag-team-fantasy",
      "path": "assets/placeholders/adventurer.png",
      "source": {
        "creator": "Project placeholder",
        "sourceUrl": "",
        "packName": "internal"
      },
      "license": {
        "licenseName": "Project-created",
        "licenseUrl": "",
        "status": "approved",
        "attributionRequired": false,
        "evidenceNotes": "Created for this project; verify repository ownership policy."
      },
      "dimensions": { "width": 32, "height": 32 },
      "layer": "body",
      "status": "approved"
    }
  ]
}
```

Do not copy the example verbatim if it conflicts with existing
repository conventions. Validate the final schema and ensure the
placeholder is actually created or change the path.

## 15. Deliverables

-   Modular sprite core and renderer adapter.
-   Versioned schema/types and initial fantasy style profile.
-   Asset manifest with stable IDs.
-   Ingestion and validation CLI/script.
-   Preview/contact-sheet tool or equivalent developer view.
-   Tests for validation, composition, animation and fallback behavior.
-   `ASSET-LICENSES.md` plus machine-readable `asset-licenses.json`.
-   `docs/sprite-system.md` explaining architecture, manifest fields,
    adding a profile, adding assets, and troubleshooting.
-   `docs/asset-ingestion.md` explaining safe import, license review and
    approval.
-   Short implementation report listing what is complete, what is
    missing, test results, and licensing decisions that still require
    human review.

## 16. Ready-to-use instruction for Claude Code

> Inspect the existing Tag Team repository and read the current Game
> Bible, UI Design specification, iPhone UX/Animation Roadmap, and Game
> Asset Roadmap before changing code. Implement this modular sprite
> system in small phases, beginning with repository reconnaissance and a
> proposed file plan. Keep it engine-independent and data-driven; start
> with the `tag-team-fantasy` profile. Use pre-drawn assets only, no
> AI/LLM dependency. Implement stable manifest IDs, layer composition,
> anchors, frame-based animations, equipment visual changes, an asset
> ingestion/validation workflow, and a license/provenance register.
> Import only assets whose exact license and source are recorded and
> reviewed; quarantine unclear licenses. Keep all runtime assets local,
> use deterministic rendering and nearest-neighbor scaling, and preserve
> the authoritative simulation, trusted runner, persistence and event
> semantics. Respect portrait-first UI, reduced motion, accessibility,
> and low-memory browser constraints. Reuse existing repository
> conventions and avoid unnecessary dependencies. Complete Phase 0 first
> and report findings before proceeding; then implement one phase at a
> time. For every phase, run relevant tests and report files changed,
> commands actually run, verified outcomes, known gaps, and unresolved
> licensing questions. Do not claim tests or license checks passed
> unless they were actually performed.
