import importlib.util
import json
from pathlib import Path
from unittest import TestCase, mock

ROOT = Path(__file__).resolve().parents[2]
ASSESSOR = ROOT / "runtime/daia-searxng-assess-recovery"

spec = importlib.util.spec_from_file_location(
    "searxng_assessor",
    ASSESSOR,
)

# The extensionless production script is loaded as Python source.
from importlib.machinery import SourceFileLoader

loader = SourceFileLoader("searxng_assessor", str(ASSESSOR))
spec = importlib.util.spec_from_loader("searxng_assessor", loader)
assessor = importlib.util.module_from_spec(spec)
loader.exec_module(assessor)

ID = "a" * 64
NETWORK_ID = "b" * 64


class FakeRegistration:
    DEFAULT_DIRECTORY = "/var/lib/daia/searxng"
    REGISTRATION_NAME = "container-id"

    def __init__(self, identity=None, valid=True):
        self.identity = identity
        self.valid = valid

    def _open_trusted_directory(self, directory):
        if directory != self.DEFAULT_DIRECTORY:
            raise PermissionError("Unexpected directory")
        return 123

    def _read_registration_fd(self, directory_fd):
        if directory_fd != 123:
            raise PermissionError("Unexpected descriptor")
        if self.identity is None:
            raise FileNotFoundError(
                2, "Registration absent", self.REGISTRATION_NAME
            )
        return self.identity

    def verify_registration(self, identity):
        if not self.valid:
            raise PermissionError("Verification rejected")
        return identity


def container(attachments=None):
    if attachments is None:
        attachments = {
            assessor.INTERNAL: {"NetworkID": NETWORK_ID}
        }

    return {
        "Id": ID,
        "Name": "/" + assessor.NAME,
        "Image": assessor.IMAGE,
        "Config": {
            "Labels": {
                "org.daia.component": "searxng",
                "org.daia.managed": "true",
            },
        },
        "HostConfig": {
            "Privileged": False,
            "SecurityOpt": ["no-new-privileges:true"],
            "NetworkMode": assessor.INTERNAL,
            "PortBindings": {},
            "Binds": [],
            "Mounts": [],
            "RestartPolicy": {"Name": "no"},
        },
        "Mounts": [],
        "State": {
            "Running": False,
            "Paused": False,
            "Restarting": False,
            "Status": "created",
        },
        "NetworkSettings": {"Networks": attachments},
    }


def internal_network():
    return {
        "Id": NETWORK_ID,
        "Name": assessor.INTERNAL,
        "Driver": "bridge",
        "Internal": True,
        "Labels": {
            "org.daia.component": "rag",
            "org.daia.managed": "true",
        },
    }


