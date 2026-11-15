#!/usr/bin/env python3
"""Reads JoyrIDE crash reports and groups them (docs/recovery.md).

    python3 -I scripts/ingest-crash.py <report.json | folder> [more]   # table of faults, most frequent first
    python3 -I scripts/ingest-crash.py --check <report.json | folder>  # fail on a report that is malformed or has an unknown schema
    python3 -I scripts/ingest-crash.py --show <signature> <paths>      # print one fault's newest report in full

A report is one JSON file the app wrote into its `crash-reports` folder (Settings > Diagnostics > Show crash reports). They are never sent
anywhere by JoyrIDE; a person attaches one to an issue and this reads them. Standard library only.
"""
import json
import sys
from collections import defaultdict
from pathlib import Path

SCHEMA = 1
KINDS = {"panic", "frontend-error", "unclean-exit"}
REQUIRED = {"schema": int, "kind": str, "id": str, "time": str, "app": str, "version": str, "os": str, "arch": str, "summary": str, "signature": str, "logTail": list}


def collect(paths):
    files = []
    for p in map(Path, paths):
        files += sorted(p.glob("*.json")) if p.is_dir() else [p]
    return [f for f in files if f.name != "session.json"]


def problems(r):
    if not isinstance(r, dict):
        return ["not an object"]
    out = []
    for k, t in REQUIRED.items():
        if k not in r:
            out.append(f"missing `{k}`")
        elif not isinstance(r[k], t):
            out.append(f"`{k}` is not a {t.__name__}")
    if r.get("schema") != SCHEMA:
        out.append(f"schema {r.get('schema')!r}, this reads {SCHEMA}")
    if r.get("kind") not in KINDS:
        out.append(f"unknown kind {r.get('kind')!r}")
    return out


def load(paths):
    good, bad = [], []
    for f in collect(paths):
        try:
            r = json.loads(f.read_text(encoding="utf-8"))
        except (OSError, ValueError) as e:
            bad.append((f, [str(e)]))
            continue
        errs = problems(r)
        (bad if errs else good).append((f, errs) if errs else (f, r))
    return good, bad


def main(argv):
    check = "--check" in argv
    show = None
    if "--show" in argv:
        i = argv.index("--show")
        show = argv[i + 1]
        argv = argv[:i] + argv[i + 2:]
    paths = [a for a in argv if not a.startswith("--")]
    if not paths:
        print(__doc__)
        return 2
    good, bad = load(paths)
    for f, errs in bad:
        print(f"{f}: {'; '.join(errs)}", file=sys.stderr)
    if check:
        print(f"{len(good)} report(s) ok, {len(bad)} bad")
        return 1 if bad else 0
    groups = defaultdict(list)
    for f, r in good:
        groups[r["signature"]].append((f, r))
    if show:
        hits = sorted(groups.get(show, []), key=lambda x: x[1]["time"])
        if not hits:
            print(f"no report with signature {show}", file=sys.stderr)
            return 1
        print(json.dumps(hits[-1][1], indent=2, ensure_ascii=False))
        return 0
    print(f"{'count':>5}  {'signature':12}  {'kind':14}  {'versions':14}  {'os':8}  newest / summary")
    for sig, items in sorted(groups.items(), key=lambda kv: (-len(kv[1]), kv[0])):
        r = max((x[1] for x in items), key=lambda r: r["time"])
        versions = ",".join(sorted({x[1]["version"] for x in items}))
        oses = ",".join(sorted({x[1]["os"] for x in items}))
        print(f"{len(items):>5}  {sig:12}  {r['kind']:14}  {versions:14.14}  {oses:8.8}  {r['time']}  {r['summary'][:70]}")
        if r.get("location"):
            print(f"{'':>5}  {'':12}  at {r['location']}")
    print(f"\n{len(good)} report(s), {len(groups)} distinct fault(s), {len(bad)} unreadable")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
