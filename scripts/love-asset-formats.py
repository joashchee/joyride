#!/usr/bin/env python3
"""Measures which image and font files the real LÖVE 11.5 and 12.0 load.

Makes tiny sample files (PPM, PGM, HDR, TGA, GIF, PSD, QOI; the system's TTF, OTF and TTC fonts and
this repo's WOFF), asks each LÖVE to decode them headless (`love.image.newImageData`,
`love.font.newRasterizer`, no window and no OpenGL), and prints one line per file.
Its results are the evidence behind `docs/love-asset-types.md`.

    scripts/love-asset-formats.py [path-to-folder-with-<version>/love.app]

Default folder: ../r3i6nos/vendor/love-mac. Needs a Mac with those LÖVEs; skips what is absent.
"""
import os, shutil, struct, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOVES = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT.parent / "r3i6nos/vendor/love-mac"

def samples(d: Path):
    (d / "a.ppm").write_bytes(b"P6\n2 1\n255\n" + bytes([255, 0, 0, 0, 0, 255]))
    (d / "a.pgm").write_bytes(b"P5\n2 1\n255\n" + bytes([10, 200]))
    (d / "a.hdr").write_bytes(b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 1 +X 2\n" + bytes([128, 0, 0, 129, 0, 0, 128, 129]))
    (d / "a.tga").write_bytes(bytes([0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 1, 0, 24, 0x20]) + bytes([0, 0, 255, 255, 0, 0]))
    (d / "a.gif").write_bytes(b"GIF89a" + struct.pack("<HH", 2, 1) + bytes([0x80, 0, 0]) + bytes([255, 0, 0, 0, 0, 255]) + b"\x2c" + struct.pack("<HHHH", 0, 0, 2, 1) + bytes([0, 2, 2, 0x44, 0x01, 0]) + b"\x3b")
    (d / "a.psd").write_bytes(b"8BPS" + struct.pack(">H", 1) + bytes(6) + struct.pack(">HIIHH", 3, 1, 2, 8, 3) + struct.pack(">I", 0) * 3 + struct.pack(">H", 0) + bytes([255, 0, 0, 0, 0, 255]))
    (d / "a.qoi").write_bytes(b"qoif" + struct.pack(">IIBB", 2, 1, 3, 0) + bytes([0xFE, 255, 0, 0, 0xFE, 0, 0, 255]) + bytes([0] * 7 + [1]))
    for src, name in [("/System/Library/Fonts/Supplemental/Arial.ttf", "a.ttf"), ("/System/Library/Fonts/Helvetica.ttc", "a.ttc"), ("/System/Library/Fonts/LastResort.otf", "a.otf"),
                      (str(ROOT / "public/fonts/ansiapps/WebPlus_IBM_VGA_8x16.woff"), "a.woff")]:
        if Path(src).is_file():
            shutil.copy(src, d / name)

CONF = "function love.conf(t) t.window=nil; for _,m in ipairs{'window','graphics','audio','joystick','video'} do t.modules[m]=false end end\n"
MAIN = """
for _, f in ipairs{"a.ppm","a.pgm","a.hdr","a.tga","a.gif","a.psd","a.qoi"} do
  local ok, r = pcall(love.image.newImageData, f)
  print("image", f, ok and "loads" or "refused")
end
for _, f in ipairs{"a.ttf","a.otf","a.ttc","a.woff"} do
  local ok, r = pcall(love.font.newRasterizer, f, 16)
  print("font", f, ok and "loads" or "refused")
end
love.event.quit()
"""

def main():
    found = [v for v in ("11.5", "12.0") if (LOVES / v / "love.app/Contents/MacOS/love").is_file()]
    if not found:
        sys.exit(f"no LÖVE found under {LOVES}")
    with tempfile.TemporaryDirectory() as t:
        d = Path(t)
        samples(d)
        (d / "conf.lua").write_text(CONF)
        (d / "main.lua").write_text(MAIN)
        for v in found:
            env = dict(os.environ, SDL_VIDEODRIVER="dummy", SDL_AUDIODRIVER="dummy")
            r = subprocess.run([str(LOVES / v / "love.app/Contents/MacOS/love"), str(d)], cwd=d, env=env, capture_output=True, text=True, timeout=60)
            print(f"== LÖVE {v}")
            print(r.stdout + r.stderr, end="")

main()
