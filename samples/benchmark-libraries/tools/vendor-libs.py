#!/usr/bin/env python3
"""Puts every library JoyrIDE ships (joyride-core/data/libraries.toml) into this project's libs/ folder, each at its pinned commit,
and writes love.toml (the dependencies) and love.lock, exactly what JoyrIDE's installer would write, so the sample runs with no network.

    python3 -I samples/benchmark-libraries/tools/vendor-libs.py [--only a,b]

Nothing from a library is run. Each is cloned with no checkout, checked out at its exact commit, and copied without `.git`, without
submodules, and without documentation, tests, pictures and sound (a library's source and licence files are what the benchmark needs).
The two native libraries (luasteam, love-nuklear) and cimgui-love carry compiled code or SDK files: their Lua side only is kept.
Run it again after libraries.toml changes; a library already at its commit is left alone.
"""
import glob, os, re, shutil, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.dirname(HERE)
LIST = os.path.join(PROJECT, "..", "..", "joyride-core", "data", "libraries.toml")
DROP_DIRS = {".git", ".vscode", ".idea", ".github", ".gitlab", "docs", "doc", "documentation", "img", "images", "image", "screenshots", "media", "spec", "specs", "test", "tests", "rockspecs",
             "examples", "example", "demo", "demos", "samples", "sample", "benchmark", "benchmarks", "bin", "build", "dist", "scripts", "tools", "luasteam-examples"}
# Per library: more to leave out. luasteam's repository carries Valve's Steamworks SDK (Valve's own agreement: never redistributed here), and
# its generator and type stubs are build tools; cimgui-love's C parser is a build tool for regenerating its bindings; neumetik-midi's
# Rust source (built by tools/build-binaries.sh from its repository) is not needed to run the Lua binding.
EXTRA_DROP = {"luasteam": {"sdk", "generator", "luals"}, "cimgui-love": {"cparser"}, "neumetik-midi": {"src", "capi"}, "serpent": {"t", "misc"}}
DROP_FILES = {"cimgui-love": {"generator.lua"}}
# Files a library loads when it is required or set up, so they stay although their type is dropped (the entries are paths from the
# repository's root; `*` and `**` work). lighter creates its default light from the image; LoveFrames registers its three skins when it loads
# and stops with "Could not find an image directory for skin" without their pictures (48 small PNGs, 34 KB); 3DreamEngine's init.lua loads its sky,
# cube and plane models (objects/*.obj, 63 KB) and six pictures (res/, 48 KB) when it loads and stops at the first one missing (the Reports found them
# one at a time: sky, then cube); its res/cubeUV.png (131 KB) is not loaded by its Lua and is left out. All are files of the library's own repository, under its licence.
# lovr-ui2d finds its font (ui2d/DejaVuSansMono.ttf, DejaVu Sans Mono 2.34, 335 KB) beside its own file. The repository states no licence for
# it (its LICENSE covers the code), but the font carries one in its own name table: the Bitstream Vera licence, and DejaVu's changes are public
# domain. The maintainer confirmed it (2026-11-15) after reading it: free to copy and distribute with software, the notices and permission text
# in every copy (FONT_LICENCE writes them beside it, read out of the font itself), the font not sold by itself, unmodified. CLAUDE.md rule 11.
KEEP_FILES = {"lighter": ("media/default_light.png",), "LoveFrames": ("loveframes/skins/*/images/**/*.png",), "3DreamEngine": ("3DreamEngine/objects/*.obj", "3DreamEngine/res/default.png", "3DreamEngine/res/defaultNormal.png", "3DreamEngine/res/godray.png",
                              "3DreamEngine/res/noise.png", "3DreamEngine/res/foam.png", "3DreamEngine/res/brdfLut.png"),
              "lovr-ui2d": ("ui2d/DejaVuSansMono.ttf",)}
# A kept font and the file its licence is written to: the copyright and licence text from the font's own `name` table (IDs 0 and 13).
FONT_LICENCE = {"lovr-ui2d": ("ui2d/DejaVuSansMono.ttf", "ui2d/LICENSE-DejaVu.txt")}


