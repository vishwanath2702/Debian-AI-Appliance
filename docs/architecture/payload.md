## Payload Content Classification

The current installer tree contains several different classes of content that
should be handled differently by the future payload format.

### Immutable Program Files

These are installed application files and should be treated as payload content:

```text
/opt/daia/bootstrap.sh
/opt/daia/install.sh
/opt/daia/builder/
/opt/daia/core/
/opt/daia/lib/
/opt/daia/modules/
/opt/daia/planner/


## Current Installation Flow

The installed `/opt/daia/install.sh` script does not copy payload files into
their destination paths.

Instead, it assumes the DAIA filesystem tree has already been placed under
`/opt/daia`.

Its responsibilities are:

1. Resolve the installed DAIA directory.
2. Load `/opt/daia/config/daia.conf`.
3. Load common and logging libraries.
4. Validate required functions and commands.
5. Require root privileges.
6. Execute `/opt/daia/bootstrap.sh`.
7. Disable `daia-firstboot.service` after successful bootstrap.
8. Report final installation success.

This means the current installation process has at least two distinct stages:

```text
filesystem deployment
        ↓
first-boot bootstrap
        ↓
disable first-boot service
