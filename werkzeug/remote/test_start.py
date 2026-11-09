"""Exercise startup path selection without building dependencies or listening."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


START = Path(__file__).with_name("start.sh").resolve()


class StartPathsTest(unittest.TestCase):
    def test_cross_target_environment_still_launches_the_new_native_build(self):
        with tempfile.TemporaryDirectory(prefix="orchester-start-") as folder:
            root = Path(folder)
            commands = root / "bin"
            commands.mkdir()
            (commands / "rustc").write_text("#!/bin/sh\nprintf 'host: native-test-host\\n'\n")
            (commands / "pnpm").write_text("#!/bin/sh\nexit 0\n")
            (commands / "cargo").write_text('''#!/usr/bin/env python3
import json, os, pathlib, sys
args = sys.argv[1:]
pathlib.Path(os.environ["QA_CARGO_ARGS"]).write_text(json.dumps(args))
target = args[args.index("--target") + 1]
directory = pathlib.Path(args[args.index("--target-dir") + 1])
binary = directory / target / "debug" / "orchester"
binary.parent.mkdir(parents=True)
binary.write_text("#!/usr/bin/env python3\\nimport json,os,sys\\nprint(json.dumps({'args':sys.argv[1:],'state':os.environ['ORCHESTER_HOME']}))\\n")
binary.chmod(0o755)
''')
            for command in commands.iterdir():
                command.chmod(0o755)
            env = dict(os.environ, PATH=str(commands) + os.pathsep + os.environ["PATH"],
                       CARGO_TARGET_DIR="relative build", CARGO_BUILD_TARGET="wrong-cross-target",
                       ORCHESTER_HOME=str(root / "state"), QA_CARGO_ARGS=str(root / "cargo.json"))
            result = subprocess.run(["bash", str(START), "--workspace", str(root), "--port", "7337"],
                                    cwd=root, env=env, capture_output=True, text=True, check=True)
            built = json.loads((root / "cargo.json").read_text())
            self.assertEqual(built[built.index("--target") + 1], "native-test-host")
            self.assertEqual(built[built.index("--target-dir") + 1], str(root / "relative build"))
            launched = json.loads(result.stdout)
            self.assertEqual(launched["args"][-4:], ["--workspace", str(root), "--port", "7337"])
            self.assertEqual(launched["state"], str(root / "state"))


if __name__ == "__main__":
    unittest.main()
