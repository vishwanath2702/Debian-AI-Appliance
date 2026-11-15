# Loom / DAIA Repository Testing Playbook

**Status: active development — November 15, 2026.** This is a tester-oriented guide, not a released or fully validated installation procedure. The first appliance is DAIA/Pragna. Other Loom appliance families are planned.

## 1. What you need

- An **amd64** Linux build host, preferably a supported Debian environment, with an initial internet connection, sufficient free disk space, and a USB drive or spare test disk.
- Git, Rust/Cargo, and the Debian ISO/build utilities required by the repository's build scripts. Exact host dependency versions and a fresh-clone bootstrap command are **not yet verified**; review build/check.sh and the build scripts before proceeding.
- A **supported Debian amd64 base ISO**, downloaded in advance. The current `builder.sh` looks for a single `debian-*-amd64-netinst.iso` in `iso/` unless `DAIA_SOURCE_ISO` is set. A matching filename alone does **not** establish that an arbitrary Debian ISO is compatible with the build pipeline.
- Separately obtained offline packages, pinned Ollama archive, and any required container images or other payload artifacts. **A Git clone alone may not contain these large assets.** Consult `payload/config/pragna.conf`, `build/check.sh`, and the payload manifests; asset acquisition and cache reuse are still being standardized.

**Safety:** Use a dedicated test disk or VM. The installer can erase selected storage. Back up important data and confirm the target disk before any destructive step. Never test installation on your only copy of important data.

## 2. Online preparation (do once, then reuse assets)

1. Clone the repository: `git clone https://github.com/vishwanath2702/Debian-AI-Appliance.git` and enter the directory.
2. Record the Git commit under test: `git rev-parse HEAD`.
3. Install the documented host prerequisites using your distribution's trusted package sources.
4. Obtain a **project-compatible** Debian base ISO from an official source; place it in `iso/` or explicitly configure its path. Verify the checksum against the publisher's authenticated checksum information.
5. Obtain the offline artifacts required by the selected payload/profile. Verify checksums, versions, and provenance; retain downloaded originals in a reusable cache. Do not assume the current scripts can automatically populate every cache on a fresh machine.
6. Check free disk space and review build configuration. Keep source downloads separate from disposable build workspaces.

The aim is to prepare all assets while internet access is available, then build and install without unexpected network downloads. **A fully offline fresh-clone build is a goal, not yet a verified guarantee.**

## 3. Build the laptop ISO

The intended public interface is:

```bash
./builder.sh build laptop
```

**Not yet end-to-end verified:** At this checkpoint, `builder.sh` forwards a capability to the Rust engine and expects existing rootfs/payload workspaces. It does **not** yet reliably perform the entire fresh-clone preparation and laptop-profile build. The separate `build/build.sh` pipeline performs more stages but requires an explicit fresh workspace and is still being integrated with the public entry point.

Until integration is verified, **do not interpret a successful command exit or an existing ISO filename as proof that the laptop profile, payload, or boot menu was correctly assembled**. Follow repository development instructions or a verified release-specific build procedure instead. Do not run cleanup or rebuild scripts blindly against existing workspaces.

For every build, record the exact command, exit status, Git commit, profile, source ISO checksum, output ISO path/checksum, and relevant logs. Never overwrite an existing candidate ISO without deliberate authorization.

## 4. Live USB smoke test — non-destructive first

1. Verify the generated ISO checksum before writing it to a USB device.
2. Carefully identify the USB target; writing an image to the wrong device destroys data.
3. Boot the test computer from USB and look for the **Try DAIA Live** and **Install DAIA** options. These are validation targets; report missing or broken options.
4. Choose **Try DAIA Live** first. Check desktop/session startup, keyboard, display, storage visibility, Wi-Fi/Ethernet, Bluetooth where available, and basic hardware detection.
5. Record whether the machine uses UEFI or Legacy BIOS, and whether boot succeeds with its default graphics drivers. An RTX machine using Nouveau is a **future compatibility test**, not a confirmed result.

A successful live boot does not prove the installer or persistent services work.

## 5. Installation test — dedicated test disk only

After backups and target-device confirmation, exercise the Rust installer: hardware check, identity, administrator/root credentials, appliance profile, content sources, storage review, destructive confirmation, installation, and first boot. Record any unexpected navigation, partitioning, bootloader, or network behavior.

On the installed laptop, verify the expected Ollama, PostgreSQL 17/pgvector, Apache Tika, and Open WebUI functionality. **Reboot at least once**, then confirm that configured services and data recover as intended without manual container starts. Record failures and logs; persistence after a real reboot remains under test.

Do not treat successful model import, successful ISO creation, and successful post-reboot recovery as interchangeable results.

## 6. Report reproducible findings

Open a GitHub issue with:

- Git commit and exact ISO checksum
- Host hardware (CPU, RAM, GPU, storage, network adapter), firmware boot mode
- Build profile, base ISO version/checksum, offline artifact versions
- Expected versus actual behavior; precise reproduction steps
- Sanitized command output, build logs, boot errors, or relevant systemd journal excerpts
- Whether the issue occurs in Live mode, during installation, on first boot, or after reboot

**Never publish passwords, private keys, authentication tokens, personal files, or database secrets.** Review logs before attaching them.

## 7. Current verification boundaries

- The laptop payload has been assembled and its staged recovery helpers checked in development; this does not by itself validate a new ISO.
- The one-command laptop builder and fresh-clone offline bootstrap still need implementation and end-to-end verification.
- Laptop physical installation/reboot recovery and RTX/Nouveau boot compatibility remain separate hardware test milestones.
- Other Loom appliance profiles and the RTX software stack are not specified by this playbook.

See the repository [README](../../README.md) for project scope and status. This playbook will be updated as the public build interface and download/bootstrap process become reproducible.
