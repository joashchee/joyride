#!/usr/bin/env python3
"""Fetches the tools the IDE bundles into src-tauri/resources/toolchains/,
pinned by exact version and SHA-256 in scripts/toolchains.lock.json.

  scripts/fetch-toolchains.py            # download what the lock pins, verify, extract
  scripts/fetch-toolchains.py --update   # re-resolve the latest releases, rewrite the lock

What is bundled (macOS; the Linux and Windows builds fetch their own asset
names the same way):
  love 11.5            love.app, for Run (12.0+ is downloaded on demand, signed manifest)
  lua-language-server  both darwin archives, for the universal app
  stylua               both macos archives

NOT bundled, on purpose: PortMaster's LÖVE. PortMaster recommends the
runtimes it installs itself (`love_11.5`), so the PortMaster export is a
game folder that sources them and never carries a LÖVE binary.

luacheck has no macOS binary (its release is a Linux/Windows executable),
so it is not fetched; see ROADMAP.md.

A mismatching hash stops everything: nothing is extracted from a download
that does not match the lock. Run --update, then read the lock diff in git.
"""
import hashlib, io, json, os, shutil, stat, sys, tarfile, urllib.request, zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LOCK = os.path.join(ROOT, "scripts", "toolchains.lock.json")
OUT = os.path.join(ROOT, "src-tauri", "resources", "toolchains")

# name -> (repo, tag filter, asset names to take)
WANT = {
    "love-11.5": ("love2d/love", "11.5", ["love-11.5-macos.zip"]),
    "lua-language-server": ("LuaLS/lua-language-server", None,
        ["lua-language-server-{v}-darwin-arm64.tar.gz", "lua-language-server-{v}-darwin-x64.tar.gz"]),
    "stylua": ("JohnnyMorganz/StyLua", None, ["stylua-macos-aarch64.zip", "stylua-macos-x86_64.zip"]),
}

def get(url, tries=4):
    req = urllib.request.Request(url, headers={"User-Agent": "joyride-fetch-toolchains"})
    for i in range(tries):
        try:
            with urllib.request.urlopen(req, timeout=120) as r:
                return r.read()
        except Exception as e:  # a cut connection: try again, the hash check decides
            if i == tries - 1:
                raise
            print("retrying", url, type(e).__name__)

def update():
    lock = {}
    for name, (repo, tag, assets) in WANT.items():
        rel = json.loads(get(f"https://api.github.com/repos/{repo}/releases/" + (f"tags/{tag}" if tag else "latest")))
        v = rel["tag_name"].lstrip("v")
        files = []
        for a in assets:
            a = a.format(v=v)
            hit = next(x for x in rel["assets"] if x["name"] == a)
            data = get(hit["browser_download_url"])
            digest = (hit.get("digest") or "").removeprefix("sha256:")
            sha = hashlib.sha256(data).hexdigest()
            if digest and digest != sha:
                sys.exit(f"{a}: GitHub says {digest}, downloaded {sha}")
            files.append({"name": a, "url": hit["browser_download_url"], "size": len(data), "sha256": sha,
                          "githubDigest": bool(digest)})
        lock[name] = {"repo": repo, "version": v, "files": files}
        print(name, v)
    json.dump(lock, open(LOCK, "w"), indent=2)
    open(LOCK, "a").write("\n")

def extract(name, f, data):
    # love-11.5/love.app (where runner::find_love looks), else <tool>/<arch>/.
    arch = "arm64" if ("arm64" in f["name"] or "aarch64" in f["name"]) else "x64"
    dest = os.path.join(OUT, name) if name.startswith("love-") else os.path.join(OUT, name, arch)
    os.makedirs(dest, exist_ok=True)
    if f["name"].endswith(".zip"):
        z = zipfile.ZipFile(io.BytesIO(data))
        for i in z.infolist():
            # Refuse paths that leave the folder.
            if os.path.isabs(i.filename) or ".." in i.filename.split("/"):
                sys.exit(f"{f['name']}: unsafe path {i.filename}")
        z.extractall(dest)
        # zipfile drops the executable bit and symlinks; restore both.
        for i in z.infolist():
            mode = i.external_attr >> 16
            p = os.path.join(dest, i.filename)
            if stat.S_ISLNK(mode):
                os.remove(p)
                os.symlink(z.read(i).decode(), p)
            elif mode & 0o111 and os.path.isfile(p):
                os.chmod(p, mode & 0o777)
    else:
        t = tarfile.open(fileobj=io.BytesIO(data))
        for m in t.getmembers():
            if os.path.isabs(m.name) or ".." in m.name.split("/") or m.isdev() or (m.issym() or m.islnk()) and (os.path.isabs(m.linkname) or ".." in m.linkname.split("/")):
                sys.exit(f"{f['name']}: unsafe member {m.name}")
        t.extractall(dest)

def fetch():
    lock = json.load(open(LOCK))
    shutil.rmtree(OUT, ignore_errors=True)
    os.makedirs(OUT)
    for name, ent in lock.items():
        for f in ent["files"]:
            data = get(f["url"])
            sha = hashlib.sha256(data).hexdigest()
            if sha != f["sha256"] or len(data) != f["size"]:
                sys.exit(f"{f['name']}: does not match the lock (got {sha}); nothing extracted")
            extract(name, f, data)
            print("ok", f["name"])
    open(os.path.join(OUT, "CREDITS.txt"), "w").write(
        "Bundled tools keep their own licences, listed in the repository's NOTICE.\n"
        + "".join(f"{n} {e['version']} ({e['repo']})\n" for n, e in lock.items()))

if __name__ == "__main__":
    update() if "--update" in sys.argv else fetch()
