#!/usr/bin/env python3
"""Ingests Benchmark Libraries reports into JoyrIDE's library data.

    python3 -I scripts/ingest-benchmark.py <benchmark-libraries-*.txt> [more reports]   # merge them in
    python3 -I scripts/ingest-benchmark.py --skip a,b <report>                          # leave out libraries changed since the run
    python3 -I scripts/ingest-benchmark.py --check                                      # fail if the data is stale or malformed

The reports are what `love samples/benchmark-libraries [--report]` writes (the text one, `benchmark-libraries-<platform>-<LÖVE>.txt`).
For every curated library this keeps, per platform and LÖVE version it was measured on:

  * whether it loaded (`loads_11_5`, `loads_12_0`: `loads`, `needs-display`, or `fails: why`), with load time, memory kept and lines,
  * its score and place in every comparison it is in (out of 5, best or not, how many it was compared with),
  * its findings: what it got wrong (`FAIL`), what it lacks that a library it was compared with has,
  * for a library with nothing to compare against: whether its own check passed.

and writes one line of that for people (`benchmark`), which the Libraries panel shows next to the library. The data lives in
`joyride-core/data/library-benchmarks.json` (committed; `Library::bundled()` merges it over libraries.toml). A richer result for the same
platform and LÖVE version (a run with a window over one without) is never replaced by a poorer one; a newer one of the same kind is.

Run this whenever new benchmark results arrive, then commit the data with the reports (see CLAUDE.md, rule 15).
"""
import json, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA = os.path.join(ROOT, "joyride-core", "data", "library-benchmarks.json")
LIBS = os.path.join(ROOT, "joyride-core", "data", "libraries.toml")


def curated():
    names = []
    for line in open(LIBS, encoding="utf-8"):
        m = re.match(r'name = "(.*)"$', line.strip())
        if m:
            names.append(m.group(1))
    return names


def parse(path):
    """One report -> {'run': {...}, 'libs': {name: {...}}}."""
    text = open(path, encoding="utf-8").read()
    if "# Benchmark Libraries report" in text:
        text = text[text.index("# Benchmark Libraries report"):]   # LÖVE may print warnings ahead of it
    head = {}
    for m in re.finditer(r"^# (made|platform|runtime|version): (.*)$", text, re.M):
        head[m.group(1)] = m.group(2)
    if "runtime" not in head or "Benchmark Libraries report" not in text.split("\n", 1)[0]:
        raise SystemExit(f"{path}: not a Benchmark Libraries report")
    love = re.search(r"LÖVE (\d+)\.(\d+)", head["runtime"])
    platform = re.sub(r"\s*\(.*$", "", head.get("platform", "?")).strip()
    window = bool(re.search(r"^INFO window: yes", text, re.M))
    run = {"platform": platform, "love": f"{love.group(1)}.{love.group(2)}", "date": head.get("made", "")[:10], "window": window}
    libs = {}

    def lib(name):
        return libs.setdefault(name, {"groups": {}, "findings": [], "lacks": []})

    section = None
    members = {}  # group -> member id -> library
    lines = text.split("\n")
    # which library is each comparison member: read from the benchmark's own group files (older reports do not say)
    groups_dir = os.path.join(ROOT, "samples", "benchmark-libraries", "bench", "groups")
    for fn in os.listdir(groups_dir):
        src = open(os.path.join(groups_dir, fn), encoding="utf-8").read()
        found = re.findall(r'\{ id = "([^"]+)", short = "[^"]*", lib = "([^"]+)"', src)
        for mid, lname in found:
            members.setdefault(fn[:-4], {})[mid] = (lname, len(found))
    for l in lines:
        m = re.match(r"== (.*) ==$", l)
        if m:
            section = m.group(1)
            continue
        if section == "libraries":
            m = re.match(r"(PASS|FAIL|INFO|SKIP) (\S+): (.*)$", l)
            if not m:
                continue
            st, name, rest = m.groups()
            d = lib(name)
            if st == "PASS":
                mm = re.search(r"; ([\d.]+) ms, ([\d.]+) KB kept", rest)
                ln = re.search(r"; (\d+) lines", rest)
                d["loads"] = "loads"
                if mm:
                    d["load_ms"], d["kept_kb"] = float(mm.group(1)), float(mm.group(2))
                if ln:
                    d["lines"] = int(ln.group(1))
            elif st in ("INFO", "FAIL") and "did not load" in rest:
                known = re.search(r"known on this LÖVE: (.*?); did not load", rest)
                why = known.group(1) if known else rest.split("did not load: ", 1)[1]
                d["loads"] = "fails: " + why.replace('"', "'")[:160]
            elif st == "SKIP" and "needs a window" in rest:
                d["loads"] = "needs-display"
        elif section and section.startswith("compare "):
            g = section.split(" ", 1)[1]
            m = re.match(r"INFO (\w+) overall (\S+): (.*?)(?:; library \S+; of \d+)?$", l)
            if m and m.group(1) == g:
                _, member, what = m.groups()
                name, n = members[g][member]
                sc = re.match(r"([\d.]+) out of 5( \(best\))?", what)
                d = lib(name)
                d["groups"][f"{g}:{member}"] = {"group": g, "member": member, "of": int(n), "score": float(sc.group(1)) if sc else None, "best": bool(sc and sc.group(2))}
                continue
            m = re.match(r"(PASS|FAIL|INFO|SKIP) (\w+)/([\w-]+) (\S+): (.*)$", l)
            if m and m.group(2) == g:
                st, _, row, member, rest = m.groups()
                if member not in members.get(g, {}) or row.startswith("cost-"):
                    continue
                name = members[g][member][0]
                d = lib(name)
                if rest.startswith("FAIL"):
                    d["findings"].append(f"{g}/{row}: {rest.split(': rating')[0][6:].strip()}"[:200])
                elif rest.startswith("no such feature"):
                    d["lacks"].append(f"{g}/{row}")
        elif section == "specialists":
            m = re.match(r"(PASS|FAIL|INFO|SKIP) (\S+): (.*?)(?:, [\d.]+ ms)?$", l)
            if m:
                st, name, rest = m.groups()
                d = lib(name)
                job = rest.split(": ", 1)[0]
                d["own"] = {"PASS": "works", "SKIP": "not tried", "INFO": "finding", "FAIL": "finding"}[st]
                d["job"] = job
                if st in ("INFO", "FAIL"):
                    d["findings"].append(f"own check: {rest.split(': ', 1)[-1]}"[:200])
    return {"run": run, "libs": libs}


