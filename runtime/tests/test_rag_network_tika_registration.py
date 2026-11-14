import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "daia-rag-verify-network"

PG = "a" * 64
TIKA = "b" * 64
NETWORK = "c" * 64


class TikaRegistrationNetworkTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)

        self.authorizer = self.directory / "pg-authorizer"
        self.arguments = self.directory / "authorization-args.txt"

        self.authorizer.write_text(
            """#!/usr/bin/env python3
import os
import sys
from pathlib import Path

with Path(os.environ["AUTH_ARGS"]).open("a") as stream:
    stream.write(" ".join(sys.argv[1:]) + "\\n")

if os.environ.get("AUTH_FAIL") == "1":
    sys.exit(1)

print(os.environ.get("AUTH_PG", os.environ["PG_ID"]))
""",
            encoding="utf-8",
        )
        self.authorizer.chmod(0o755)

        docker = self.directory / "docker"
        docker.write_text(
            """#!/usr/bin/env python3
import os
import sys

args = sys.argv[1:]
if args == ["info"]:
    sys.exit(0)

if args[:2] == ["network", "inspect"]:
    fmt = args[3] if len(args) > 3 else ""
    if ".Name" in fmt:
        print("daia-rag-internal\\tbridge\\ttrue\\trag\\ttrue")
    elif ".Containers" in fmt:
        print(os.environ.get("ATTACHED", ""))
    elif ".Id" in fmt:
        print(os.environ["NETWORK_ID"])
    else:
        sys.exit(2)
elif args[:2] == ["container", "inspect"]:
    print(os.environ["NETWORK_ID"])
else:
    sys.exit(2)
""",
            encoding="utf-8",
        )
        docker.chmod(0o755)

        original = SOURCE.read_text(encoding="utf-8")
        self.assertEqual(
            original.count('if [[ "$EUID" -ne 0 ]]; then'), 1
        )
        self.assertEqual(
            original.count(
                "/usr/libexec/daia-pgvector-authorize-start"
            ), 1
        )

        fixture = original.replace(
            'if [[ "$EUID" -ne 0 ]]; then',
            'if [[ 0 -ne 0 ]]; then',
            1,
        ).replace(
            "/usr/libexec/daia-pgvector-authorize-start",
            str(self.authorizer),
            1,
        )

        self.script = self.directory / "verify-network"
        self.script.write_text(fixture, encoding="utf-8")
        self.script.chmod(0o755)

        self.environment = {
            **os.environ,
            "PATH": str(self.directory) + os.pathsep
                    + os.environ.get("PATH", ""),
            "PG_ID": PG,
            "NETWORK_ID": NETWORK,
            "AUTH_ARGS": str(self.arguments),
            "ATTACHED": PG,
        }

    def invoke(self, *arguments, **environment):
        env = dict(self.environment)
        env.update(environment)
        return subprocess.run(
            ["bash", str(self.script), "registration", *arguments],
            capture_output=True,
            text=True,
            check=False,
            timeout=10,
            env=env,
        )

    def auth_calls(self):
        if not self.arguments.exists():
            return []
        return self.arguments.read_text(
            encoding="utf-8"
        ).splitlines()

    def test_legacy_postgresql_only_remains_valid(self):
        result = self.invoke(PG)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "daia-rag-internal\n")
        self.assertEqual(self.auth_calls(), [])

    def test_legacy_rejects_unexpected_tika_member(self):
        result = self.invoke(PG, ATTACHED=PG + "\n" + TIKA)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.auth_calls(), [])

    def test_combined_registration_delegates_trusted_proof(self):
        result = self.invoke(
            PG, TIKA, ATTACHED=PG + "\n" + TIKA
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "daia-rag-internal\n")
        self.assertEqual(
            self.auth_calls(),
            ["runtime --tika-id " + TIKA],
        )

    def test_combined_registration_rejects_authorizer_failure(self):
        result = self.invoke(
            PG, TIKA, AUTH_FAIL="1",
            ATTACHED=PG + "\n" + TIKA,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(self.auth_calls()), 1)

    def test_combined_registration_rejects_wrong_pg_identity(self):
        result = self.invoke(
            PG, TIKA, AUTH_PG="d" * 64,
            ATTACHED=PG + "\n" + TIKA,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(self.auth_calls()), 1)

    def test_rejects_invalid_or_duplicate_tika_id(self):
        for tika_id in ("invalid", PG):
            with self.subTest(tika_id=tika_id):
                result = self.invoke(PG, tika_id)
                self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.auth_calls(), [])

    def test_rejects_extra_registration_arguments(self):
        result = self.invoke(PG, TIKA, "unexpected")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.auth_calls(), [])


if __name__ == "__main__":
    unittest.main()