def font_licence_text(path):
    """The copyright and licence text a TrueType font carries in its `name` table, as written there."""
    import struct
    b = open(path, "rb").read()
    count_tables = struct.unpack(">H", b[4:6])[0]
    tables = {b[12 + 16 * i:16 + 16 * i].decode("latin1"): struct.unpack(">II", b[20 + 16 * i:28 + 16 * i]) for i in range(count_tables)}
    off, _ = tables["name"]
    _, count, string_offset = struct.unpack(">HHH", b[off:off + 6])
    found = {}
    for i in range(count):
        platform, _, language, name_id, length, o = struct.unpack(">6H", b[off + 6 + 12 * i:off + 18 + 12 * i])
        raw = b[off + string_offset + o:off + string_offset + o + length]
        if (platform, language) == (3, 0x409):
            found.setdefault(name_id, raw.decode("utf-16-be"))
        elif (platform, language) == (1, 0):
            found.setdefault(name_id + 1000, raw.decode("latin1"))
    copyright_text = found.get(0) or found.get(1000)
    licence_text = found.get(13) or found.get(1013)
    if not copyright_text or not licence_text:
        raise RuntimeError(f"{path}: the font carries no copyright and licence text in its name table")
    return copyright_text.strip() + "\n\n" + licence_text.strip() + "\n"
DROP_EXT = {".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp", ".ogg", ".wav", ".mp3", ".flac", ".mid", ".ttf", ".otf", ".woff", ".woff2", ".ico", ".icns", ".psd", ".xcf", ".zip", ".gz",
            ".dll", ".so", ".dylib", ".a", ".lib", ".exe", ".o", ".obj", ".mtl", ".pdf", ".rockspec", ".yml", ".yaml", ".sh", ".bat", ".ps1", ".cpp", ".cc", ".c", ".h", ".hpp", ".cmake", ".mk"}


def entries():
    out, cur = [], None
    for line in open(LIST, encoding="utf-8"):
        line = line.strip()
        if line == "[[library]]":
            cur = {}
            out.append(cur)
        elif cur is not None and "=" in line and not line.startswith("#"):
            k, v = line.split("=", 1)
            m = re.match(r'\s*"(.*)"\s*$', v)
            if m:
                cur[k.strip()] = m.group(1)
    return out


def git(*args, cwd=None):
    r = subprocess.run(["git", "-c", "protocol.ext.allow=never", "-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false", *args], cwd=cwd, capture_output=True, text=True)
    if r.returncode:
        raise RuntimeError(r.stderr.strip())
    return r.stdout


