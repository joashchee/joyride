#!/bin/sh
# Builds the compiled parts of the C libraries this project benchmarks, from source, into bin/ next to the game (git-ignored: a binary is
# per platform, and never goes into the repository). macOS for now; Linux and Windows need the same three builds with their own compilers.
#
#   sh samples/benchmark-libraries/tools/build-binaries.sh
#
#   nuklear.so          love-nuklear at the commit libraries.toml pins, a plain C module (links LuaJIT's symbols from LÖVE at load time)
#   cimgui.dylib        cimgui from cimgui-love's pinned commit (Dear ImGui 1.92, docking), by CMake
#   libdiscord-rpc.dylib  Discord's discord-rpc (archived upstream, MIT) by CMake; its install name is made the bare file name so the
#                       library's own `ffi.load("discord-rpc")` finds it once lib/libload.lua has loaded it by full path
#   libneumetik_midi.dylib  neumetik-midi's C API (capi/, Rust with no dependencies) at the commit libraries.toml pins, by cargo for both
#                       Apple targets and joined with lipo; its install name is made the bare file name, as for discord-rpc
#   luasteam            NOT built: it needs Valve's Steamworks SDK, which needs a Steamworks partner login and may not be redistributed
#
# Needs: git, cmake, a C and C++ compiler, Rust with the two Apple targets (for neumetik-midi), LuaJIT's headers (Homebrew: /opt/homebrew/include/luajit-2.1). Network: github.com, codeberg.org.
# Everything is universal (arm64 and x86_64). Nothing from the cloned repositories is run.
set -eu
HERE=$(cd "$(dirname "$0")/.." && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
OUT="$HERE/bin"
WORK=${TMPDIR:-/tmp}/benchmark-libraries-build
LUAJIT_INC=${LUAJIT_INC:-/opt/homebrew/include/luajit-2.1}
mkdir -p "$OUT" "$WORK"

rev() { awk -v n="$1" '$0 == "name = \"" n "\"" { f = 1 } f && /^rev = / { gsub(/"/, "", $3); print $3; exit }' "$ROOT/joyride-core/data/libraries.toml"; }
clone() { # url dir rev
  [ -d "$2/.git" ] || git clone -q "$1" "$2"
  git -C "$2" checkout -q "$3"
  git -C "$2" submodule update --init --recursive -q
}
sign() { codesign -s - --force "$1" >/dev/null 2>&1 || true; }   # an edited Mach-O needs a fresh (ad hoc) signature on Apple silicon

echo "== love-nuklear"
clone https://github.com/keharriso/love-nuklear "$WORK/love-nuklear" "$(rev love-nuklear)"
clang -O2 -mmacosx-version-min=11.0 -shared -fPIC -arch arm64 -arch x86_64 -undefined dynamic_lookup -I"$LUAJIT_INC" "$WORK/love-nuklear/src/nuklear_love.c" -o "$OUT/nuklear.so"
sign "$OUT/nuklear.so"

echo "== cimgui"
clone https://codeberg.org/apicici/cimgui-love "$WORK/cimgui-love" "$(rev cimgui-love)"
cmake -S "$WORK/cimgui-love/cimgui" -B "$WORK/cimgui-love/cimgui/build" -DCMAKE_BUILD_TYPE=Release -DCMAKE_OSX_ARCHITECTURES="arm64;x86_64" -DCMAKE_POLICY_VERSION_MINIMUM=3.5 -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0 >/dev/null
cmake --build "$WORK/cimgui-love/cimgui/build" -j8 >/dev/null
cp "$WORK/cimgui-love/cimgui/build/cimgui.dylib" "$OUT/cimgui.dylib"

echo "== discord-rpc"
[ -d "$WORK/discord-rpc/.git" ] || git clone -q --depth 1 https://github.com/discord/discord-rpc "$WORK/discord-rpc"
# its CMake downloads rapidjson from codeload; fetch the same release from git instead
[ -d "$WORK/discord-rpc/thirdparty/rapidjson-1.1.0" ] || { mkdir -p "$WORK/discord-rpc/thirdparty"; git clone -q --depth 1 --branch v1.1.0 https://github.com/Tencent/rapidjson "$WORK/discord-rpc/thirdparty/rapidjson-1.1.0"; }
cmake -S "$WORK/discord-rpc" -B "$WORK/discord-rpc/build" -DBUILD_SHARED_LIBS=ON -DCMAKE_BUILD_TYPE=Release -DCMAKE_OSX_ARCHITECTURES="arm64;x86_64" -DCMAKE_POLICY_VERSION_MINIMUM=3.5 -DCMAKE_OSX_DEPLOYMENT_TARGET=11.0 >/dev/null
cmake --build "$WORK/discord-rpc/build" -j8 --target discord-rpc >/dev/null
cp "$WORK/discord-rpc/build/src/libdiscord-rpc.dylib" "$OUT/libdiscord-rpc.dylib"
install_name_tool -id libdiscord-rpc.dylib "$OUT/libdiscord-rpc.dylib" 2>/dev/null || true
sign "$OUT/libdiscord-rpc.dylib"

echo "== neumetik-midi"
clone https://github.com/joashchee/neumetik-midi "$WORK/neumetik-midi" "$(rev neumetik-midi)"
for t in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo build --quiet --release --manifest-path "$WORK/neumetik-midi/capi/Cargo.toml" --target "$t"
done
lipo -create -output "$OUT/libneumetik_midi.dylib" "$WORK"/neumetik-midi/capi/target/aarch64-apple-darwin/release/libneumetik_midi.dylib "$WORK"/neumetik-midi/capi/target/x86_64-apple-darwin/release/libneumetik_midi.dylib
install_name_tool -id libneumetik_midi.dylib "$OUT/libneumetik_midi.dylib" 2>/dev/null || true
sign "$OUT/libneumetik_midi.dylib"

ls -l "$OUT"
