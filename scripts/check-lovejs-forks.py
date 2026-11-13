#!/usr/bin/env python3
"""Surveys every fork of love.js for work Davidobot/love.js lacks.

JoyrIDE's web exporter (joyride-core/src/web.rs) packages with a love.js
template. Davidobot/love.js (LÖVE 11.5) was last touched in 2024; forks may
have newer engines. This lists the whole fork network (github.com's
network/members page; the API host is not needed), blob-less clones each fork
(commit history only: nothing from them is run), and reports the commits each
branch has that Davidobot's tip lacks, newest first, with the last commit's
message. Informational: it exits 0. Read the top entries before changing which
template JoyrIDE targets, and update docs/lovejs-forks.md.

  scripts/check-lovejs-forks.py [--days N]   # only forks active in the last N days (default: all)
"""
import json, os, re, subprocess, sys, tempfile, urllib.request
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timedelta, timezone

UPSTREAM = "Davidobot/love.js"

def git(args, cwd=None, timeout=240):
    return subprocess.run(["git", "-c", "core.hooksPath=/dev/null", "-c", "protocol.ext.allow=never"] + args, cwd=cwd,
                          capture_output=True, text=True, timeout=timeout, env=dict(os.environ, GIT_TERMINAL_PROMPT="0"))

def members():
    req = urllib.request.Request(f"https://github.com/{UPSTREAM}/network/members", headers={"User-Agent": "joyride-fork-survey"})
    html = urllib.request.urlopen(req, timeout=60).read().decode("utf-8", "replace")
    names = set(re.findall(r'href="/([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)"[^>]*>\s*(?:[A-Za-z0-9_.-]+)\s*</a>', html))
    return sorted(n for n in names if n != UPSTREAM)

def main():
    days = int(sys.argv[sys.argv.index("--days") + 1]) if "--days" in sys.argv else None
    cutoff = datetime.now(timezone.utc) - timedelta(days=days) if days else None
    with tempfile.TemporaryDirectory() as work:
        ref = f"{work}/ref"
        r = git(["clone", "--quiet", "--filter=blob:none", "--no-checkout", f"https://github.com/{UPSTREAM}", ref])
        if r.returncode:
            sys.exit(f"could not clone {UPSTREAM}: {r.stderr.strip()}")
        head = git(["rev-parse", "HEAD"], ref).stdout.strip()
        print(f"{UPSTREAM} tip {head[:8]} of {git(['log', '-1', '--format=%cs'], ref).stdout.strip()}")
        names = members()

        def scan(n):
            d = f"{work}/{n.replace('/', '__')}"
            try:
                if git(["clone", "--quiet", "--filter=blob:none", "--no-checkout", f"https://github.com/{n}", d]).returncode:
                    return None
                git(["remote", "set-branches", "origin", "*"], d)
                git(["fetch", "--quiet", "--filter=blob:none", "origin", "+refs/heads/*:refs/remotes/origin/*"], d)
                git(["fetch", "--quiet", "--filter=blob:none", ref, head], d)
                best = None
                for line in git(["for-each-ref", "--format=%(refname:short)|%(committerdate:iso-strict)", "refs/remotes/origin"], d).stdout.splitlines():
                    b, date = line.split("|")
                    if b.endswith("/HEAD"):
                        continue
                    lr = git(["rev-list", "--left-right", "--count", f"{head}...{b}"], d).stdout.split()
                    if len(lr) == 2 and int(lr[1]) > 0 and (best is None or date > best[1]):
                        subject = git(["log", "-1", "--format=%s", b], d).stdout.strip()[:80]
                        best = (b.replace("origin/", ""), date, int(lr[1]), subject)
                return (n, best) if best else None
            except Exception:
                return None

        with ThreadPoolExecutor(8) as ex:
            rows = [r for r in ex.map(scan, names) if r]
    rows.sort(key=lambda r: r[1][1], reverse=True)
    if cutoff:
        rows = [r for r in rows if datetime.fromisoformat(r[1][1]) >= cutoff]
    print(f"{len(names)} forks; {len(rows)} with commits {UPSTREAM} lacks" + (f" in the last {days} days" if days else ""))
    for n, (b, date, ahead, msg) in rows:
        print(f"  {date[:10]}  +{ahead:<3} {n}  [{b}]  {msg}")

if __name__ == "__main__":
    main()
