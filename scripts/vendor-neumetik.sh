#!/usr/bin/env bash
# Copies the part of Neumetik that `nttplay` needs into vendor/neumetik/,
# relicensed MIT (decided 2026-11-13 by the maintainer, as for Painter,
# Composer and Animator: the same owner; Neumetik's own repo stays
# proprietary except midi/, which is already MIT). nttplay is Neumetik's
# command-line player (`nttplay probe FILE`, `nttplay decode FILE`), which
# JoyrIDE's file preview runs for MIDI, tracker modules and music written
# as text. Standard library only: no third-party crate comes in.
#
# Copied, per crate: src/, Cargo.toml, the data its source includes, and its
# third-party licence files. Not copied: tests, examples, docs, build output,
# bindings, the R36S C host. Change the code in Neumetik and copy, never
# only here (a vendored copy edited in place is overwritten).
#
#   scripts/vendor-neumetik.sh [path-to-neumetik]
set -euo pipefail
src="$(cd "${1:-../neumetik}" && pwd)"
root="$(cd "$(dirname "$0")/.." && pwd)"
dest="$root/vendor/neumetik"
[ -f "$src/r36s/lib/src/bin/nttplay.rs" ] || { echo "no nttplay in $src" >&2; exit 1; }
rm -rf "$dest"; mkdir -p "$dest"
copy() { # <source folder> <name here> <extra paths...>
  local from="$src/$1" to="$dest/$2"; shift 2
  mkdir -p "$to"
  cp -R "$from/src" "$to/src"
  cp "$from/Cargo.toml" "$to/Cargo.toml"
  [ -f "$from/build.rs" ] && cp "$from/build.rs" "$to/"
  for extra in "$@"; do mkdir -p "$to/$(dirname "$extra")"; cp -R "$from/$extra" "$to/$extra"; done
  cp "$root/LICENSE" "$to/LICENSE"
}
copy midi midi
copy bgn bgn
copy synth synth data
copy visual visual
copy tracker tracker docs/dropped-formats.md
copy r36s/lib nttplay
cp "$src/synth/data/organya/LICENSE" "$dest/synth/data/organya/LICENSE"
# The source tests are not needed to play a file.
find "$dest" -name target -prune -o -type d -name tests -print0 | xargs -0 rm -rf
python3 - "$dest" <<'PY'
import re, sys, pathlib
d = pathlib.Path(sys.argv[1])
for t in d.glob('*/Cargo.toml'):
    s = t.read_text()
    s = re.sub(r'(?m)^license-file\s*=.*$', 'license = "MIT"', s)
    s = re.sub(r'(?m)^# Proprietary:.*\n', '', s)
    s = s.replace('path = "../../midi"', 'path = "../midi"').replace('path = "../../bgn"', 'path = "../bgn"')
    s = s.replace('path = "../../tracker"', 'path = "../tracker"').replace('path = "../../visual"', 'path = "../visual"')
    # Examples, tests and benches are not copied, so their entries go.
    s = re.sub(r'(?ms)^\[\[(?:example|test|bench)\]\]\n.*?(?=^\[|\Z)', '', s)
    t.write_text(s)
PY
cat > "$dest/NOTICE" <<'N'
Neumetik players (midi, bgn, synth, visual, tracker, nttplay)
Copyright 2026 ansiapps

Licensed under the MIT License (see LICENSE in each folder). Vendored into
JoyrIDE from ansiapps' own Neumetik (scripts/vendor-neumetik.sh), which stays
proprietary apart from midi/, already MIT. Standard library only.

synth/data/organya: the Organya editor's waveforms and drum samples, BSD
licensed; its LICENSE is kept beside them.
N
echo "vendored $(find "$dest" -name '*.rs' | wc -l | tr -d ' ') Rust files, $(du -sh "$dest" | cut -f1)"
