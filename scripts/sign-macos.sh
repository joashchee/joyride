#!/bin/sh
# Finishes JoyrIDE.app for notarization, after `tauri build` and before
# Apple sees it (scripts/build-release.sh runs it). Modelled on Floppy's
# scripts/sign-macos.sh, which also bundles programs it did not build.
#
#   scripts/sign-macos.sh [JoyrIDE.app]          sign the app and what is in it
#   scripts/sign-macos.sh --dmg [JoyrIDE.app]    rebuild and sign Tauri's DMG
#                                                from the finished app, if
#                                                this build made one
#
# APPLE_SIGNING_IDENTITY  "Developer ID Application: ... (TEAMID)" (required);
#                         "-" signs ad hoc, for trying the script without an
#                         Apple account (no hardened runtime then: its library
#                         validation wants every file in one Team ID, and ad
#                         hoc signatures have none).
#
# What it does that Tauri does not:
#
# 1. Puts back the symlinks. Tauri copies resources as plain files, so every
#    framework in the bundled love.app lost its Versions/Current, Resources
#    and top-level links (measured: none left in the built app), which
#    leaves duplicated files and a bundle codesign cannot seal.
# 2. Signs every executable and library in the app's resources with the
#    Developer ID, the hardened runtime and a secure timestamp, innermost
#    first. Tauri signs the app but not its resources, and notarization
#    refuses the whole app for one unsigned or untimestamped Mach-O. The
#    bundled LÖVE gets src-tauri/macos/love.entitlements (LuaJIT needs
#    executable memory; measured). StyLua, lua-language-server and nttplay
#    need none (measured: they run under the hardened runtime without).
# 3. Signs the app again, last, since changing what is inside breaks its seal.
# 4. Checks every Mach-O in the app one by one, and fails on any that is not
#    signed as it should be (a real identity: Developer ID, hardened runtime,
#    secure timestamp).
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
DMG_MODE=
if [ "${1:-}" = "--dmg" ]; then DMG_MODE=1; shift; fi
APP=${1:-"$ROOT/src-tauri/target/universal-apple-darwin/release/bundle/macos/JoyrIDE.app"}
IDENTITY=${APPLE_SIGNING_IDENTITY:-}
RES="$APP/Contents/Resources"
ENTS="$ROOT/src-tauri/macos"

[ -d "$APP" ] || { echo "No app at $APP" >&2; exit 1; }
[ -n "$IDENTITY" ] || { echo "Set APPLE_SIGNING_IDENTITY (\"-\" for ad hoc)." >&2; exit 1; }

sign() { # sign <path> [entitlements]
  if [ "$IDENTITY" = "-" ]; then
    set -- "$1" ${2:+--entitlements "$2"}
    codesign --force --sign - "$@"
  else
    set -- "$1" ${2:+--entitlements "$2"}
    codesign --force --sign "$IDENTITY" --options runtime --timestamp "$@"
  fi
}

# ---- --dmg: a DMG from the finished (stapled) app --------------------------
if [ -n "$DMG_MODE" ]; then
  BUNDLE=$(cd "$(dirname "$APP")/.." && pwd)
  VERSION=$(defaults read "$APP/Contents/Info" CFBundleShortVersionString)
  # Tauri names it for the build's target: _universal, _aarch64 or _x64.
  DMG=$(ls -t "$BUNDLE"/dmg/JoyrIDE_"${VERSION}"_*.dmg 2>/dev/null | head -n 1 || true)
  # A `--bundles app` build leaves an older build's DMG behind: leave it.
  if [ -z "$DMG" ] || [ ! "$DMG" -nt "$APP/Contents/Info.plist" ]; then
    echo "No DMG from this build."
    exit 0
  fi
  STAGE=$(mktemp -d "${TMPDIR:-/tmp}/joyride-dmg.XXXXXX")
  ditto "$APP" "$STAGE/JoyrIDE.app"
  ln -s /Applications "$STAGE/Applications"
  # Made beside the staging folder and moved into place once whole, so a
  # failure leaves Tauri's DMG where it was.
  hdiutil create -volname JoyrIDE -srcfolder "$STAGE" -fs HFS+ -format UDZO "$STAGE.dmg" >/dev/null
  mv -f "$STAGE.dmg" "$DMG"
  rm -rf "$STAGE"
  if [ "$IDENTITY" = "-" ]; then codesign --force --sign - "$DMG"; else codesign --force --sign "$IDENTITY" --timestamp "$DMG"; fi
  echo "Rebuilt and signed $DMG"
  exit 0
