#!/usr/bin/env python3
"""Security and leak audit of the curated libraries, at their exact pinned commits.

    python3 -I scripts/audit-libraries.py [names...]            scan and print (default: every entry)
    python3 -I scripts/audit-libraries.py --stamp "who/what" [names...]
                                                                  also record the audit in
                                                                  joyride-core/data/library-audits.toml

Rule (CLAUDE.md): every library added to, or updated in, joyride-core/data/libraries.toml is audited
BEFORE it is probed (the probe runs its code) and before it is committed. A test refuses an entry whose
pinned commit has no audit record. `--stamp` is the person (or assistant) saying "I read the findings
and they are acceptable": give it a short note of what was reviewed.

Nothing from a repository is ever run. git runs with hooks and remote helpers off; files are only read.
Needs github.com / codeberg.org. Exit 1 when a HIGH finding is not expected for the entry's kind.
"""
import datetime
import json
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA = os.path.join(ROOT, "joyride-core", "data")
VERSION = "audit-libraries.py v1"

BINARY_EXT = {".dll", ".so", ".dylib", ".exe", ".a", ".o", ".lib", ".bin", ".jar", ".class", ".pyc", ".zip", ".7z", ".gz", ".tar", ".rar", ".apk", ".ipa", ".msi", ".dmg", ".app", ".node", ".wasm", ".luac"}
ASSET_EXT = {".png", ".jpg", ".jpeg", ".gif", ".bmp", ".ogg", ".wav", ".mp3", ".flac", ".ttf", ".otf", ".woff", ".woff2", ".ico", ".svg", ".glsl", ".obj", ".mtl", ".ase", ".aseprite", ".json", ".tmx", ".tsx"}
SKIP_DIRS = {".git"}
LUA_EXT = {".lua", ".glsl", ".rockspec"}

# (id, severity, regex, what it means)
RULES = [
    ("exec", "HIGH", r"\b(os\.execute|io\.popen|os\.exit\s*\(\s*true|package\.loadlib)\b", "runs a program or loads a native library"),
    ("ffi", "HIGH", r"\bffi\.(load|cdef)\b|require\s*\(?\s*[\"']ffi[\"']", "uses LuaJIT's FFI (native code access)"),
    ("dynamic", "MED", r"\b(loadstring|loadfile|dofile|setfenv|getfenv|string\.dump)\b|(?<![.\w:])load\s*\((?!\s*(args|arg)\b)", "builds or loads code at run time"),
    ("network", "MED", r"[\"'](socket(\.http|\.url)?|enet|ssl\.https|https?|luasocket|lua-https)[\"']|socket\.(http|tcp|udp|bind)|\.bind\s*\(\s*[\"']\*", "uses the network"),
    ("filewrite", "MED", r"\bio\.open\s*\([^)]*[\"'][wa]b?\+?[\"']|\bos\.(remove|rename|tmpname)\b|love\.filesystem\.(write|append|remove|createDirectory)|\bnewFile\s*\([^)]*[\"']w", "writes or removes files"),
    ("system", "MED", r"love\.system\.(openURL|setClipboardText)|love\.thread\.newThread|os\.getenv|os\.setlocale", "reaches the system (browser, clipboard, threads, environment)"),
    ("debuglib", "INFO", r"\bdebug\.(getinfo|sethook|getlocal|setmetatable|getregistry|setupvalue|upvaluejoin)\b", "uses the debug library"),
    ("obfuscation", "HIGH", r"string\.char\s*\((\s*\d+\s*,){12,}|(\\x[0-9a-fA-F]{2}){24,}|[A-Za-z0-9+/]{400,}={0,2}", "looks encoded or obfuscated"),
]
SECRETS = [
    ("aws key", r"AKIA[0-9A-Z]{16}"),
    ("github token", r"\b(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}"),
    ("slack token", r"xox[baprs]-[A-Za-z0-9-]{10,}"),
    ("private key", r"-----BEGIN (RSA |EC |DSA |OPENSSH |PGP )?PRIVATE KEY"),
    ("api key / secret assignment", r"(?i)\b(api[_-]?key|secret|passwd|password|auth[_-]?token|access[_-]?token)\b\s*[:=]\s*[\"'][A-Za-z0-9_\-+/=]{16,}[\"']"),
    ("bearer token", r"(?i)bearer\s+[A-Za-z0-9_\-\.]{24,}"),
    ("home path", r"/Users/[A-Za-z0-9._-]+/|/home/[A-Za-z0-9._-]+/|[A-Z]:\\Users\\[A-Za-z0-9._-]+\\"),
]
URL_RE = re.compile(r"https?://([A-Za-z0-9.-]+)[^\s\"')>\]]*")
EMAIL_RE = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[a-z]{2,}")
# Hosts that only document or credit things: not worth listing.
QUIET_HOSTS = ("github.com", "love2d.org", "wikipedia.org", "lua.org", "luarocks.org", "opensource.org", "creativecommons.org", "keepachangelog.com", "shields.io", "travis-ci", "coveralls.io", "gnu.org", "zlib.net", "w3.org", "youtube.com", "youtu.be", "imgur.com", "discord.gg", "twitter.com", "itch.io", "patreon.com", "ko-fi.com", "githubusercontent.com", "readthedocs.io", "codeberg.org")