class RecoveryAssessmentTests(TestCase):

    def execute(
        self,
        present=True,
        registration=None,
        valid=True,
        value=None,
        network=None,
        fail=None,
    ):
        calls = []

        def fake_docker(*args):
            calls.append(args)

            if args == fail:
                raise assessor.Untrusted("Mocked Docker failure")

            if args == ("info",):
                return ""

            if args[:2] == ("container", "ls"):
                return assessor.NAME + "\n" if present else ""

            if args[:2] == ("container", "inspect"):
                return json.dumps([value or container()])

            if args[:2] == ("network", "inspect"):
                return json.dumps([network or internal_network()])

            raise AssertionError("Unexpected Docker call: " + str(args))

        with (
            mock.patch.object(assessor.os, "geteuid", return_value=0),
            mock.patch.object(assessor.os, "close"),
            mock.patch.object(assessor, "docker", side_effect=fake_docker),
        ):
            result = assessor.assess(
                FakeRegistration(registration, valid)
            )

        return result, calls

    def test_absent(self):
        result, calls = self.execute(present=False)
        self.assertEqual(result, "ABSENT")
        self.assertFalse(
            any(call[:2] == ("container", "inspect") for call in calls)
        )

    def test_absent_with_registration_rejected(self):
        with self.assertRaises(assessor.Untrusted):
            self.execute(present=False, registration=ID)

    def test_registered_verified(self):
        result, _ = self.execute(registration=ID)
        self.assertEqual(result, "REGISTERED_VERIFIED")

    def test_registered_verification_failure(self):
        with self.assertRaises(PermissionError):
            self.execute(registration=ID, valid=False)

    def test_registration_identity_mismatch(self):
        with self.assertRaises(assessor.Untrusted):
            self.execute(registration="c" * 64)

    def test_incomplete_created_container(self):
        result, calls = self.execute()
        self.assertEqual(result, "UNREGISTERED_STOPPED")
        self.assertEqual(
            sum(call[:2] == ("container", "inspect") for call in calls),
            2,
        )

    def test_running_unregistered_rejected(self):
        value = container()
        value["State"]["Running"] = True
        value["State"]["Status"] = "running"

        with self.assertRaises(assessor.Untrusted):
            self.execute(value=value)

    def test_extra_egress_attachment_rejected(self):
        attachments = {
            assessor.INTERNAL: {"NetworkID": NETWORK_ID},
            assessor.EGRESS: {"NetworkID": "c" * 64},
        }

        with self.assertRaises(assessor.Untrusted):
            self.execute(value=container(attachments))

    def test_untrusted_network_rejected(self):
        network = internal_network()
        network["Internal"] = False

        with self.assertRaises(assessor.Untrusted):
            self.execute(network=network)

    def test_identity_mismatch_rejected(self):
        value = container()
        value["Image"] = "sha256:" + "f" * 64

        with self.assertRaises(assessor.Untrusted):
            self.execute(value=value)

    def test_docker_inventory_failure_rejected(self):
        with self.assertRaises(assessor.Untrusted):
            self.execute(
                fail=(
                    "container", "ls", "-a",
                    "--format", "{{.Names}}",
                )
            )

    def test_root_required(self):
        with mock.patch.object(
            assessor.os, "geteuid", return_value=1000
        ):
            with self.assertRaises(assessor.Untrusted):
                assessor.assess(FakeRegistration())

    def test_no_mutation_calls(self):
        _, calls = self.execute()
        for call in calls:
            self.assertIn(call[0], ("info", "container", "network"))
            self.assertNotIn(
                call[1] if len(call) > 1 else "",
                ("create", "start", "stop", "rm", "connect"),
            )

    def test_not_wired_to_existing_recovery(self):
        existing = (ROOT / "runtime/daia-rag-recover").read_text()
        self.assertNotIn("daia-searxng-assess-recovery", existing)

    def test_registration_directory_missing_fail_closed(self):
        class MissingDirectory(FakeRegistration):
            def _open_trusted_directory(self, directory):
                raise FileNotFoundError(
                    2, "Directory absent", str(directory)
                )

        with self.assertRaises(assessor.Untrusted):
            assessor.registered_identity(MissingDirectory())

    def test_registration_permission_failure_rejected(self):
        class DeniedDirectory(FakeRegistration):
            def _open_trusted_directory(self, directory):
                raise PermissionError("Untrusted directory")

        with self.assertRaises(PermissionError):
            assessor.registered_identity(DeniedDirectory())

    def test_registration_ambiguous_missing_path_rejected(self):
        class AmbiguousFile(FakeRegistration):
            def _read_registration_fd(self, directory_fd):
                raise FileNotFoundError(
                    2, "Unexpected missing path", "other-file"
                )

        with mock.patch.object(assessor.os, "close"):
            with self.assertRaises(assessor.Untrusted):
                assessor.registered_identity(AmbiguousFile())

    def test_registration_malformed_file_rejected(self):
        class MalformedFile(FakeRegistration):
            def _read_registration_fd(self, directory_fd):
                raise PermissionError("Malformed registration")

        with mock.patch.object(assessor.os, "close"):
            with self.assertRaises(PermissionError):
                assessor.registered_identity(MalformedFile())

    def test_registration_descriptor_closed_on_failure(self):
        class MalformedFile(FakeRegistration):
            def _read_registration_fd(self, directory_fd):
                raise PermissionError("Malformed registration")

        with mock.patch.object(assessor.os, "close") as close:
            with self.assertRaises(PermissionError):
                assessor.registered_identity(MalformedFile())

        close.assert_called_once_with(123)

    def test_second_inspection_configuration_drift_rejected(self):
        first = container()
        second = json.loads(json.dumps(first))
        second["HostConfig"]["Privileged"] = True

        with (
            mock.patch.object(
                assessor, "inspect_container", return_value=second
            ),
            mock.patch.object(
                assessor, "inspect_internal_network",
                return_value=internal_network(),
            ),
        ):
            with self.assertRaises(assessor.Untrusted):
                assessor.incomplete_stopped_container(first)

    def test_second_inspection_identity_drift_rejected(self):
        first = container()
        second = json.loads(json.dumps(first))
        second["Id"] = "c" * 64

        with (
            mock.patch.object(
                assessor, "inspect_container", return_value=second
            ),
            mock.patch.object(
                assessor, "inspect_internal_network",
                return_value=internal_network(),
            ),
        ):
            with self.assertRaises(assessor.Untrusted):
                assessor.incomplete_stopped_container(first)

    def test_assessor_packaged_without_activation(self):
        payload = (ROOT / "build/build-payload.sh").read_text()
        recovery = (ROOT / "runtime/daia-rag-recover").read_text()

        self.assertIn(
            '$DAIA_RUNTIME_SOURCE/daia-searxng-assess-recovery',
            payload,
        )
        self.assertIn(
            '$DAIA_WORKSPACE/usr/libexec/daia-searxng-assess-recovery',
            payload,
        )
        self.assertNotIn(
            "daia-searxng-assess-recovery",
            recovery,
        )