fi

# ---- 1. Symlinks ------------------------------------------------------------
# restore_links <folder in the source> <where it landed in the app>
restore_links() {
  [ -d "$1" ] || return 0
  ( cd "$1" && find . -type l ) | while IFS= read -r link; do
    dest="$2/${link#./}"
    rm -rf "$dest"
    mkdir -p "$(dirname "$dest")"
    ln -s "$(readlink "$1/${link#./}")" "$dest"
  done
}
restore_links "$ROOT/src-tauri/resources/toolchains" "$RES/resources/toolchains"
restore_links "$ROOT/src-tauri/resources/soundfonts" "$RES/resources/soundfonts"
restore_links "$ROOT/build/samples-stage/samples" "$RES/_up_/samples"
echo "Restored the bundled programs' symlinks."

# ---- 2. Everything inside, innermost first ----------------------------------
# Deepest paths first, one per line (names may hold spaces, never newlines).
deepest_first() { awk -F/ '{ print NF "\t" $0 }' | sort -rn | cut -f2-; }
is_macho() { file -b "$1" | grep -q Mach-O; }

sign_nested_app() { # sign_nested_app <app> <entitlements>
  nested=$1 ents=$2
  main="$nested/Contents/MacOS/$(defaults read "$nested/Contents/Info" CFBundleExecutable)"
  # Loose Mach-O files: dylibs and helper tools, anything but the main
  # executable (signed with the app) and what is inside a framework.
  find "$nested" -type f ! -path "*.framework/*" | deepest_first |
    while IFS= read -r f; do
      [ "$f" = "$main" ] && continue
      is_macho "$f" || continue
      sign "$f"
    done
  # Frameworks are signed whole, innermost first.
  find "$nested" -type d -name "*.framework" | deepest_first |
    while IFS= read -r fw; do sign "$fw"; done
  sign "$nested" "$ents"
}

# The bundled LÖVE(s): every app under toolchains/love-*.
find "$RES/resources/toolchains" -maxdepth 2 -type d -name "love.app" | while IFS= read -r l; do
  sign_nested_app "$l" "$ENTS/love.entitlements"
done

# Every other executable and library in the app's resources (StyLua,
# lua-language-server, nttplay, and anything a sample or a later tool adds).
find "$RES" -type f | deepest_first | while IFS= read -r f; do
  # Not what is inside a nested app (signed above); matched on the path below
  # Resources, as every path in the bundle holds "JoyrIDE.app/".
  case "${f#"$RES"/}" in *.app/*) continue ;; esac
  is_macho "$f" || continue
  sign "$f"
done

# ---- 3. The app, last -------------------------------------------------------
sign "$APP"
codesign --verify --deep --strict "$APP"

# ---- 4. Audit: every Mach-O, one by one -------------------------------------
BAD=$(mktemp "${TMPDIR:-/tmp}/joyride-audit.XXXXXX")
find "$APP" -type f | while IFS= read -r f; do
  is_macho "$f" || continue
  info=$(codesign -dvv "$f" 2>&1) || { echo "unsigned: $f" >> "$BAD"; continue; }
  codesign --verify --strict "$f" >/dev/null 2>&1 || echo "does not verify: $f" >> "$BAD"
  if [ "$IDENTITY" != "-" ]; then
    printf '%s' "$info" | grep -q "Authority=Developer ID Application" || echo "not a Developer ID: $f" >> "$BAD"
    printf '%s' "$info" | grep -q "flags=.*runtime" || echo "no hardened runtime: $f" >> "$BAD"
    printf '%s' "$info" | grep -q "^Timestamp=" || echo "no secure timestamp: $f" >> "$BAD"
  fi
done
if [ -s "$BAD" ]; then
  echo "Not everything in the app is signed as notarization needs:" >&2
  sed 's|'"$APP"'/||' "$BAD" | head -40 >&2
  rm -f "$BAD"
  exit 1
fi
rm -f "$BAD"
COUNT=$(find "$APP" -type f | while IFS= read -r f; do is_macho "$f" && echo x; done | wc -l | tr -d ' ')
echo "Signed $APP and checked $COUNT executables and libraries one by one."