def richer(new, old):
    """Whether a run replaces another of the same platform and LÖVE version."""
    if new["window"] != old["window"]:
        return new["window"]
    return new["date"] >= old["date"]


def line(name, results):
    """The one line people read."""
    parts = []
    for key in sorted(results):
        r = results[key]
        run = r["run"]
        bits = []
        for gm in sorted(r["groups"].values(), key=lambda x: x["group"]):
            if gm["score"] is None:
                continue
            of = f" of {gm['of']}" if gm["of"] > 1 else ""
            label = gm["group"] + (f" ({gm['member']})" if gm["member"] != name else "")
            bits.append(f"{label} {gm['score']:.1f}/5{' best' + of if gm['best'] else ' (' + str(gm['of']) + ' compared)'}")
        if "own" in r and not r["groups"]:
            bits.append(f"{r.get('job', 'its job')}: {r['own']}")
        if "load_ms" in r:
            bits.append(f"loads in {r['load_ms']:.2f} ms, keeps {r['kept_kb']:.1f} KB" + (f", {r['lines']} lines" if r.get("lines") else ""))
        elif r.get("loads", "").startswith("fails"):
            bits.append("does not load: " + r["loads"][7:])
        bad = r["findings"][:2]
        if bad:
            bits.append("findings: " + "; ".join(bad))
        if r["lacks"]:
            bits.append("lacks: " + ", ".join(r["lacks"][:4]) + ("…" if len(r["lacks"]) > 4 else ""))
        parts.append(f"{run['platform']}, LÖVE {run['love']}{', window' if run['window'] else ''} ({run['date']}): " + "; ".join(bits))
    return " | ".join(parts)


def merge(files, skip=()):
    data = json.load(open(DATA, encoding="utf-8")) if os.path.exists(DATA) else {"libraries": {}}
    names = set(curated())
    for f in files:
        rep = parse(f)
        run = rep["run"]
        key = f"{run['platform']}|{run['love']}"
        for name, d in rep["libs"].items():
            if name in skip:
                continue   # the library's behaviour in this project changed after the report was made
            if name not in names:
                print(f"{f}: `{name}` is not a curated library (ignored)", file=sys.stderr)
                continue
            lib = data["libraries"].setdefault(name, {"results": {}})
            old = lib["results"].get(key)
            if old and not richer(run, old["run"]):
                continue
            entry = dict(d)
            entry["run"] = run
            lib["results"][key] = entry
        print(f"ingested {f}: {run['platform']}, LÖVE {run['love']}, {'window' if run['window'] else 'no window'}, {len(rep['libs'])} libraries")
    for name, lib in data["libraries"].items():
        lib["benchmark"] = line(name, lib["results"])
        # What loading looked like on each LÖVE: the best-evidenced result (a window beats none; a measured result beats "not tried").
        for ver in ("11.5", "12.0"):
            best = None
            for r in lib["results"].values():
                if r["run"]["love"] == ver and r.get("loads"):
                    if best is None or (r["run"]["window"], r["run"]["date"]) > (best[0], best[1]):
                        best = (r["run"]["window"], r["run"]["date"], r["loads"])
            if best:
                lib["loads_" + ver.replace(".", "_")] = best[2]
    json.dump(data, open(DATA, "w", encoding="utf-8"), indent=1, ensure_ascii=False, sort_keys=True)
    open(DATA, "a").write("\n")
    return data


def check():
    if not os.path.exists(DATA):
        raise SystemExit("joyride-core/data/library-benchmarks.json is missing: ingest a report")
    data = json.load(open(DATA, encoding="utf-8"))
    names = set(curated())
    bad = [n for n in data["libraries"] if n not in names]
    missing = [n for n in names if n not in data["libraries"]]
    if bad or missing:
        raise SystemExit(f"benchmark data and libraries.toml disagree: unknown {bad}, not benchmarked {missing}")
    print(f"ok: {len(names)} libraries benchmarked")


if __name__ == "__main__":
    if sys.argv[1:] == ["--check"]:
        check()
    elif len(sys.argv) > 1:
        argv, skip = sys.argv[1:], ()
        if argv[0] == "--skip":
            skip, argv = tuple(argv[1].split(",")), argv[2:]
        d = merge(argv, skip)
        print(f"{len(d['libraries'])} libraries in {os.path.relpath(DATA, ROOT)}")
    else:
        print(__doc__)
        sys.exit(2)
