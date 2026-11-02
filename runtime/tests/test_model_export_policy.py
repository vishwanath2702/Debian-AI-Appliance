"""Security regression tests for the DAIA Model Exporter polkit policy."""

from pathlib import Path
import unittest
import xml.etree.ElementTree as ET


POLICY = (
    Path(__file__).resolve().parents[1]
    / "polkit-1/actions/org.daia.model-export.policy"
)


def validate_policy(source):
    root = ET.fromstring(source)
    actions = root.findall("action")

    if root.tag != "policyconfig" or len(actions) != 1:
        return False

    action = actions[0]
    if action.get("id") != "org.daia.model-export":
        return False

    defaults = action.find("defaults")
    if defaults is None:
        return False

    expected = {
        "allow_any": "no",
        "allow_inactive": "no",
        "allow_active": "auth_admin",
    }

    if any(defaults.findtext(key) != value for key, value in expected.items()):
        return False

    annotations = [
        element
        for element in action.findall("annotate")
        if element.get("key") == "org.freedesktop.policykit.exec.path"
    ]

    return (
        len(annotations) == 1
        and annotations[0].text == "/usr/libexec/daia-model-export-broker"
    )


class ExportPolicyTests(unittest.TestCase):
    def test_installed_policy_contract(self):
        self.assertTrue(validate_policy(POLICY.read_text()))

    def test_rejects_cached_authorization(self):
        source = POLICY.read_text().replace(
            "<allow_active>auth_admin</allow_active>",
            "<allow_active>auth_admin_keep</allow_active>",
        )
        self.assertFalse(validate_policy(source))

    def test_rejects_automatic_authorization(self):
        source = POLICY.read_text().replace(
            "<allow_active>auth_admin</allow_active>",
            "<allow_active>yes</allow_active>",
        )
        self.assertFalse(validate_policy(source))

    def test_rejects_wrong_broker_path(self):
        source = POLICY.read_text().replace(
            "/usr/libexec/daia-model-export-broker",
            "/usr/bin/daia-model-import",
        )
        self.assertFalse(validate_policy(source))

    def test_rejects_inactive_session_authorization(self):
        source = POLICY.read_text().replace(
            "<allow_inactive>no</allow_inactive>",
            "<allow_inactive>auth_admin</allow_inactive>",
        )
        self.assertFalse(validate_policy(source))


if __name__ == "__main__":
    unittest.main()