def vendor(e, libs):
    dest = os.path.join(libs, e["name"])
    mark = os.path.join(dest, ".joyride-commit")
    if os.path.isfile(mark) and open(mark).read().strip() == e["rev"]:
        return "unchanged"
    tmp = tempfile.mkdtemp(prefix="vendor-")
    try:
        src = os.path.join(tmp, "src")
        git("clone", "--quiet", "--no-checkout", e["git"], src)
        git("checkout", "--quiet", "--detach", e["rev"], cwd=src)
        if git("rev-parse", "HEAD", cwd=src).strip() != e["rev"]:
            raise RuntimeError("wrong commit")
        if os.path.isdir(dest):
            shutil.rmtree(dest)
        for root, dirs, files in os.walk(src):
            rel = os.path.relpath(root, src)
            # A folder on the entry's `paths` stays even when its name is usually dropped (SYSL-Text's code is in example/library).
            wanted = [x.strip().strip("/") for x in e.get("paths", "").split(";") if x.strip()]
            def on_path(d):
                full = os.path.normpath(os.path.join(rel, d)).strip("./")
                return any(w == full or w.startswith(full + "/") or full.startswith(w + "/") for w in wanted)
            dirs[:] = [d for d in dirs if on_path(d) or (d not in DROP_DIRS and d not in EXTRA_DROP.get(e["name"], ()))]
            for f in files:
                p = os.path.join(root, f)
                if os.path.islink(p):
                    continue
                ext = os.path.splitext(f)[1].lower()
                licence = re.match(r"(?i)(licen[cs]e|copying|copyright|notice|unlicense)", f) is not None
                if ext in DROP_EXT and not licence:
                    continue
                if f in DROP_FILES.get(e["name"], ()):
                    continue
                if f.startswith(".") or f in {"Makefile", "CMakeLists.txt", "Dockerfile"}:
                    continue
                if ext == ".md" and not licence and f.lower() != "readme.md":
                    continue
                out = os.path.normpath(os.path.join(dest, rel, f))
                os.makedirs(os.path.dirname(out), exist_ok=True)
                shutil.copy2(p, out)
        for pattern in KEEP_FILES.get(e["name"], ()):
            hits = [h for h in glob.glob(os.path.join(src, pattern), recursive=True) if os.path.isfile(h) and not os.path.islink(h)]
            if not hits:
                raise RuntimeError(f"{e['name']}: no file matches the kept path {pattern} (did the repository move it?)")
            for h in hits:
                out = os.path.join(dest, os.path.relpath(h, src))
                os.makedirs(os.path.dirname(out), exist_ok=True)
                shutil.copy2(h, out)
        if e["name"] in FONT_LICENCE:
            ttf, licence_file = FONT_LICENCE[e["name"]]
            open(os.path.join(dest, licence_file), "w", encoding="utf-8").write(font_licence_text(os.path.join(dest, ttf)))
        open(mark, "w").write(e["rev"] + "\n")
        return "vendored"
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def main():
    only = None
    if "--only" in sys.argv:
        only = set(sys.argv[sys.argv.index("--only") + 1].split(","))
    libs = os.path.join(PROJECT, "libs")
    os.makedirs(libs, exist_ok=True)
    es = entries()
    for e in es:
        if only and e["name"] not in only:
            continue
        try:
            print(f"{e['name']:14} {vendor(e, libs)}", flush=True)
        except Exception as ex:
            print(f"{e['name']:14} FAILED {ex}", file=sys.stderr)
            sys.exit(1)
    # love.toml and love.lock as JoyrIDE's own installer writes them (BTreeMap order = sorted by name).
    es.sort(key=lambda e: e["name"])
    toml = ['[project]', 'name = "Benchmark Libraries"', 'bundle_id = "dev.joyride.benchmarklibraries"', 'version = "1.0.0"', 'love_version = "11.5"', ""]
    lock = ["# Written by JoyrIDE. Commit it."]
    for e in es:
        toml += [f'[dependencies.{e["name"]}]' if re.match(r"^[A-Za-z0-9_-]+$", e["name"]) else f'[dependencies."{e["name"]}"]', f'git = "{e["git"]}"', f'rev = "{e["rev"]}"', ""]
        lock += ["", "[[package]]", f'name = "{e["name"]}"', f'git = "{e["git"]}"', f'spec = "rev={e["rev"]}"', f'commit = "{e["rev"]}"']
    open(os.path.join(PROJECT, "love.toml"), "w").write("\n".join(toml))
    open(os.path.join(PROJECT, "love.lock"), "w").write("\n".join(lock) + "\n")
    # lib/catalog.lua: the curated list's facts about each library (what to require, what kind it is, its licence and commit), so the
    # benchmark never has a second list to keep in step.
    def q(v): return '"' + v.replace("\\", "\\\\").replace('"', '\\"') + '"'
    def size(name):
        """Lua lines, bytes and files of a library as installed (a demo's main.lua and conf.lua at its root are not the library)."""
        lines = nbytes = files = 0
        base = os.path.join(libs, name)
        for root, _, fs in os.walk(base):
            for f in fs:
                if not f.endswith(".lua") or (root == base and f in ("main.lua", "conf.lua")):
                    continue
                data = open(os.path.join(root, f), "rb").read()
                lines += data.count(b"\n") + 1
                nbytes += len(data)
                files += 1
        return lines, nbytes, files

    out = ["-- Written by tools/vendor-libs.py from joyride-core/data/libraries.toml. Do not edit.", "return {"]
    for e in es:
        lo, by, fi = size(e["name"])
        out.append("  { name = %s, loc = %d, bytes = %d, files = %d, module = %s, kind = %s, licence = %s, rev = %s, tag = %s, summary = %s, native = %s, paths = %s }," % (
            q(e["name"]), lo, by, fi, q(e["module"]), q(e.get("kind", "lua")), q(e.get("licence", "?")), q(e["rev"]), q(e["tag"]) if "tag" in e else "nil", q(e.get("summary", "")), q(e["native"]) if "native" in e else "nil", q(e["paths"]) if "paths" in e else "nil"))
    out.append("}")
    open(os.path.join(PROJECT, "lib", "catalog.lua"), "w", encoding="utf-8").write("\n".join(out) + "\n")
    print("wrote love.toml, love.lock and lib/catalog.lua")


main()
