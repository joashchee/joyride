import copy, importlib.util, json, tempfile, unittest
from pathlib import Path
spec = importlib.util.spec_from_file_location("cp", Path(__file__).parent / "check-portmaster.py")
cp = importlib.util.module_from_spec(spec); spec.loader.exec_module(cp)

SCRIPT = '''#!/bin/bash
source $controlfolder/control.txt
get_controls
GAMEDIR=/$directory/ports/x
source $controlfolder/runtimes/"love_11.5"/love.txt
$GPTOKEYB "$LOVE_GPTK" -c "./x.gptk" &
pm_platform_helper "$LOVE_BINARY"
$LOVE_RUN "$GAMEDIR/gamedata/game.love"
pm_finish
'''
ATTR = {"title": "x", "porter": ["p"], "desc": "d", "desc_md": "", "inst": "i", "inst_md": "", "genres": [], "image": None,
        "rtr": True, "exp": False, "runtime": [], "store": [], "availability": "full", "reqs": [], "arch": [], "min_glibc": ""}

def fake_repo(d, n=3, script=SCRIPT, version=4, attr=None):
    for i in range(n):
        p = Path(d) / "ports" / f"game{i}"
        p.mkdir(parents=True)
        (p / f"Game {i}.sh").write_text(script)
        (p / "port.json").write_text(json.dumps({"version": version, "attr": attr or ATTR}))
    (Path(d) / "ports" / "native").mkdir()
    (Path(d) / "ports" / "native" / "Native.sh").write_text("#!/bin/bash\n./game\n")
    return Path(d)

OURS = {"launch_script": SCRIPT, "port_json": {"version": 4, "attr": ATTR}, "love_versions": ["11.5"]}

class Check(unittest.TestCase):
    def drift(self, ours=OURS, **kw):
        with tempfile.TemporaryDirectory() as d:
            return cp.compare(cp.observe(fake_repo(d, **kw)), ours)

    def test_finds_only_love_ports_and_their_facts(self):
        with tempfile.TemporaryDirectory() as d:
            o = cp.observe(fake_repo(d))
        self.assertEqual(o["ports"], 3)
        self.assertEqual(o["runtimes"], {"love_11.5": 3})
        self.assertEqual(o["port_json_versions"], {"4": 3})
        self.assertEqual(o["conventions"]['uses `$GPTOKEYB "$LOVE_GPTK"`'], 1.0)

    def test_agreement_is_no_drift(self):
        self.assertEqual(self.drift(), [])

    def test_a_new_runtime_or_a_dropped_one_is_drift(self):
        text = " | ".join(self.drift(script=SCRIPT.replace("love_11.5", "love_12.0")))
        self.assertIn("love_12.0, which export.rs does not know", text)
        self.assertIn("allows love_11.5, but no LÖVE port sources it", text)

    def test_a_newer_port_json_version_is_drift(self):
        self.assertIn("newest in use is 5", " | ".join(self.drift(version=5)))

    def test_missing_wrong_typed_and_invented_keys(self):
        ours = copy.deepcopy(OURS)
        del ours["port_json"]["attr"]["arch"]
        ours["port_json"]["attr"]["porter"] = "JoyrIDE"
        ours["port_json"]["attr"]["invented"] = 1
        text = " | ".join(self.drift(ours))
        for needle in ["attr.arch is in 3 of 3", "attr.porter: ports use list", "attr.invented appears in no"]:
            self.assertIn(needle, text)

    def test_conventions_we_dropped_or_nobody_uses(self):
        ours = copy.deepcopy(OURS)
        ours["launch_script"] = SCRIPT.replace("pm_finish", "").replace('$GPTOKEYB "$LOVE_GPTK"', '$GPTOKEYB2 "love"')
        text = " | ".join(self.drift(ours))
        self.assertIn("lacks `calls pm_finish`", text)
        self.assertIn('lacks `uses `$GPTOKEYB "$LOVE_GPTK"`', text)

    def test_an_empty_or_wrong_repo_is_not_a_pass(self):
        with tempfile.TemporaryDirectory() as d:
            (Path(d) / "ports").mkdir()
            self.assertIn("no LÖVE ports", cp.compare(cp.observe(Path(d)), OURS)[0])

    def test_the_real_exporter_output_has_the_expected_shape(self):
        # cargo is slow to build cold, so only run when its output is already built.
        try:
            ours = cp.our_samples(None)
        except SystemExit:
            self.skipTest("could not run the exporter")
        self.assertEqual(ours["port_json"]["version"], 4)
        self.assertIn("$LOVE_RUN", ours["launch_script"])

if __name__ == "__main__":
    unittest.main()
