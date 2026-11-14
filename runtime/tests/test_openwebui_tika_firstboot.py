import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "modules/openwebui.sh"
WEBUI = "a" * 64
TIKA = "b" * 64
OTHER = "c" * 64
IMAGE = (
    "sha256:e29d4d2457b54bb14434bee2666420e822cb0e9389466f02405d69fa7fd8d06c"
)


def extract(name, source):
    pattern = re.compile(
        rf"(?ms)^{re.escape(name)}\(\) \{{\n.*?^\}}\n"
    )
    matches = pattern.findall(source)
    if len(matches) != 1:
        raise AssertionError("Function not unique: " + name)
    return matches[0]


class TikaFirstbootTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        folder = Path(temporary.name)
        self.log = folder / "calls.log"
        self.log.write_text("", encoding="utf-8")

        webui = folder / "webui-register"
        tika = folder / "tika-register"
        rag = folder / "rag-authorize"

        webui.write_text(
            '#!/usr/bin/env bash\n'
            'printf "register %s\\n" "$*" >> "$MOCK_LOG"\n'
            'case "$1" in\n'
            '  --verify-identity|--verify-running)\n'
            '    printf "%s\\n" "$WEBUI_ID" ;;\n'
            'esac\n',
            encoding="utf-8",
        )
        tika.write_text(
            '#!/usr/bin/env bash\n'
            'printf "tika %s\\n" "$*" >> "$MOCK_LOG"\n'
            '[[ "$1" == "--resolve-registered" ]] || exit 1\n'
            '[[ "${MOCK_TIKA_FAIL:-0}" != 1 ]] || exit 1\n'
            'printf "%s\\n" "${MOCK_TIKA_RESULT:-$TIKA_ID}"\n',
            encoding="utf-8",
        )
        rag.write_text(
            '#!/usr/bin/env bash\n'
            'printf "rag %s\\n" "$*" >> "$MOCK_LOG"\n',
            encoding="utf-8",
        )
        for path in (webui, tika, rag):
            path.chmod(0o700)

        source = SOURCE.read_text(encoding="utf-8")
        functions = "\n".join(
            extract(name, source)
            for name in (
                "openwebui_resolve_registered_tika_id",
                "openwebui_require_tika_id",
                "openwebui_install",
                "openwebui_verify",
            )
        )
        functions = functions.replace(
            "/usr/libexec/daia-tika-register-container", str(tika)
        ).replace(
            "/usr/libexec/daia-openwebui-register-container", str(webui)
        ).replace(
            "/usr/libexec/daia-openwebui-rag-authorize", str(rag)
        ).replace('"$EUID"', '"0"')

        self.script = functions + r"""
openwebui_validate() { return 0; }
openwebui_ensure_volume() { return 0; }

openwebui_prepare_rag() {
    printf "prepare %s\n" "$*" >> "$MOCK_LOG"
}

openwebui_start_registered() {
    printf "start %s\n" "$*" >> "$MOCK_LOG"
}

docker() {
    printf "docker %s\n" "$*" >> "$MOCK_LOG"

    if [[ "$1" == "container" && "$2" == "inspect" ]]; then
        if [[ "${3:-}" != "--format" ]]; then
            [[ "${MOCK_EXIST:-0}" == 1 ]]
            return
        fi
        if [[ "$4" == "{{.Id}}" ]]; then
            printf "%s\n" "$WEBUI_ID"
        elif [[ "$4" == *State.Running* ]]; then
            printf "%s\n" "${MOCK_STATE:-true|false|false|running}"
        else
            printf "OLLAMA_BASE_URL=http://127.0.0.1:11434\n"
        fi
        return 0
    fi

    if [[ "$1" == "load" ]]; then
        printf "Loaded image: mock-image\n"
    elif [[ "$1" == "image" && "$2" == "inspect" ]]; then
        printf "%s\n" "$IMAGE_ID"
    elif [[ "$1" == "container" && "$2" == "create" ]]; then
        printf "%s\n" "$WEBUI_ID"
    fi
    return 0
}

"$@"
"""

    def execute(self, function, enabled=True, **options):
        env = {
            **os.environ,
            "MOCK_LOG": str(self.log),
            "WEBUI_ID": WEBUI,
            "TIKA_ID": TIKA,
            "IMAGE_ID": IMAGE,
            "DAIA_ENABLE_TIKA": "true" if enabled else "false",
            **options,
        }
        result = subprocess.run(
            ["bash", "-c", self.script, "mock", function],
            capture_output=True, text=True, env=env,
            timeout=15, check=False,
        )
        return result, self.log.read_text(
            encoding="utf-8"
        ).splitlines()

    def test_new_install_propagates_tika_to_all_gates(self):
        result, calls = self.execute("openwebui_install")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("prepare " + TIKA, calls)
        self.assertIn("rag --tika-id " + TIKA, calls)
        self.assertIn(
            "register --tika-id " + TIKA + " " + WEBUI,
            calls,
        )
        self.assertIn("start " + WEBUI + " " + TIKA, calls)
        self.assertLess(
            calls.index("tika --resolve-registered"),
            calls.index("docker container create " + next(
                x.split("docker container create ", 1)[1]
                for x in calls
                if x.startswith("docker container create ")
            )),
        )

    def test_running_recovery_uses_combined_verification(self):
        result, calls = self.execute(
            "openwebui_install", MOCK_EXIST="1"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            "register --verify-running " + WEBUI +
            " --tika-id " + TIKA,
            calls,
        )
        self.assertFalse(any(x.startswith("docker load") for x in calls))

    def test_stopped_recovery_uses_combined_start(self):
        result, calls = self.execute(
            "openwebui_install",
            MOCK_EXIST="1",
            MOCK_STATE="false|false|false|exited",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("start " + WEBUI + " " + TIKA, calls)

    def test_missing_trusted_tika_blocks_docker(self):
        result, calls = self.execute(
            "openwebui_install", MOCK_TIKA_FAIL="1"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(x.startswith("docker ") for x in calls))
        self.assertFalse(any(x.startswith("prepare ") for x in calls))

    def test_invalid_tika_identity_blocks_install(self):
        result, calls = self.execute(
            "openwebui_install", MOCK_TIKA_RESULT="invalid"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(x.startswith("docker ") for x in calls))

    def test_disabled_tika_preserves_legacy_install(self):
        result, calls = self.execute(
            "openwebui_install", enabled=False
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("prepare ", calls)
        self.assertIn("rag ", calls)
        self.assertIn("register " + WEBUI, calls)
        self.assertIn("start " + WEBUI, calls)
        self.assertFalse(any(x.startswith("tika ") for x in calls))

    def test_enabled_tika_auto_selected_for_verification(self):
        result, calls = self.execute("openwebui_verify")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            "register --verify-running " + WEBUI +
            " --tika-id " + TIKA,
            calls,
        )

    def test_disabled_tika_keeps_legacy_verification(self):
        result, calls = self.execute(
            "openwebui_verify", enabled=False
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("register --verify-running " + WEBUI, calls)
        self.assertFalse(any(x.startswith("tika ") for x in calls))

    def test_unavailable_tika_blocks_verification(self):
        result, calls = self.execute(
            "openwebui_verify", MOCK_TIKA_FAIL="1"
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(
            any(x.startswith("register --verify-running") for x in calls)
        )


if __name__ == "__main__":
    unittest.main()
