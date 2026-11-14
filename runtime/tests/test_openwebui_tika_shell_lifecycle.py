import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MODULE = ROOT / "modules/openwebui.sh"

WEBUI = "a" * 64
TIKA = "b" * 64
OTHER = "c" * 64


def extract(name, source):
    pattern = re.compile(
        rf"(?ms)^{re.escape(name)}\(\) \{{\n.*?^\}}\n"
    )
    matches = pattern.findall(source)
    if len(matches) != 1:
        raise AssertionError("Function missing or duplicated: " + name)
    return matches[0]


class TikaShellLifecycleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)

        self.folder = Path(self.temporary.name)
        self.log = self.folder / "calls.log"
        self.log.write_text("", encoding="utf-8")

        self.helper = self.folder / "webui-helper"
        self.tika_helper = self.folder / "tika-helper"

        self.helper.write_text(
            '#!/usr/bin/env bash\n'
            'printf "webui %s\\n" "$*" >> "$MOCK_LOG"\n'
            'case "$1" in\n'
            '  --authorize-start|--verify-identity)\n'
            '    printf "%s\\n" "$WEBUI_ID" ;;\n'
            '  --verify-running)\n'
            '    if [[ "${MOCK_FAIL_RUNNING:-0}" == 1 ]]; then\n'
            '      exit 1\n'
            '    fi\n'
            '    printf "%s\\n" "$WEBUI_ID" ;;\n'
            '  *) exit 2 ;;\n'
            'esac\n',
            encoding="utf-8",
        )

        self.tika_helper.write_text(
            '#!/usr/bin/env bash\n'
            'printf "tika %s\\n" "$*" >> "$MOCK_LOG"\n'
            'if [[ "$1" != "--resolve-registered" ]]; then exit 2; fi\n'
            'printf "%s\\n" "${MOCK_TIKA_RESULT:-$TIKA_ID}"\n',
            encoding="utf-8",
        )

        self.helper.chmod(0o700)
        self.tika_helper.chmod(0o700)

        source = MODULE.read_text(encoding="utf-8")

        functions = "\n".join(
            extract(name, source)
            for name in (
                "openwebui_require_tika_id",
                "openwebui_start_registered",
                "openwebui_verify",
            )
        )

        functions = functions.replace(
            "/usr/libexec/daia-openwebui-register-container",
            str(self.helper),
        ).replace(
            "/usr/libexec/daia-tika-register-container",
            str(self.tika_helper),
        )

        # Mock root only inside the isolated test script.
        functions = functions.replace('"$EUID"', '"0"')

        self.script = functions + r"""
docker() {
    printf "docker %s\n" "$*" >> "$MOCK_LOG"

    if [[ "$1" == "container" && "$2" == "inspect" &&
          "${3:-}" == "--format" ]]; then
        if [[ "$4" == "{{.Id}}" ]]; then
            printf "%s\n" "$WEBUI_ID"
        else
            printf "OLLAMA_BASE_URL=http://127.0.0.1:11434\n"
        fi
    fi

    return 0
}

"$@"
"""

    def execute(self, function, *args, **extra_env):
        env = {
            **os.environ,
            "MOCK_LOG": str(self.log),
            "WEBUI_ID": WEBUI,
            "TIKA_ID": TIKA,
            **extra_env,
        }

        result = subprocess.run(
            ["bash", "-c", self.script, "mock", function, *args],
            capture_output=True,
            text=True,
            env=env,
            timeout=15,
            check=False,
        )

        calls = self.log.read_text(
            encoding="utf-8"
        ).splitlines()

        return result, calls

    def test_legacy_start(self):
        result, calls = self.execute(
            "openwebui_start_registered", WEBUI
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            "webui --authorize-start " + WEBUI, calls
        )
        self.assertIn(
            "webui --verify-running " + WEBUI, calls
        )
        self.assertFalse(
            any(x.startswith("tika ") for x in calls)
        )

    def test_combined_start(self):
        result, calls = self.execute(
            "openwebui_start_registered", WEBUI, TIKA
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("tika --resolve-registered", calls)

        self.assertIn(
            "webui --authorize-start " + WEBUI +
            " --tika-id " + TIKA,
            calls,
        )

        self.assertIn(
            "webui --verify-running " + WEBUI +
            " --tika-id " + TIKA,
            calls,
        )

        self.assertLess(
            calls.index("tika --resolve-registered"),
            calls.index("docker start " + WEBUI),
        )

    def test_untrusted_tika_blocks_start(self):
        result, calls = self.execute(
            "openwebui_start_registered",
            WEBUI,
            TIKA,
            MOCK_TIKA_RESULT=OTHER,
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn(
            "docker start " + WEBUI, calls
        )

        self.assertFalse(
            any(
                x.startswith("webui --authorize-start")
                for x in calls
            )
        )

    def test_invalid_tika_blocks_start(self):
        result, calls = self.execute(
            "openwebui_start_registered",
            WEBUI,
            "invalid",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn(
            "docker start " + WEBUI, calls
        )

    def test_failed_poststart_verification(self):
        result, calls = self.execute(
            "openwebui_start_registered",
            WEBUI,
            TIKA,
            MOCK_FAIL_RUNNING="1",
        )

        self.assertNotEqual(result.returncode, 0)

        self.assertIn(
            "webui --verify-identity " + WEBUI,
            calls,
        )

        self.assertIn(
            "docker stop --time 60 " + WEBUI,
            calls,
        )

        self.assertFalse(
            any("docker rm " in x for x in calls)
        )

    def test_combined_verify_read_only(self):
        result, calls = self.execute(
            "openwebui_verify", TIKA
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("tika --resolve-registered", calls)

        self.assertIn(
            "webui --verify-running " + WEBUI +
            " --tika-id " + TIKA,
            calls,
        )

        self.assertFalse(
            any(
                x.startswith((
                    "docker start",
                    "docker stop",
                    "docker create",
                    "docker rm",
                ))
                for x in calls
            )
        )

    def test_legacy_verify(self):
        result, calls = self.execute("openwebui_verify")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            "webui --verify-running " + WEBUI,
            calls,
        )

        self.assertFalse(
            any(x.startswith("tika ") for x in calls)
        )

    def test_changed_tika_blocks_verification(self):
        result, calls = self.execute(
            "openwebui_verify",
            TIKA,
            MOCK_TIKA_RESULT=OTHER,
        )

        self.assertNotEqual(result.returncode, 0)

        self.assertFalse(
            any(
                x.startswith("webui --verify-running")
                for x in calls
            )
        )

    def test_extra_arguments_rejected(self):
        for function, args in (
            (
                "openwebui_start_registered",
                (WEBUI, TIKA, OTHER),
            ),
            (
                "openwebui_verify",
                (TIKA, OTHER),
            ),
        ):
            self.log.write_text("", encoding="utf-8")
            result, calls = self.execute(function, *args)

            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn(
                "docker start " + WEBUI, calls
            )


if __name__ == "__main__":
    unittest.main()
