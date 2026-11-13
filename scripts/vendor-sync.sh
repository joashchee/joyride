#!/usr/bin/env bash
# Copies ansiapps' three drawing, animation and music engines into
# src/vendor/, relicensed MIT (decided 2026-11-13; the engines' own repos
# stay proprietary, the same owner, as Coupler vendors Painter under
# Apache-2.0). Each copy is the runtime code (*.js, *.d.ts, recipes/, kits/),
# its README, CHANGELOG and tests, and the third-party licences that go
# with its data; not the R36S ports, reference kits or tools. The package.json
# says MIT and the LICENSE is this repo's; NOTICE keeps the engine's
# third-party credits. The originals are the engine repos: change them there
# and copy, never only here (a vendored copy edited in place is overwritten).
#
#   scripts/vendor-sync.sh [parent-folder-of-painter-composer-animator]
set -euo pipefail
parent="$(cd "${1:-../}" && pwd)"
root="$(cd "$(dirname "$0")/.." && pwd)"
copy() { # <repo> <vendor name> <title>
  local src="$parent/$1" dest="$root/src/vendor/$2"
  [ -f "$src/package.json" ] || { echo "no $src/package.json" >&2; exit 1; }
  rm -rf "$dest"; mkdir -p "$dest"
  (cd "$src" && cp *.js *.d.ts README.md CHANGELOG.md package.json "$dest/" 2>/dev/null || true)
  for d in recipes kits test LICENSES; do [ -d "$src/$d" ] && cp -R "$src/$d" "$dest/$d"; done
  [ "$1" = painter ] && [ -d "$src/docs/fonts" ] && mkdir -p "$dest/docs" && cp -R "$src/docs/fonts" "$dest/docs/fonts"
  if [ "$1" = painter ]; then # the data and doc its tests read
    mkdir -p "$dest/docs/coupler-handover/painter-data"
    cp "$src/docs/coupler-handover/painter-data/"{figure,painter-tables}.json "$dest/docs/coupler-handover/painter-data/"
    cp "$src/docs/coupler-painter-parity.md" "$src"/docs/*.md "$dest/docs/"
  fi
  cp "$root/LICENSE" "$dest/LICENSE"
  python3 - "$src" "$dest" "$3" <<'PY'
import json, re, sys, pathlib
src, dest, title = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), sys.argv[3]
p = dest / 'package.json'; j = json.loads(p.read_text())
j['license'] = 'MIT'; p.write_text(json.dumps(j, indent=2) + '\n')
n = src / 'NOTICE'; text = n.read_text() if n.exists() else ''
head = f"{title}\nCopyright 2026 ansiapps\n\nLicensed under the MIT License (see LICENSE). Vendored into JoyrIDE\nfrom ansiapps' own engine (scripts/vendor-sync.sh).\n"
body = re.sub(r'\(proprietary, as Animator is\)', '(MIT here, as Animator is)', text.split('\n', 3)[-1]) if text else ''
(dest / 'NOTICE').write_text(head + '\n' + body.strip() + '\n')
rd = dest / 'README.md'
if rd.exists():
    r = re.sub(r'(?m)^(?:Proprietary|License: proprietary).*$', 'MIT, Copyright 2026 ansiapps (see `LICENSE`); see `NOTICE` for what it includes from others.', rd.read_text())
    r = r.replace('Private repo: not published to npm.', 'Vendored into JoyrIDE under MIT.')
    rd.write_text(r)
PY
  echo "$2: $(grep -m1 '"version"' "$dest/package.json" | tr -d ' ,')"
}
copy painter art-sprites "art-sprites (the Painter engine)"
copy composer music-composer "music-composer (the Composer engine)"
copy animator sprite-animator "sprite-animator (the Animator engine)"
