#!/bin/sh
# Builds Neumetik's `nttplay` (vendor/neumetik/nttplay, MIT, standard library
# only) into src-tauri/resources/toolchains/nttplay/, where the app bundle
# ships it and the file preview finds it (`preview::find_nttplay`): MIDI,
# tracker modules and music written as text play through it.
#
# Run it AFTER scripts/fetch-toolchains.py, which clears the whole
# toolchains folder first. scripts/build-release.sh runs it for you.
#
# macOS: both architectures, joined by lipo into one universal binary, signed
# ad hoc (an arm64 binary must carry a signature to run, and lipo's output is
# a new file). The Developer ID signature, hardened runtime and secure
# timestamp that notarization needs are put on by scripts/sign-macos.sh,
# with every other program in the app. Elsewhere: this machine's build, as it
# is.
#
# Decoder only (`--no-default-features`): the BGN scenes and Visual painter
# are for the R36S C host, which nttplay does not use.
#
# RUSTFLAGS is passed through, so a release build's --remap-path-prefix
# keeps the builder's home folder out of the binary.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
CRATE="$ROOT/vendor/neumetik/nttplay"
OUT="$ROOT/src-tauri/resources/toolchains/nttplay"

[ -f "$CRATE/Cargo.toml" ] || { echo "No vendored nttplay at $CRATE (scripts/vendor-neumetik.sh)." >&2; exit 1; }

build() { # <target triple or empty>
  if [ -n "$1" ]; then
    cargo build --release --offline --no-default-features --manifest-path "$CRATE/Cargo.toml" --target "$1" >&2
  else
    cargo build --release --offline --no-default-features --manifest-path "$CRATE/Cargo.toml" >&2
  fi
}

rm -rf "$OUT"
mkdir -p "$OUT"

case "$(uname -s)" in
  Darwin)
    build aarch64-apple-darwin
    build x86_64-apple-darwin
    lipo -create \
      "$CRATE/target/aarch64-apple-darwin/release/nttplay" \
      "$CRATE/target/x86_64-apple-darwin/release/nttplay" \
      -output "$OUT/nttplay"
    codesign --force --sign - "$OUT/nttplay"
    # Checked, as the release script checks the app: both architectures.
    ARCHS=$(lipo -archs "$OUT/nttplay")
    case "$ARCHS" in
      *arm64*x86_64* | *x86_64*arm64*) ;;
      *) echo "nttplay is not universal: $ARCHS" >&2; exit 1 ;;
    esac
    echo "nttplay: universal ($ARCHS)."
    ;;
  *)
    build ""
    EXE=nttplay
    [ -f "$CRATE/target/release/nttplay.exe" ] && EXE=nttplay.exe
    cp "$CRATE/target/release/$EXE" "$OUT/$EXE"
    echo "nttplay: built for this machine only."
    ;;
esac

cp "$CRATE/LICENSE" "$OUT/LICENSE"
cat > "$OUT/CREDITS.txt" <<'EOF'
nttplay: ansiapps' Neumetik command-line player (MIDI, tracker modules, music
written as text), MIT, standard library only. Source: vendor/neumetik/ in the
JoyrIDE repository (scripts/vendor-neumetik.sh).
EOF

# It must run (this machine's slice, at least).
"$OUT/nttplay" --version >/dev/null 2>&1 || { echo "The built nttplay does not run." >&2; exit 1; }
echo "nttplay is in $OUT"