def read_entries():
    text = open(os.path.join(DATA, "libraries.toml"), encoding="utf-8").read()
    out = []
    for block in text.split("\n[[library]]\n")[1:]:
        e = {}
        for line in block.splitlines():
            m = re.match(r'(\w+) = "(.*)"$', line)
            if m:
                e[m.group(1)] = m.group(2).replace('\\"', '"')
        out.append(e)
    return out


def git(args, cwd=None):
    env = dict(os.environ, GIT_TERMINAL_PROMPT="0")
    r = subprocess.run(["git", "-c", "protocol.ext.allow=never", "-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false", *args], cwd=cwd, env=env, capture_output=True, text=True)
    if r.returncode:
        raise RuntimeError(r.stderr.strip())
    return r.stdout


def fetch(e, dest):
    git(["clone", "--quiet", "--no-checkout", e["git"], dest])
    git(["checkout", "--quiet", "--detach", e["rev"]], cwd=dest)
    head = git(["rev-parse", "HEAD"], cwd=dest).strip()
    if head != e["rev"]:
        raise RuntimeError(f"checked out {head}, expected {e['rev']}")


def is_binary(path):
    try:
        with open(path, "rb") as f:
            return b"\0" in f.read(8192)
    except OSError:
        return False


def scan(e, repo):
    """Returns (findings, notes): findings are (severity, rule, file, line number, text)."""
    findings, notes = [], []
    kind = e.get("kind", "lua")
    files = 0
    lua_bytes = 0
    licences = {}
    if os.path.exists(os.path.join(repo, ".gitmodules")):
        # JoyrIDE's installer clones and checks out only (no submodule init: a test in library.rs guards that),
        # so submodules are never fetched. Listed so the person knows the repository has them.
        findings.append(("INFO", "submodule", ".gitmodules", 0, "has git submodules (not fetched by JoyrIDE's installer)"))
    for dirpath, dirs, names in os.walk(repo):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for n in names:
            path = os.path.join(dirpath, n)
            rel = os.path.relpath(path, repo)
            if os.path.islink(path):
                findings.append(("HIGH", "symlink", rel, 0, "is a symbolic link -> " + os.readlink(path)))
                continue
            files += 1
            ext = os.path.splitext(n)[1].lower()
            if ext in BINARY_EXT or (ext not in ASSET_EXT and ext not in LUA_EXT and ext not in {".md", ".txt", ".rst", ".yml", ".yaml", ".toml", ".ld", ".cfg", ".html", ".css", ".js", ".sh", ".bat", ".ps1", ""} and is_binary(path)):
                findings.append(("HIGH", "binary", rel, 0, "compiled or archive file (%d bytes)" % os.path.getsize(path)))
                continue
            if ext in ASSET_EXT - {".json", ".svg", ".glsl", ".tmx", ".tsx", ".obj", ".mtl"}:
                notes.append(f"asset: {rel}")
                continue
            if ext in {".sh", ".bat", ".ps1"} or n in {"Makefile", "install", "setup.py"}:
                findings.append(("MED", "script", rel, 0, "build or install script (not run by JoyrIDE)"))
            try:
                text = open(path, encoding="utf-8", errors="replace").read()
            except OSError:
                continue
            is_lua = ext in LUA_EXT
            if is_lua:
                lua_bytes += len(text)
            for i, line in enumerate(text.splitlines(), 1):
                if len(line) > 1500 and is_lua:
                    findings.append(("HIGH", "obfuscation", rel, i, f"a {len(line)}-character line (minified or encoded?)"))
                if is_lua:
                    stripped = re.sub(r"^\s*--.*$", "", line)
                    for rid, sev, rx, what in RULES:
                        if re.search(rx, stripped):
                            findings.append((sev, rid, rel, i, line.strip()[:140]))
                for what, rx in SECRETS:
                    if re.search(rx, line):
                        # A path inside a doc comment is not a leak by itself, but a person's home path is personal data.
                        findings.append(("HIGH", "secret/personal", rel, i, f"{what}: {line.strip()[:100]}"))
                for m in URL_RE.finditer(line):
                    host = m.group(1).lower()
                    if not host.endswith(QUIET_HOSTS):
                        findings.append(("INFO", "url", rel, i, m.group(0)[:120]))
            lic = re.findall(r"(?i)\b(GPL|LGPL|AGPL|MPL|Mozilla Public|Creative Commons|CC[- ]BY(?:-SA|-NC)?|Apache License|BSD|MIT License|zlib|ISC License|Public Domain|WTFPL|Unlicense)\b", text[:4000]) if is_lua or n.upper().startswith(("LICENSE", "LICENCE", "COPYING")) else []
            for l in lic:
                licences.setdefault(l.upper().replace("MIT LICENSE", "MIT"), set()).add(rel)
            for m in EMAIL_RE.finditer(text) if not n.upper().startswith(("LICENSE", "LICENCE")) else []:
                if "noreply" not in m.group(0) and not m.group(0).endswith(("@example.com", "@localhost")):
                    findings.append(("INFO", "email", rel, text[:m.start()].count("\n") + 1, m.group(0)))
    declared = e.get("licence", "").upper()
    odd = {k: sorted(v)[:3] for k, v in licences.items() if k.split("-")[0] not in declared and declared not in k and k not in ("PUBLIC DOMAIN",)}
    for k, v in odd.items():
        findings.append(("MED", "licence", ", ".join(v), 0, f"mentions {k}, but the entry declares {e.get('licence')}: check that file's own licence"))
    # What a C library is expected to do is not a finding: ffi is the point of an ffi entry.
    out = []
    for f in findings:
        if f[1] in ("ffi", "exec") and kind in ("ffi", "native"):
            f = ("INFO",) + f[1:]
        out.append(f)
    notes.append(f"{files} files, {lua_bytes // 1024} KiB of Lua")
    return out, notes


