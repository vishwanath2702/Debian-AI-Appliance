# Loom / DAIA Repository Testing Playbook

**Status: active development — November 18, 2026.** This is a tester-oriented guide, not a released or fully validated installation procedure. The first appliance is DAIA/Pragna. Other Loom appliance families are planned.

## 1. What you need

- An **amd64** Linux development host with sufficient disk space, a USB drive or spare test disk, and any network access needed to obtain approved missing assets.
- Git, Rust/Cargo, and Debian ISO/build utilities required by the repository. Check the actual host and asset prerequisites before building:

  ```bash
  bash builder.sh prepare laptop
  ```

- The **pinned Debian 13.7.0 (Trixie) amd64 netinst ISO** and its signed release evidence. The November 18 development-desktop checkpoint verified the locally available source ISO's checksum and Debian signature provenance; do not substitute an arbitrary matching filename.
- Separately available offline payload artifacts, including the required Ollama archive, Open WebUI, PostgreSQL/pgvector and Apache Tika images, and WhichLLM components. A Git clone alone does **not** supply all large artifacts.

**Safety:** Use a dedicated test disk or VM. The installer can erase selected storage. Back up important data and confirm the target disk before any destructive step. Never test installation on your only copy of important data.

## 2. Online preparation and offline asset verification

1. Clone the repository: `git clone https://github.com/vishwanath2702/Debian-AI-Appliance.git` and enter the directory.
2. Record the exact commit: `git rev-parse HEAD`.
3. Run `bash builder.sh prepare laptop` to inspect required tools, source ISO, offline payload assets, and relevant verification results **without building an ISO**.
4. Supply missing host tools or offline artifacts through trusted, independently verified sources. Review the preparation report before deciding whether to acquire anything.
5. The laptop build entry point uses a pinned Debian 13.7.0 source. If that ISO is absent, the implemented Debian acquisition gate requires signed release evidence and **explicit consent for that invocation**. Do not assume the command may download other missing artifacts: their automatic acquisition is not yet implemented.
6. Preserve verified downloads and evidence for reuse. Keep disposable build workspaces separate, and review available disk space before any build.

The November 18, 2026 preparation checkpoint **passed on the development desktop**: host prerequisites, pinned Debian source verification, offline Rust resolution, and required local payload asset checks. This is not a fresh-clone or laptop-hardware certification, and it does not establish a fully offline build guarantee.

## 3. Build the laptop ISO

After reviewing preparation results and obtaining **separate explicit approval for the build**, the intended public entry point is:

```bash
bash builder.sh build laptop
```

The builder now includes a laptop preparation and pinned Debian source acquisition gate. **The complete build path has not yet been validated end to end at this checkpoint.** Do not treat successful preparation, a successful command exit, or an existing ISO filename as proof that the intended laptop profile, payload, boot menu, and installer are correct.

For each build, capture visible timestamped stage progress in the invoking terminal, with contemporaneous logs where practical. Record the exact command, exit status, Git commit, profile, source ISO checksum, output ISO path and SHA-256 checksum, and relevant build logs. Do not silently download artifacts, install host packages, or overwrite existing ISO candidates without deliberate authorization.

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

- The November 18 development-desktop preparation and pinned Debian source verification passed; the scoped changes were committed and pushed as `162a59a4`.
- The public laptop builder now has preparation and consent-gated pinned Debian acquisition, but **a new complete ISO build is not yet verified**.
- Automatic acquisition of other missing offline application artifacts is not implemented; verified local copies are still required.
- Physical laptop Live USB boot, installation, application integration, and post-reboot persistence/recovery remain separate test milestones.
- RTX/Nouveau compatibility and other Loom appliance profiles are outside the confirmed laptop validation scope.

See the repository [README](../../README.md) for project scope and status. Update this playbook as the public build interface and physical laptop test results are verified.
