#!/bin/sh
# Copies the sample projects that ship with the app into build/samples-stage/
# (as samples/), from the files git tracks and nothing else, so a release
# can only contain what is in the repository. The bundle used to take the
# whole samples/ folder, and with it whatever was on the builder's disk: the
# Benchmark sample's locally built binaries (bin/, git-ignored, compiled
# from other people's source and unsigned), .DS_Store files, downloaded
# .love archives, and the maintainer's reports. Reports (results/) are
# tracked, but they hold the machine's paths, and the app does not read them.
#
# scripts/build-release.sh runs it and points the bundle at the result.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT="$ROOT/build/samples-stage"
cd "$ROOT"

rm -rf "$OUT"
mkdir -p "$OUT"
# -z: names may hold spaces. A tracked file deleted from the disk makes
# rsync fail, which is right: a release is built from a tree that matches git.
git ls-files -z -- samples ':(exclude)samples/benchmark-libraries/results' | rsync -a --from0 --files-from=- ./ "$OUT/"
echo "Staged $(find "$OUT/samples" -type f | wc -l | tr -d ' ') sample files in $OUT/samples"
