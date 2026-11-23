from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
FILE = ROOT / "runtime/daia-searxng-verify-container"

CID = "a" * 64
INTERNAL_ID = "b" * 64
EGRESS_ID = "c" * 64

scope = {"__name__": "searxng_verifier"}
exec(compile(FILE.read_text(), str(FILE), "exec"), scope)

verify = scope["verify"]
valid_id = scope["valid_id"]


def fixture():
    container = {
        "Id": CID,
        "Name": "/daia-searxng",
        "Image": "sha256:be131d45cb5daf22a1f47c530b6d11063c76e53bf0efa99d047b07e4b3d4274d",
        "Config": {
            "Labels": {
                "org.daia.component": "searxng",
                "org.daia.managed": "true",
            }
        },
        "HostConfig": {
            "Privileged": False,
            "SecurityOpt": ["no-new-privileges:true"],
            "NetworkMode": "daia-rag-internal",
            "PortBindings": {},
            "Binds": None,
            "Mounts": None,
        },
        "Mounts": [],
        "State": {
            "Running": True,
            "Paused": False,
            "Restarting": False,
            "Status": "running",
        },
        "NetworkSettings": {
            "Networks": {
                "daia-rag-internal": {
                    "NetworkID": INTERNAL_ID
                },
                "daia-searxng-egress": {
                    "NetworkID": EGRESS_ID
                },
            }
        },
    }

    def make_network(name, network_id, internal, component):
        return {
            "Name": name,
            "Id": network_id,
            "Driver": "bridge",
            "Internal": internal,
            "Labels": {
                "org.daia.component": component,
                "org.daia.managed": "true",
            },
            "Containers": {CID: {}},
        }

    return (
        container,
        make_network(
            "daia-rag-internal",
            INTERNAL_ID,
            True,
            "rag",
        ),
        make_network(
            "daia-searxng-egress",
            EGRESS_ID,
            False,
            "searxng",
        ),
    )


class SearxngSecurityTests(unittest.TestCase):
    def check(self, mutation=None, allowed=True):
        container, internal, egress = fixture()

        if mutation:
            mutation(container, internal, egress)

        def mock_inspect(*args):
            if args[:2] == ("container", "inspect"):
                return [container]
            if args == (
                "network", "inspect", "daia-rag-internal"
            ):
                return [internal]
            if args == (
                "network", "inspect", "daia-searxng-egress"
            ):
                return [egress]
            raise AssertionError(args)

        with patch.dict(
            verify.__globals__,
            {"inspect": mock_inspect},
        ):
            with patch("os.geteuid", return_value=0):
                if allowed:
                    self.assertEqual(verify(CID), CID)
                else:
                    with self.assertRaises(PermissionError):
                        verify(CID)

    def test_valid_id(self):
        self.assertTrue(valid_id(CID))
        self.assertFalse(valid_id("not-an-id"))
        self.assertFalse(valid_id("g" * 64))

    def test_authorized_identity(self):
        self.check()

    def test_stopped_created_without_active_membership(self):
        def mutate(c, internal, egress):
            c["State"]["Running"] = False
            c["State"]["Status"] = "created"
            internal["Containers"] = {}
            egress["Containers"] = {}
        self.check(mutate)

    def test_stopped_exited_without_active_membership(self):
        def mutate(c, internal, egress):
            c["State"]["Running"] = False
            c["State"]["Status"] = "exited"
            internal["Containers"] = {}
            egress["Containers"] = {}
        self.check(mutate)

    def test_running_requires_both_memberships(self):
        self.check(
            lambda c, i, e: e.update(Containers={}),
            False,
        )

    def test_stopped_rejects_foreign_egress_member(self):
        def mutate(c, internal, egress):
            c["State"]["Running"] = False
            c["State"]["Status"] = "exited"
            egress["Containers"] = {"d" * 64: {}}
        self.check(mutate, False)

    def test_paused_container_rejected(self):
        self.check(
            lambda c, i, e:
            c["State"].update(Paused=True),
            False,
        )

    def test_restarting_container_rejected(self):
        self.check(
            lambda c, i, e:
            c["State"].update(Restarting=True),
            False,
        )

    def test_wrong_image(self):
        self.check(
            lambda c, i, e:
            c.update(Image="sha256:" + "0" * 64),
            False,
        )

    def test_unmanaged_container(self):
        self.check(
            lambda c, i, e:
            c["Config"]["Labels"].update(
                {"org.daia.managed": "false"}
            ),
            False,
        )

    def test_privileged_container(self):
        self.check(
            lambda c, i, e:
            c["HostConfig"].update(Privileged=True),
            False,
        )

    def test_published_port(self):
        self.check(
            lambda c, i, e:
            c["HostConfig"].update(
                PortBindings={
                    "8080/tcp": [{"HostPort": "8080"}]
                }
            ),
            False,
        )

    def test_internal_network_isolation(self):
        self.check(
            lambda c, i, e:
            i.update(Internal=False),
            False,
        )

    def test_egress_network_isolation(self):
        self.check(
            lambda c, i, e:
            e.update(Internal=True),
            False,
        )

    def test_unauthorized_egress_peer(self):
        self.check(
            lambda c, i, e:
            e["Containers"].update({"d" * 64: {}}),
            False,
        )

    def test_extra_network(self):
        self.check(
            lambda c, i, e:
            c["NetworkSettings"]["Networks"].update(
                {"bridge": {"NetworkID": "d" * 64}}
            ),
            False,
        )

    def test_wrong_network_identity(self):
        self.check(
            lambda c, i, e:
            c["NetworkSettings"]["Networks"][
                "daia-searxng-egress"
            ].update(NetworkID="d" * 64),
            False,
        )


if __name__ == "__main__":
    unittest.main()
