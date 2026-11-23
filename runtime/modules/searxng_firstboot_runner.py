"""Fixed-purpose, fail-closed SearXNG firstboot lock holder.

Not yet wired into firstboot.
"""

import os
import subprocess

from searxng_firstboot_lock import exclusive_firstboot_lock


class RunnerRefused(RuntimeError):
    pass


PROVISION_COMMAND = (
    "/usr/libexec/daia-searxng-firstboot-provision",
)


def run():
    if os.geteuid() != 0:
        raise RunnerRefused("Root required")

    with exclusive_firstboot_lock():
        try:
            result = subprocess.run(
                PROVISION_COMMAND,
                check=False,
                timeout=300,
                close_fds=True,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise RunnerRefused(
                "Firstboot provisioning execution failed"
            ) from error

        if result.returncode != 0:
            raise RunnerRefused(
                "Firstboot provisioning returned failure"
            )

    return True


if __name__ == "__main__":
    try:
        run()
    except Exception as error:
        print(
            "SearXNG firstboot refused: " + str(error),
            file=__import__("sys").stderr,
        )
        raise SystemExit(1)
