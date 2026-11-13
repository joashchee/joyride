# Asset Library — design notes

A catalog of every usable asset (images, sprites, tilesets, fonts, audio,
music, shaders, Lua libraries, data) across the user's projects and folders,
with its **licence**, category, tags and a preview, so an asset can be put
into a project in one step. It also makes licence-free assets
deterministically, imports from other projects only after the licence is
confirmed, and de-duplicates on the way in. Roadmap: section 5.

Researched 2026-11-13 in the sister apps; what each gives, and how.

## What to take from each sister app

| Need | Source | What it gives | How to use it |
|---|---|---|---|
| Catalog store, search | **Diskette** `src-tauri/src/db.rs`, `search.rs`, `store.rs` | SQLite catalog, `entry_metadata`, contentless FTS5 (`catalog_search`) with `year:`/`duration>` style queries, per-volume rescan by (path, size, mtime) | Same schema shape: `assets`, `asset_metadata`, `asset_tags`, FTS5 search. Incremental rescan of watched folders. Copy the code (same owner), don't depend on it |
| Previews | **Diskette** `thumbnails.rs`, `ansiart.rs`, `extrainfo.rs` | Pure-Rust image thumbnails (≤ N px JPEG in the catalog, served by a custom protocol), audio tags, EXIF | Thumbnails for png/jpg/gif; audio shows waveform + `<audio>`; fonts render a sample; spritesheets show frames. HEIC/RAW out of scope |
| Tagging, Smart Folders | **Diskette** `src/lib/schema.ts` (drafted, not built), `diskette-framework.md` build order 5 | A drafted `SmartFolder` (saved search) type | Build here first if Diskette hasn't; share the format |
| Tag and licence vocabulary | **Neumetik** `music-catalog-system.md` §3 record schema, §16 Rights, §17 tag syntax | Faceted tags with slugs, controlled vocab + free folksonomy, rights fields (`license_type`, `licensor`/`source`, `license_ref`, `credits_required`, `modification_allowed`, `ai_involvement`, `expiry_alert`) | Use as the asset record: trim to what a game dev needs (licence SPDX id, source URL, author, attribution text, commercial use, modification, redistribution, AI-generated flag) |
| Duplicate detection | **Crunchy** `scan.rs`, `docs/check-strategies.md`, `clones.rs`; **Diskette** `hashes.rs` | Pipeline proven on real data: size → 4 KiB prefix → first+last 64 KiB → full **BLAKE3**, parallel; APFS clone ids; hashes keyed by path and valid only while size+mtime match; Crunchy only finds, never deletes (`trash.rs`, `undo.rs` for acts) | Reuse the pipeline for import. Identity = BLAKE3 of content; near-duplicates (resized/re-encoded) are *suggested*, never merged silently |
| Provenance record per ported thing | **Stylus** `stylus-core/src/convert/` `ConverterInfo` (origin, licence, commit) and `licenses/` | Pattern: every borrowed thing carries origin + licence + revision, and its licence text ships | Each asset's record keeps source, licence and a copy of the licence text/URL; the library can export a `CREDITS.txt` |
| Deterministic generators | **Painter** `art-sprites` (pixel/ANSI art, DB32 palette, PNG codec), **Animator** `sprite-animator` (clips), **Composer** `music-composer` (music, MIDI), **Neumetik chipbank** (758 chip-sound recipes as text) | Same recipe + params + seed → same output, dependency-free, run in a page or worker; seeded PRNG with `fork(label)` streams (`rng.js`); lint of craft rules | Generators are the "licence-free" source: output is ours, so the record is auto-filled (`CC0`/MIT, source = recipe+params+seed) and regenerable from that triple. Needs the licensing decision below |
| Reproducibility rule | **Stylus** CLAUDE.md rule 5 | Own PRNG (never `rand`'s default), integer math, generator name/version/seed stored with output | Same rule here: recipe id, version, seed and params stored in the asset record |
| Safe handling | **Crunchy** handoff guard, **Diskette** `handoff.rs` | Versioned file contracts, read-only until the user acts | Library files in the project are copied, never moved; originals untouched |

## What was built, and how it differs

`asset-core/` (2026-11-13) is clean-room: written from the ideas above, not
from Diskette's or Crunchy's source. Deliberately different: identity is
content only (no per-volume paths); the hash cache lives in the same
catalog file; licences are first-class and gate import; import is a pure
plan followed by an apply that needs a named confirmer; the staged finder
is a small function over `(path, size)` entries rather than a scan pipeline.
Left out on purpose (not needed for game assets): resource forks and Finder
metadata, APFS clone ids, cloud placeholders, volumes, read-only media,
byte-for-byte compare before deletion (nothing here deletes). Revisit
cloud placeholders and package bundles if libraries are ever scanned from
arbitrary folders.

## Licensing decision (made 2026-11-13: Painter, Composer, Animator vendored under MIT)

JoyrIDE is MIT and public. **Painter, Composer, Animator and Neumetik
chipbank are proprietary** (all rights reserved); Diskette and Crunchy have
no licence file. None can be copied into this repo as they are. Precedent:
Coupler vendors Painter as `src/vendor/art-sprites` **under Apache-2.0 with
its own LICENSE and NOTICE**, and Neumetik MIDI likewise. Same owner, so
each engine can be vendored here under MIT the same way, but that
relicenses a snapshot; do it deliberately, per engine, and record it in
`NOTICE`. Until decided, the roadmap builds the library, catalog and
import first and the generators last. Chipbank's sound text is the most
valuable and the most proprietary: ask before including it.

## Record (sketch)

`assets`: id, content BLAKE3 (unique), size, mime, kind, name, source path,
created. `asset_licence`: SPDX id or `custom`, licence text/URL, author,
source URL, attribution required + text, commercial use, modification,
redistribution, AI involvement, `confirmed` (who/when/how), expiry.
`asset_tags` (facet:value, free tags). `asset_origin`: where it came from
(project, URL, generator recipe + params + seed + version). `asset_use`: which
projects use it and at what path. Preview in `asset_previews`.

## Rules

1. **No asset is usable until its licence is confirmed.** Unknown is a state,
   shown plainly, and blocks "Add to project" and export (with an override
   that records the user's reason).
2. **Import confirms licences first.** Scan a project, show each asset with
   the licence guessed from `LICENSE*`, `README`, `CREDITS`, sidecar files
   and filename hints, the user confirms or edits, then import. Nothing is
   copied before that.
3. **De-duplicate on import** by content hash: an identical asset becomes a
   new *use* of the existing record, not a second copy; if the licences
   differ, ask. Near-duplicates are suggestions.
4. **Generated assets are regenerable**: store recipe + seed + params;
   output is deterministic.
5. Attribution: the Export Center collects required credits into
   `CREDITS.txt` for each build.
6. No server, no telemetry: the library is a local database under the
   app-data dir; a project carries only `.joyride/assets.toml` (what it
   uses, with licences) so it's shareable in Git.
