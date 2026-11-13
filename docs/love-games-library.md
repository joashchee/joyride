# The LÖVE games and apps library

A second library beside the libraries manager (`library.rs`): not code to put
in a project, but **what has been made with LÖVE**: games and apps, where to
get each, under what licence, which version is the latest. For finding things
to play, learn from, port to a handheld, or study.

Code: `joyride-core/src/games.rs` (the data model and its rules, no network),
`data/games.toml` (the catalogue, generated), `data/game-seeds.toml` (where to
look), `examples/curate_games.rs` (the fetcher).

## What an entry holds

`id` (slug), `name`, `kind` (game or app), `summary`, `authors`, `tags`,
`licence` (SPDX id, `proprietary` for free-to-play with rights reserved, or
`unknown`; where it was read from; confirmed by a person or not),
`love_version` (what it targets, when known), `latest` (version and date),
`source`s (the pages it is known from: GitHub, GitLab, Codeberg, itch.io, the
LÖVE forums, PortMaster), `download`s (URL, platform, version, date, size, and
a BLAKE3 once someone has fetched it), `archived`, `checked` (last refresh).

## Rules

1. **Links, not copies.** The catalogue stores where to download, never the
   file. Nothing is downloaded or run to build it; opening a download is the
   person's act (CLAUDE.md rule 1: no network use they did not ask for).
2. **Licences are recorded as found and never guessed up.** GitHub's detection
   gives an SPDX id (marked unconfirmed); `NOASSERTION` and no licence are
   `unknown`. `unknown` is shown as unknown and blocks "open source" filters,
   as in the Asset Library (`asset-library.md`, rule 2). A known licence
   replaces an unknown one and a confirmed one replaces a guess.
3. **De-duplication.** An entry added is the same game as one already there
   if they share a source page (URLs reduced: scheme, `www.`, query,
   `.git` and `/releases/...` dropped), a download file (same URL, or same
   BLAKE3 under two URLs: both URLs are kept as mirrors), or the same name
   with an author in common. The same name by *different* authors is
   **not** merged: it is kept apart and listed as a look-alike for a person
   to decide.
4. **Keep the latest.** When two entries merge, the newer `latest` wins
   (dotted versions compare by number, `v1.10` after `v1.9`; otherwise by
   date); an older release arriving later never rolls it back. Downloads are
   listed newest first. Sources, authors and tags are the union.
5. **Refresh, never drop.** Re-running the fetcher updates entries; a game
   that disappears from a source stays, with its last known facts.

## Sources

| Source | State |
|---|---|
| GitHub repositories (seeds, then topic `love2d`) | `from_github` and `curate_games` written and tested on GitHub's JSON shape; **not yet run against GitHub** (the sandbox's `gh` cannot verify GitHub's certificate) |
| PortMaster-New's LÖVE ports (68 as of `40186ea1`) | `curate_portmaster` written and run (2026-11-14): 68 entries, each with its zip, store links and runtime; licences unknown (a port does not state the game's licence) |
| itch.io (the `made-with-love` tag; it has an API key flow, so the person supplies theirs) | not yet |
| LÖVE forums showcase, awesome-love2d | not yet |

"All LÖVE games" is not knowable: this tracks what the sources list, and says
which source each entry came from.
