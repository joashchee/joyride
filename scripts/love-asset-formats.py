#!/usr/bin/env python3
"""Measures which asset files the real LÖVE 11.5 and 12.0 load.

Asks each LÖVE to load every sample headless (`love.image.newImageData`, `love.sound.newDecoder`,
`love.font.newRasterizer`, `love.filesystem.load`; no window and no OpenGL) and prints one line per
file with the result on each version. The samples are:

  * tiny files this script writes (PPM, PGM, HDR, TGA, GIF, PSD, QOI, a MIDI under other names), and
  * the real sample files of the r3i6nos corpus (`corpus/files`: tracker modules in 20 formats,
    TTF/TTC/WOFF/WOFF2/BDF fonts, GIF, PSD, PCX, TIFF, ICO, WebP, QOI, Lua and LuaJIT bytecode,
    FLAC, MP3, Ogg, Opus, AIFF, ...), when that checkout is beside this one.

The results are the evidence behind `docs/love-asset-types.md` and `asset-core/src/kind.rs`
(`love_support`). A refusal of a corpus sample says LÖVE refused *that file*: the corpus was built
for other programs, so a format LÖVE lists but refuses here is reported as not confirmed, not as
unsupported.

    scripts/love-asset-formats.py [folder-with-<version>/love.app]

Default folder: ../r3i6nos/vendor/love-mac. Needs a Mac with those LÖVEs.
"""
import os, shutil, struct, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOVES = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT.parent / "r3i6nos/vendor/love-mac"
CORPUS = ROOT.parent / "r3i6nos/corpus/files"


