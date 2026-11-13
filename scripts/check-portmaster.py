#!/usr/bin/env python3
"""Checks what JoyrIDE generates for PortMaster against PortMaster's own repository.

PortMaster changes (runtimes, launch-script helpers, port.json versions), and
nobody can run the output on every handheld. So this reads the real LÖVE ports
in PortMaster-New, runs the exporter (joyride-core's `export_samples` example)
and compares what it makes with what those ports actually do: the runtimes they
source, the helpers and variables their launch scripts use, port.json's version
and fields. Exit 1 means drift: fix export.rs and its tests, then rerun. Run it
before touching the PortMaster exporter and on the weekly workflow
(.github/workflows/portmaster-check.yml).

  scripts/check-portmaster.py                  # clone PortMaster-New (needs network)
  scripts/check-portmaster.py --repo-dir DIR   # an existing checkout
  scripts/check-portmaster.py --samples FILE   # exporter output as JSON, instead of running cargo
  scripts/check-portmaster.py --json           # machine-readable report

PORTMASTER_REPO overrides the clone URL (default: PortsMaster/PortMaster-New).
Only launch scripts and port.json files are fetched (the repo holds gigabytes
of game files): a shallow, blob-less clone with a sparse checkout.
"""
import argparse, json, os, re, subprocess, sys, tempfile
from collections import Counter
from pathlib import Path

REPO = os.environ.get("PORTMASTER_REPO", "https://github.com/PortsMaster/PortMaster-New")
ROOT = Path(__file__).resolve().parent.parent

RUNTIME_RE = re.compile(r'runtimes/"?(love_[0-9][0-9.]*)"?/')

# (name, regex): conventions a LÖVE launch script can have. Ours must use the
# ones most real ports use, and not use one almost no port does.
CONVENTIONS = [
    ("sources control.txt", r'source\s+"?\$controlfolder"?/control\.txt'),
    ("calls get_controls", r"^\s*get_controls\b"),
    ("sources the love runtime", r'source\s+"?\$controlfolder"?/runtimes/"?love_[0-9.]+"?/love\.txt'),
    ("runs $LOVE_RUN", r"\$LOVE_RUN\b"),
    ('uses `$GPTOKEYB "$LOVE_GPTK"`', r'\$GPTOKEYB\s+"?\$LOVE_GPTK"?'),
    ("calls pm_finish", r"^\s*pm_finish\b"),
    ("calls pm_platform_helper", r"pm_platform_helper\b"),
    ("sets GAMEDIR from $directory", r'GAMEDIR="?/\$directory/ports/'),
]

def love_ports(repo: Path):
    ports = repo / "ports"
    if not ports.is_dir():
        sys.exit(f"{repo} has no ports/ folder: not a PortMaster-New checkout")
    for d in sorted(p for p in ports.iterdir() if p.is_dir()):
        for sh in d.glob("*.sh"):
            try:
                text = sh.read_text(errors="replace")
            except OSError:
                continue
            if RUNTIME_RE.search(text) or "$LOVE_RUN" in text:
                yield d, text

def observe(repo: Path):
    runtimes, versions, keys, kinds = Counter(), Counter(), Counter(), {}
    scripts, names = [], []
    for d, text in love_ports(repo):
        names.append(d.name)
        scripts.append(text)
        runtimes.update(set(RUNTIME_RE.findall(text)))
        pj = d / "port.json"
        if pj.is_file():
            try:
                j = json.loads(pj.read_text())
            except json.JSONDecodeError:
                continue
            versions[j.get("version")] += 1
            for k, v in (j.get("attr") or {}).items():
                keys[k] += 1
                kinds.setdefault(k, Counter())[type(v).__name__] += 1
    n = len(names)
    conv = {name: sum(1 for s in scripts if re.search(rx, s, re.M)) / n if n else 0 for name, rx in CONVENTIONS}
    return {"ports": n, "runtimes": dict(runtimes), "port_json_versions": {str(k): v for k, v in versions.items()},
            "attr_keys": dict(keys), "attr_types": {k: dict(v) for k, v in kinds.items()}, "conventions": conv}