def report(e, findings, notes, limit=4):
    kind = e.get("kind", "lua")
    c = {s: sum(1 for f in findings if f[0] == s) for s in ("HIGH", "MED", "INFO")}
    print(f"\n=== {e['name']} @ {e['rev'][:8]} ({e.get('licence')}, {kind})  HIGH {c['HIGH']}  MED {c['MED']}  INFO {c['INFO']}")
    print("    " + "; ".join(n for n in notes if not n.startswith("asset:")))
    assets = [n for n in notes if n.startswith("asset:")]
    if assets:
        print(f"    {len(assets)} asset file(s), e.g. {assets[0][7:]}")
    by = {}
    for f in findings:
        by.setdefault((f[0], f[1]), []).append(f)
    for (sev, rid), items in sorted(by.items(), key=lambda kv: ("HIGH MED INFO".split().index(kv[0][0]), kv[0][1])):
        print(f"  {sev:4} {rid}: {len(items)}")
        for f in items[:limit]:
            loc = f"{f[2]}:{f[3]}" if f[3] else f[2]
            print(f"       {loc}  {f[4]}")
        if len(items) > limit:
            print(f"       … {len(items) - limit} more")
    return c


def stamp(records, note):
    path = os.path.join(DATA, "library-audits.toml")
    existing = open(path, encoding="utf-8").read() if os.path.exists(path) else "# Written by scripts/audit-libraries.py --stamp. One record per audited pinned commit.\n# A test refuses a library whose pinned commit has no record here.\n"
    today = datetime.date.today().isoformat()
    for name, rev, c in records:
        # json.dumps gives a valid TOML basic string (quotes and backslashes escaped, non-ASCII kept).
        existing += f'\n[[audit]]\nname = "{name}"\nrev = "{rev}"\ndate = "{today}"\nscanner = "{VERSION}"\nhigh = {c["HIGH"]}\nmed = {c["MED"]}\nreviewed = {json.dumps(note, ensure_ascii=False)}\n'
    open(path, "w", encoding="utf-8").write(existing)


def main():
    args = sys.argv[1:]
    note = None
    if "--stamp" in args:
        i = args.index("--stamp")
        note = args[i + 1]
        del args[i:i + 2]
    entries = read_entries()
    if args:
        want = {a.lower() for a in args}
        entries = [e for e in entries if e["name"].lower() in want]
        missing = want - {e["name"].lower() for e in entries}
        if missing:
            sys.exit("not in libraries.toml: " + ", ".join(sorted(missing)))
    bad, records = 0, []
    with tempfile.TemporaryDirectory() as work:
        for e in entries:
            dest = os.path.join(work, e["name"])
            try:
                fetch(e, dest)
            except RuntimeError as err:
                print(f"\n=== {e['name']}: could not fetch the pinned commit: {err}")
                bad += 1
                continue
            findings, notes = scan(e, dest)
            c = report(e, findings, notes)
            records.append((e["name"], e["rev"], c))
            bad += c["HIGH"] > 0
    print(f"\n{len(records)} audited; {bad} with HIGH findings or a fetch failure.")
    if note:
        stamp(records, note)
        print("recorded in joyride-core/data/library-audits.toml")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
