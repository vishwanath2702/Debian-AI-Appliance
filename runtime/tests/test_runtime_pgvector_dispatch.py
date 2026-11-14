"""Execute the real DAIA runtime with isolated inert dependencies."""

from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[1] / "runtime.sh"


class RuntimePgvectorDispatchTests(unittest.TestCase):
    def execute(self, enabled=True, failure="none"):
        with tempfile.TemporaryDirectory(prefix="daia-dispatch-") as tmp:
            base = Path(tmp)
            for directory in ("config", "lib", "modules"):
                (base / directory).mkdir()

            shutil.copyfile(SOURCE, base / "runtime.sh")

            flags = (
                "DAIA_DESKTOP_ENABLED=false\n"
                "DAIA_ENABLE_DOCKER=false\n"
                "DAIA_ENABLE_OLLAMA=false\n"
                "DAIA_ENABLE_TIKA=false\n"
                "DAIA_ENABLE_OPEN_WEBUI=false\n"
                f"DAIA_ENABLE_PGVECTOR={'true' if enabled else 'false'}\n"
            )
            (base / "config/pragna.conf").write_text(flags)

            for name in ("common", "logging", "packages"):
                (base / "lib" / f"{name}.sh").write_text(
                    "# inert test library\n"
                )

            lifecycle = r"""
module_load() {
    printf 'load:%s\n' "$1"
    [[ "$DAIA_TEST_FAILURE" != load ]]
}

module_operation_run() {
    printf '%s:%s\n' "$1" "$2"
    [[ "$DAIA_TEST_FAILURE" != "$2" ]]
}
"""
            (base / "lib/lifecycle.sh").write_text(lifecycle)

            env = os.environ.copy()
            env["DAIA_TEST_FAILURE"] = failure
            return subprocess.run(
                ["bash", str(base / "runtime.sh")],
                env=env, capture_output=True, text=True,
                timeout=10, check=False,
            )

    def test_pgvector_enabled(self):
        result = self.execute()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout.splitlines(),
            ["load:pgvector", "pgvector:validate",
             "pgvector:install", "pgvector:verify"],
        )

    def test_pgvector_disabled(self):
        result = self.execute(enabled=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "")

    def test_module_load_failure(self):
        result = self.execute(failure="load")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout.splitlines(), ["load:pgvector"])

    def test_validation_failure(self):
        result = self.execute(failure="validate")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            result.stdout.splitlines(),
            ["load:pgvector", "pgvector:validate"],
        )

    def test_installation_failure(self):
        result = self.execute(failure="install")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            result.stdout.splitlines(),
            ["load:pgvector", "pgvector:validate",
             "pgvector:install"],
        )

    def test_verification_failure(self):
        result = self.execute(failure="verify")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            result.stdout.splitlines(),
            ["load:pgvector", "pgvector:validate",
             "pgvector:install", "pgvector:verify"],
        )


if __name__ == "__main__":
    unittest.main()