def tiny(d: Path):
    (d / "tiny.ppm").write_bytes(b"P6\n2 1\n255\n" + bytes([255, 0, 0, 0, 0, 255]))
    (d / "tiny.hdr").write_bytes(b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 1 +X 2\n" + bytes([128, 0, 0, 129, 0, 0, 128, 129]))
    (d / "tiny.tga").write_bytes(bytes([0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 1, 0, 24, 0x20]) + bytes([0, 0, 255, 255, 0, 0]))
    track = bytes([0, 0x90, 60, 100, 96, 0x80, 60, 0, 0, 0xFF, 0x2F, 0])
    mid = b"MThd" + struct.pack(">IHHH", 6, 0, 1, 96) + b"MTrk" + struct.pack(">I", len(track)) + track
    # LÖVE picks an audio decoder by content, so the name does not matter.
    for name in ("tiny.mid", "tiny.midi", "tiny.xyz"):
        (d / name).write_bytes(mid)


def textures(d: Path):
    """One tiny file of each compressed-texture container, built by hand from the format headers."""
    u = lambda *v: struct.pack("<" + "I" * len(v), *v)

    def dds(four, w, h, data, dx10=None):
        pf = u(32, 4) + four + u(0, 0, 0, 0, 0)
        head = b"DDS " + u(124, 0x81007, h, w, len(data), 0, 1) + bytes(44) + pf + u(0x1000, 0, 0, 0, 0)
        return head + (u(dx10, 3, 0, 1, 0) if dx10 else b"") + data
    (d / "bc1.dds").write_bytes(dds(b"DXT1", 4, 4, bytes(8)))
    (d / "bc3.dds").write_bytes(dds(b"DXT5", 4, 4, bytes(16)))
    (d / "bc7.dds").write_bytes(dds(b"DX10", 4, 4, bytes(16), dx10=98))
    # KTX 1: glInternalFormat 0x9278 (ETC2 RGBA8), 4x4, one 16-byte block.
    ktx = b"\xabKTX 11\xbb\r\n\x1a\n" + u(0x04030201, 0, 1, 0, 0x9278, 0x1908, 4, 4, 0, 0, 1, 1, 0) + u(16) + bytes(16)
    (d / "etc2.ktx").write_bytes(ktx)
    (d / "etc1.pkm").write_bytes(b"PKM 10" + struct.pack(">HHHHH", 0, 4, 4, 4, 4) + bytes(8))
    # PVR 3: PVRTC 4bpp RGBA (format 3), 8x8, one 32-byte level.
    (d / "pvrtc.pvr").write_bytes(b"PVR\x03" + u(0, 3, 0, 0, 0, 8, 8, 1, 1, 1, 1, 0) + bytes(32))
    (d / "astc4.astc").write_bytes(b"\x13\xab\xa1\x5c" + bytes([4, 4, 1, 4, 0, 0, 4, 0, 0, 1, 0, 0]) + bytes(16))
    (d / "tiny.exr").write_bytes(exr(2, 1))


def exr(pixel_type: int, compression: int) -> bytes:
    """A 4x4 scanline OpenEXR (pixel type 1 half, 2 float; compression 0 none, 1 RLE). Not tiled: LÖVE aborts on a tiled one."""
    attr = lambda n, t, v: n.encode() + b"\0" + t.encode() + b"\0" + struct.pack("<I", len(v)) + v
    ch = b"".join(c.encode() + b"\0" + struct.pack("<iBBBBii", pixel_type, 0, 0, 0, 0, 1, 1) for c in "BGR") + b"\0"
    box = struct.pack("<iiii", 0, 0, 3, 3)
    hdr = b"v/1\x01" + struct.pack("<I", 2) + attr("channels", "chlist", ch) + attr("compression", "compression", bytes([compression]))
    hdr += attr("dataWindow", "box2i", box) + attr("displayWindow", "box2i", box) + attr("lineOrder", "lineOrder", b"\0")
    hdr += attr("pixelAspectRatio", "float", struct.pack("<f", 1)) + attr("screenWindowCenter", "v2f", struct.pack("<ff", 0, 0)) + attr("screenWindowWidth", "float", struct.pack("<f", 1)) + b"\0"
    px = struct.pack("<H", 0x3800) if pixel_type == 1 else struct.pack("<f", 0.5)
    chunks, offsets, base = [], [], len(hdr) + 8 * 4
    for y in range(4):
        raw = px * 12
        if compression:  # EXR's RLE: split bytes in two halves, delta-code, then literal runs
            t = bytearray(raw[0::2] + raw[1::2])
            for i in range(len(t) - 1, 0, -1):
                t[i] = (t[i] - t[i - 1] + 128) & 255
            raw = b"".join(bytes([256 - n]) + bytes(t[i:i + n]) for i in range(0, len(t), 127) for n in [min(127, len(t) - i)])
        offsets.append(base + sum(map(len, chunks)))
        chunks.append(struct.pack("<iI", y, len(raw)) + raw)
    return hdr + b"".join(struct.pack("<Q", o) for o in offsets) + b"".join(chunks)


def corpus(d: Path):
    if not CORPUS.is_dir():
        return
    for sub in ("music/trackers", "music/midi", "fonts", "images", "audio"):
        for f in (CORPUS / sub).glob("*"):
            if f.is_file() and f.stat().st_size < 3_000_000:
                shutil.copy(f, d / f.name)
    for rel in ("text/font.bdf", "programs/chunk.luac", "programs/chunk.ljbc"):
        if (CORPUS / rel).is_file():
            shutil.copy(CORPUS / rel, d / Path(rel).name)


CONF = "function love.conf(t) t.window=nil; for _,m in ipairs{'window','graphics','audio','joystick','video'} do t.modules[m]=false end end\n"
MAIN = """
local IMG = {png=1,jpg=1,jpeg=1,gif=1,bmp=1,tga=1,webp=1,tiff=1,tif=1,ico=1,qoi=1,pcx=1,psd=1,hdr=1,exr=1,ppm=1,pgm=1,pbm=1,pnm=1,avif=1,heic=1,svg=1}
local TEX = {dds=1,ktx=1,pkm=1,pvr=1,astc=1}
local FONT = {ttf=1,otf=1,ttc=1,woff=1,woff2=1,bdf=1,psf=1,pcf=1,pfb=1}
local BYTECODE = {luac=1,ljbc=1}
local items = love.filesystem.getDirectoryItems("")
table.sort(items)
for _, f in ipairs(items) do
  local ext = f:match("%.([^.]+)$")
  if ext and f ~= "main.lua" and f ~= "conf.lua" then
    ext = ext:lower()
    local ok, kind
    if TEX[ext] then kind = "texture"; ok = pcall(love.image.newCompressedData, f)
    elseif IMG[ext] then kind = "image"; ok = pcall(love.image.newImageData, f)
    elseif FONT[ext] then kind = "font"; ok = pcall(love.font.newRasterizer, f, 16)
    elseif BYTECODE[ext] then kind = "lua"; ok = pcall(love.filesystem.load, f)
    else kind = "audio"; ok = pcall(love.sound.newDecoder, f) end
    print(kind, f, ok and "loads" or "refused")
  end
end
love.event.quit()
"""


def main():
    found = [v for v in ("11.5", "12.0") if (LOVES / v / "love.app/Contents/MacOS/love").is_file()]
    if not found:
        sys.exit(f"no LÖVE found under {LOVES}")
    results = {}
    with tempfile.TemporaryDirectory() as t:
        d = Path(t)
        tiny(d)
        textures(d)
        corpus(d)
        (d / "conf.lua").write_text(CONF)
        (d / "main.lua").write_text(MAIN)
        for v in found:
            env = dict(os.environ, SDL_VIDEODRIVER="dummy", SDL_AUDIODRIVER="dummy")
            r = subprocess.run([str(LOVES / v / "love.app/Contents/MacOS/love"), str(d)], cwd=d, env=env, capture_output=True, text=True, timeout=180)
            for line in r.stdout.splitlines():
                p = line.split("\t")
                if len(p) == 3:
                    results.setdefault((p[0], p[1]), {})[v] = p[2]
    print("kind    file                         " + "  ".join(f"LÖVE {v}" for v in found))
    for (kind, f), r in sorted(results.items()):
        print(f"{kind:7} {f:28} " + "  ".join(f"{r.get(v, '-'):9}" for v in found))


main()