def compare(obs, ours):
    drift = []
    if not obs["ports"]:
        return ["found no LÖVE ports in the repository: its layout may have changed"]
    # runtimes
    seen = set(obs["runtimes"])
    allowed = {f"love_{v}" for v in ours["love_versions"]}
    for r in sorted(allowed - seen):
        drift.append(f"export.rs allows {r}, but no LÖVE port sources it")
    for r in sorted(seen - allowed):
        drift.append(f"PortMaster ports use {r}, which export.rs does not know (PORTMASTER_LOVE_VERSIONS)")
    # port.json
    pj = ours["port_json"]
    vers = [int(v) for v in obs["port_json_versions"] if v.lstrip("-").isdigit()]
    if vers and pj.get("version") != max(vers):
        drift.append(f"port.json version: export.rs writes {pj.get('version')}, the newest in use is {max(vers)}")
    attr, n = pj.get("attr", {}), obs["ports"]
    for k, c in obs["attr_keys"].items():
        if c / n >= 0.9 and k not in attr:
            drift.append(f"port.json attr.{k} is in {c} of {n} ports; export.rs omits it")
    for k in attr:
        if k not in obs["attr_keys"]:
            drift.append(f"port.json attr.{k} appears in no LÖVE port")
    for k, kinds in obs["attr_types"].items():
        top = max(kinds, key=kinds.get)
        if k in attr and kinds[top] / sum(kinds.values()) >= 0.9:
            ours_t = type(attr[k]).__name__
            if ours_t != top and not (attr[k] is None and top == "NoneType"):
                drift.append(f"port.json attr.{k}: ports use {top}, export.rs writes {ours_t}")
    # launch script
    script = ours["launch_script"]
    for name, rx in CONVENTIONS:
        frac = obs["conventions"][name]
        mine = bool(re.search(rx, script, re.M))
        if frac >= 0.5 and not mine:
            drift.append(f"launch script lacks `{name}` ({frac:.0%} of LÖVE ports have it)")
        if frac < 0.05 and mine:
            drift.append(f"launch script has `{name}`, which only {frac:.0%} of LÖVE ports use")
    return drift

def our_samples(path):
    if path:
        return json.loads(Path(path).read_text())
    env = dict(os.environ)
    r = subprocess.run(["cargo", "run", "-q", "--manifest-path", str(ROOT / "joyride-core/Cargo.toml"), "--example", "export_samples"],
                       capture_output=True, text=True, env=env)
    if r.returncode:
        sys.exit(f"could not run the exporter's samples:\n{r.stderr}")
    return json.loads(r.stdout)

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo-dir")
    ap.add_argument("--samples")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()
    ours = our_samples(args.samples)
    with tempfile.TemporaryDirectory() as tmp:
        if args.repo_dir:
            repo = Path(args.repo_dir)
        else:
            repo = Path(tmp) / "pm"
            for step in (
                ["git", "clone", "--depth", "1", "--filter=blob:none", "--no-checkout", "--quiet", REPO, str(repo)],
                ["git", "-C", str(repo), "sparse-checkout", "set", "--no-cone", "/ports/*/*.sh", "/ports/*/port.json"],
                ["git", "-C", str(repo), "checkout", "--quiet"],
            ):
                r = subprocess.run(step, capture_output=True, text=True)
                if r.returncode:
                    sys.exit(f"{' '.join(step[:4])} failed: {r.stderr.strip()}\n(needs network; or pass --repo-dir)")
        sha = subprocess.run(["git", "-C", str(repo), "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip() or "unknown"
        obs = observe(repo)
    drift = compare(obs, ours)
    if args.json:
        print(json.dumps({"commit": sha, "observed": obs, "drift": drift}, indent=2))
    else:
        print(f"PortMaster-New @ {sha}: {obs['ports']} LÖVE ports")
        print(f"  runtimes: {obs['runtimes']}")
        print(f"  port.json versions: {obs['port_json_versions']}")
        print("  launch-script conventions (share of ports):")
        for k, v in obs["conventions"].items():
            print(f"    {v:4.0%}  {k}")
        print("DRIFT:" if drift else "OK: the exporter's output matches PortMaster's LÖVE ports.")
        for d in drift:
            print("  -", d)
    sys.exit(1 if drift else 0)

if __name__ == "__main__":
    main()
