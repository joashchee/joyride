#!/usr/bin/env python3
"""Writes docs/third-party-licenses.txt: the license texts of everything
JoyrIDE ships that someone else wrote, as their licenses ask (MIT and BSD:
"the above copyright notice and this permission notice shall be included").
The app will show it under About, and every release includes it.

Included:
- every Rust crate the shipping crates link (normal and build dependencies,
  from asset-core, joyride-core and src-tauri once it exists; not dev
  dependencies), with the license files in each crate's folder;
- every npm production package, once there is a package-lock.json;
- the third-party notices of the vendored engines (src/vendor/*/NOTICE and
  their LICENSES/ folders: CoffeeMUD data, the IBM VGA font);
- NOTICE: the tools bundled with the app (LÖVE, lua-language-server...).

Identical texts are written once, with every package that uses them. A crate
that publishes no license file is listed separately with the license it
declares, and the script says so on stderr: add its text by hand to
scripts/licenses/<spdx>.txt (the script uses a file there for the declared
license) or find the crate's real file.

Run it whenever Cargo.lock or package-lock.json changes (with
license-scan.py), and commit the result. Usage:
  scripts/third-party-licenses.py [--online] [--check]
--check exits 1 if the committed file is out of date, for CI.
"""
import hashlib, json, re, subprocess, sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs/third-party-licenses.txt"
CRATES = ["asset-core", "joyride-core", "ogv", "src-tauri"]
OFFLINE = "--online" not in sys.argv
LICENSE_FILE = re.compile(r"^(licen[sc]e|copying|unlicense|notice|copyright)([-_.].*)?$", re.I)
TEMPLATES = ROOT / "scripts/licenses"

def license_files(d: Path):
    if not d.is_dir():
        return []
    found = []
    for p in sorted(d.iterdir()):
        if p.is_file() and LICENSE_FILE.match(p.name) and p.stat().st_size < 200_000:
            found.append(p)
    # Some crates keep them in a licenses/ folder.
    for sub in ("licenses", "LICENSES", "license"):
        if (d / sub).is_dir():
            found += [p for p in sorted((d / sub).iterdir()) if p.is_file() and p.stat().st_size < 200_000]
    return found

def rust_packages():
    seen = {}
    for crate in CRATES:
        manifest = ROOT / crate / "Cargo.toml"
        if not manifest.exists():
            continue
        meta = json.loads(subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--manifest-path", str(manifest)] + (["--offline"] if OFFLINE else []),
            check=True, capture_output=True, text=True).stdout)
        nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
        pkgs = {p["id"]: p for p in meta["packages"]}
        done, stack = set(), [meta["resolve"]["root"]]
        while stack:
            pid = stack.pop()
            if pid in done:
                continue
            done.add(pid)
            for dep in nodes[pid]["deps"]:
                if any(k["kind"] in (None, "build") for k in dep["dep_kinds"]):
                    stack.append(dep["pkg"])
        for pid in done:
            p = pkgs[pid]
            if p["source"] is not None:
                seen[(p["name"], p["version"])] = p
    return [seen[k] for k in sorted(seen)]

def npm_packages():
    lock = ROOT / "package-lock.json"
    if not lock.exists():
        return
    for path, info in json.loads(lock.read_text()).get("packages", {}).items():
        if path and not info.get("dev"):
            yield path.split("node_modules/")[-1], info.get("version", "?"), ROOT / path

def norm(text: str) -> str:
    return "\n".join(l.rstrip() for l in text.strip().splitlines())

def build():
    groups = defaultdict(list)   # normalised text -> [package labels]
    texts = {}
    declared_only = []
    def add(label, files, declared):
        got = False
        for f in files:
            try:
                t = norm(f.read_text(errors="replace"))
            except OSError:
                continue
            if t:
                h = hashlib.sha256(t.encode()).hexdigest()
                groups[h].append(label); texts[h] = t; got = True
        if not got:
            declared_only.append((label, declared))
    for p in rust_packages():
        add(f"{p['name']} {p['version']}", license_files(Path(p["manifest_path"]).parent), p.get("license") or "(none declared)")
    for name, version, d in npm_packages():
        add(f"{name} {version} (npm)", license_files(d), "(see package.json)")
    for notice in sorted((ROOT / "src/vendor").glob("*/NOTICE")):
        add(f"{notice.parent.name} (vendored: its notice)", [notice] + license_files(notice.parent / "LICENSES"), "")
        for f in sorted((notice.parent / "docs/fonts").glob("*LICENSE*")) if (notice.parent / "docs/fonts").is_dir() else []:
            add(f"{notice.parent.name}: font license", [f], "")
    add("Bundled tools (NOTICE)", [ROOT / "NOTICE"], "")
    out = ["JoyrIDE third-party licenses",
           "Generated by scripts/third-party-licenses.py; do not edit.", ""]
    # Dedupe the many copies of the same text, but keep each package's name with it.
    for h in sorted(groups, key=lambda h: (-len(groups[h]), h)):
        out.append("=" * 78)
        out.append(", ".join(sorted(set(groups[h]))))
        out.append("=" * 78)
        out.append(texts[h])
        out.append("")
    if declared_only:
        out.append("=" * 78)
        out.append("Packages that publish no license file (license as declared):")
        out.append("=" * 78)
        for label, lic in declared_only:
            out.append(f"  {label}: {lic}")
        out.append("")
    return "\n".join(out), declared_only

def main():
    text, declared_only = build()
    if declared_only:
        print(f"{len(declared_only)} package(s) publish no license file (listed as declared only):", file=sys.stderr)
        for label, lic in declared_only[:20]:
            print(f"  {label}: {lic}", file=sys.stderr)
    if "--check" in sys.argv:
        if not OUT.exists() or OUT.read_text() != text:
            sys.exit(f"{OUT.relative_to(ROOT)} is out of date: run scripts/third-party-licenses.py")
        print("third-party-licenses.txt is up to date.")
        return
    OUT.parent.mkdir(exist_ok=True)
    OUT.write_text(text)
    print(f"wrote {OUT.relative_to(ROOT)}: {len(text.splitlines())} lines")

if __name__ == "__main__":
    main()
