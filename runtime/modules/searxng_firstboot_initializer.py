"""Inactive, root-only SearXNG registration initializer.

No advisory grant or caller-selected resource names are accepted.
Firstboot wiring and concurrency protection remain separate work.
"""

import os
import subprocess

import searxng_bootstrap
import searxng_config
import searxng_initialize


class InitializationRefused(RuntimeError):
    pass


def _docker(command):
    try:
        result = subprocess.run(
            ("docker",) + command,
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise InitializationRefused("Docker inspection failed") from exc

    if result.returncode != 0 or type(result.stdout) is not str:
        raise InitializationRefused("Docker inspection failed")

    return result.stdout


def _require_absent(kind, name):
    if (kind, name) == ("container", "daia-searxng"):
        command = ("container", "ls", "-a", "--format", "{{.Names}}")
    elif (kind, name) == ("network", "daia-searxng-egress"):
        command = ("network", "ls", "--format", "{{.Name}}")
    else:
        raise InitializationRefused("Unexpected Docker resource")

    names = _docker(command).splitlines()

    if (
        len(names) != len(set(names))
        or any(not item or item != item.strip() for item in names)
    ):
        raise InitializationRefused("Malformed Docker inventory")

    if name in names:
        raise InitializationRefused("Existing SearXNG resource")


def initialize_from_firstboot():
    if os.geteuid() != 0:
        raise InitializationRefused("Root required")

    if searxng_config.enabled() is not True:
        raise InitializationRefused("SearXNG not enabled")

    eligible = searxng_bootstrap.BootstrapDecision.INITIALIZE_ELIGIBLE

    if searxng_bootstrap.decide() is not eligible:
        raise InitializationRefused("Registration initialization not eligible")

    if not _docker(("info", "--format", "{{.ID}}")).strip():
        raise InitializationRefused("Docker unavailable")

    resources = (
        ("container", "daia-searxng"),
        ("network", "daia-searxng-egress"),
    )

    for kind, name in resources:
        _require_absent(kind, name)

    for kind, name in resources:
        _require_absent(kind, name)

    if searxng_config.enabled() is not True:
        raise InitializationRefused("Configuration changed")

    if searxng_bootstrap.decide() is not eligible:
        raise InitializationRefused("Registration state changed")

    searxng_initialize.initialize()
