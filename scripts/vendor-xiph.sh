#!/usr/bin/env bash
# Vendors libogg and libtheora's DECODER into vendor/xiph/ (BSD-3-Clause,
# Xiph.org Foundation), so the file preview can play Theora video in Ogg
# (`.ogv`, the only video LÖVE plays). Pinned by SHA-256: the download is
# refused unless it matches. Only the portable C decoder is copied (no
# encoder, no x86 or ARM assembly, no build system); `config_types.h` is
# written for the platforms we build on (stdint.h).
#
#   scripts/vendor-xiph.sh              download from downloads.xiph.org
#   scripts/vendor-xiph.sh DIR          use the two tarballs already in DIR
set -euo pipefail
OGG=libogg-1.3.6; OGG_SHA=5c8253428e181840cd20d41f3ca16557a9cc04bad4a3d04cce84808677fa1061
THEORA=libtheora-1.2.0; THEORA_SHA=ebdf77a8f5c0a8f7a9e42323844fa09502b34eb1d1fece7b5f54da41fe2122ec
root="$(cd "$(dirname "$0")/.." && pwd)"
dest="$root/vendor/xiph"
work="$(mktemp -d "${TMPDIR:-/tmp}/vendor-xiph.XXXXXX")"; trap 'rm -rf "$work"' EXIT
fetch() { # <dir> <url path> <name> <sha>
  local f="$work/$3.tar.xz"
  if [ -n "${1:-}" ]; then cp "$1/$3.tar.xz" "$f"; else curl -fsSL -o "$f" "https://downloads.xiph.org/releases/$2/$3.tar.xz"; fi
  [ "$(shasum -a 256 "$f" | cut -d' ' -f1)" = "$4" ] || { echo "$3: SHA-256 does not match the pin" >&2; exit 1; }
  tar -xJf "$f" -C "$work"
}
fetch "${1:-}" ogg "$OGG" "$OGG_SHA"
fetch "${1:-}" theora "$THEORA" "$THEORA_SHA"
rm -rf "$dest"; mkdir -p "$dest/ogg/src" "$dest/ogg/include/ogg" "$dest/theora/lib" "$dest/theora/include/theora"
o="$work/$OGG"; t="$work/$THEORA"
cp "$o/src/bitwise.c" "$o/src/framing.c" "$o/src/crctable.h" "$dest/ogg/src/"
cp "$o/include/ogg/ogg.h" "$o/include/ogg/os_types.h" "$dest/ogg/include/ogg/"
cp "$o/COPYING" "$o/AUTHORS" "$dest/ogg/"
cat > "$dest/ogg/include/ogg/config_types.h" <<'H'
#ifndef __CONFIG_TYPES_H__
#define __CONFIG_TYPES_H__
/* Written by scripts/vendor-xiph.sh for platforms with <stdint.h>
   (macOS, Linux, Windows with MSVC 2010 or later). */
#include <stdint.h>
typedef int16_t ogg_int16_t;
typedef uint16_t ogg_uint16_t;
typedef int32_t ogg_int32_t;
typedef uint32_t ogg_uint32_t;
typedef int64_t ogg_int64_t;
typedef uint64_t ogg_uint64_t;
#endif
H
for f in apiwrapper.c apiwrapper.h bitpack.c bitpack.h dct.h decapiwrapper.c decinfo.c decint.h decode.c dequant.c dequant.h fragment.c huffdec.c huffdec.h huffman.h idct.c info.c internal.c internal.h modedec.h ocintrin.h quant.c quant.h state.c state.h; do cp "$t/lib/$f" "$dest/theora/lib/"; done
cp "$t/include/theora/"{codec.h,theora.h,theoradec.h,theoraenc.h} "$dest/theora/include/theora/"
cp "$t/COPYING" "$t/AUTHORS" "$dest/theora/"
cat > "$dest/NOTICE" <<N
libogg $OGG and the decoder of libtheora $THEORA
Copyright Xiph.org Foundation. BSD-3-Clause (COPYING in ogg/ and theora/).
Vendored unmodified by scripts/vendor-xiph.sh, pinned by SHA-256:
  $OGG.tar.xz     $OGG_SHA
  $THEORA.tar.xz  $THEORA_SHA
Only the portable C decoder is included: no encoder, no assembly.
ogg/include/ogg/config_types.h is written by the script, not from upstream.
N
echo "vendored: $(find "$dest" -type f | wc -l | tr -d ' ') files, $(du -sk "$dest" | cut -f1) KB"
