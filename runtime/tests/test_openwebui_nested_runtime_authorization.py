"""Nested RAG authorization must retain explicit WebUI identity."""

import importlib.util
import runpy
import subprocess
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
RUNTIME = ROOT / "runtime"
PG_ID = "a" * 64
WEBUI_ID = "b" * 64


class NestedRuntimeAuthorizationTests(unittest.TestCase):
    def test_provision_authorizer_receives_webui_identity(self):
        path = RUNTIME / "daia-openwebui-rag-provision.py"
        spec = importlib.util.spec_from_file_location(
            "pragna_nested_rag_provision", path
        )
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)

        commands = []

        def runner(command, **kwargs):
            commands.append(command)
            return SimpleNamespace(
                returncode=0, stdout=PG_ID + "\n"
            )

        self.assertEqual(
            module.authorize_container(
                runner=runner, webui_id=WEBUI_ID
            ),
            PG_ID,
        )
        self.assertEqual(
            commands,
            [[module.AUTHORIZER, "runtime", WEBUI_ID]],
        )

    def test_default_provision_authorization_is_unchanged(self):
        path = RUNTIME / "daia-openwebui-rag-provision.py"
        spec = importlib.util.spec_from_file_location(
            "pragna_nested_rag_default", path
        )
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)

        commands = []

        def runner(command, **kwargs):
            commands.append(command)
            return SimpleNamespace(
                returncode=0, stdout=PG_ID + "\n"
            )

        self.assertEqual(
            module.authorize_container(runner=runner), PG_ID
        )
        self.assertEqual(
            commands, [[module.AUTHORIZER, "runtime"]]
        )

    def test_outer_authorizer_passes_identity_to_both_checks(self):
        namespace = runpy.run_path(
            str(RUNTIME / "daia-openwebui-rag-authorize")
        )
        commands = []

        def runner(command, **kwargs):
            commands.append(command)
            if len(commands) == 1:
                return subprocess.CompletedProcess(
                    command, 0, PG_ID + "\n", ""
                )
            return subprocess.CompletedProcess(command, 0, "", "")

        with (
            patch("os.geteuid", return_value=0),
            patch("subprocess.run", side_effect=runner),
        ):
            self.assertTrue(namespace["preflight"](WEBUI_ID))

        self.assertEqual(
            commands[0],
            [namespace["PG_AUTHORIZER"], "runtime", WEBUI_ID],
        )
        self.assertEqual(
            commands[2],
            [
                namespace["ROLE_VERIFIER"],
                "--verify-only", PG_ID, WEBUI_ID,
            ],
        )

    def test_invalid_identity_fails_before_subprocess(self):
        namespace = runpy.run_path(
            str(RUNTIME / "daia-openwebui-rag-authorize")
        )
        with (
            patch("os.geteuid", return_value=0),
            patch("subprocess.run") as runner,
        ):
            with self.assertRaises(namespace["AuthorizationError"]):
                namespace["preflight"]("invalid")
            runner.assert_not_called()

    def test_running_helper_propagates_exact_identity(self):
        source = (
            RUNTIME / "daia-openwebui-register-container"
        ).read_text(encoding="utf-8")
        self.assertIn(
            'verify_running_rag_role(container_id)', source
        )
        self.assertIn('"--webui-id",', source)
        self.assertIn(
            'validate_container_id(container_id)', source
        )


    def test_role_verifier_cli_validates_optional_identity(self):
        import importlib.util
        from unittest.mock import patch

        path = RUNTIME / "daia-openwebui-rag-provision.py"
        spec = importlib.util.spec_from_file_location(
            "pragna_nested_cli_validation", path
        )
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)

        with patch.object(
            module, "verify_existing_role", return_value=True
        ) as verifier:
            self.assertEqual(
                module.main([
                    "--verify-only", PG_ID, "invalid-identity"
                ]),
                1,
            )
            verifier.assert_not_called()

            self.assertEqual(
                module.main([
                    "--verify-only", PG_ID, WEBUI_ID
                ]),
                0,
            )
            verifier.assert_called_once_with(PG_ID, WEBUI_ID)


if __name__ == "__main__":
    unittest.main()
