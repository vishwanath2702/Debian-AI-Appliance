![Loom weaving shuttle logo](branding/loom-shuttle.png)

# Loom

**Loom is a Rust-based package manager and appliance-building platform for creating purpose-built, Debian-based operating systems.**

Loom brings together appliance definitions, packages, services, configuration, and installation workflows. Its aim is to make specialized systems reproducible and easier to build, install, and maintain without creating a separate build engine for every appliance.

**Loom is the builder. The appliances are what it builds.** The first appliance under development is **DAIA (Debian AI Appliance)**, codename **Pragna**.

## What Can Loom Build?

Loom is designed to support a family of distinct appliances, each with its own purpose and declarative capability and payload definitions:

- **DAIA — Pragna (AI):** Local AI assistants, Ollama, Open WebUI, model management, and offline knowledge workflows.
- **Wanderer — Sanchari (offline exploration):** Offline maps and reference material, navigation, software-defined radio (SDR) reception, and exploration or rescue-support tools.
- **Storage — Nidhi:** File sharing, storage management, and related services.
- **Security and networking — Raksha:** A planned security assessment and OSINT (Open-Source Intelligence) appliance. OSINT involves collecting, verifying, and analyzing information from publicly available sources—such as websites, public records, news, and openly accessible online data—to support lawful research, investigations, and security analysis. Raksha also targets authorized penetration testing, vulnerability analysis, digital forensics, reverse engineering, and security research, with additional network infrastructure capabilities such as DNS, DHCP, VPN, firewall, and network services.
- **Backup, recovery, and disaster recovery — Sharan (ಶರಣ):** A planned universal recovery system for all Loom appliances, combining backup and preservation, file and configuration restoration, boot and filesystem repair, and complete appliance rebuilding. Its emergency Live USB will be as small as practical, offline-first, and independently bootable even if the installed operating system fails.

**Project status matters:** DAIA/Pragna is under active development and hardware testing. The other named appliance families are planned; listing them here does **not** mean their ISOs or capabilities are implemented or released.

**Sharan recovery principle:** Every Loom appliance should support a common backup and recovery contract. The minimal recovery ISO carries essential tools; larger backups and appliance payloads may live on external or network storage. Recovery begins with read-only inspection and requires explicit confirmation before destructive operations. Sharan absorbs the previously proposed Smriti role; Smriti is not a separate planned appliance.

## One Engine. Many Appliances.

Instead of hardcoding a separate operating-system builder for every purpose, Loom aims to reuse a common Rust engine, capability registry, dependency resolution, planning, installer, and hardware-support foundation.

Appliance-specific requirements should be expressed through separate declarative manifests and profiles. This lets each appliance remain distinct while sharing infrastructure where appropriate.

## Shared Foundations. Independent Appliances.

Loom's technology choices should complement the entire appliance family without forcing every appliance to use the same software stack. Shared capabilities should expose clear, documented interfaces, use open standards and portable data formats where practical, and remain optional when an appliance does not need them.

Appliances should operate independently of other appliances and optional shared services. Different hardware and use cases may require different inference engines, interfaces, databases, or deployment methods; Pragna's current choices (including Ollama, Open WebUI, and Docker) are not mandatory Loom-wide dependencies or predetermined choices for a future AI server. Alternatives should be evaluated through compatibility, performance, security, maintainability, and recovery testing.

## Package Version Policy

Loom aims to let each appliance select a declarative package freshness policy rather than always accepting the default Debian stable versions:

- **Stable:** Packages from Debian stable.
- **Stable + Backports:** Debian stable with selected newer backported packages.
- **Latest Compatible:** The newest versions verified to work together from trusted, authenticated repositories and supported vendor sources.
- **Experimental:** Explicitly opted-in testing or unstable packages, with clear compatibility and security warnings.

Package selection must validate dependencies, repository trust, compatibility, and appliance-specific constraints. Loom should record exact package versions and sources in a reproducible manifest for auditing, rebuilding, and recovery. These are planned policy capabilities, not a claim that all modes are implemented today.

## Built on Debian. Powered by Rust.

Loom builds on Debian and its established package, boot, and system-management tools. Rust coordinates appliance modeling, capability selection, planning, execution, and installation; targeted system-integration scripts remain useful where needed.

The project follows an **offline-first** approach. Another core requirement is **recoverability**: the operating system should be replaceable while user data and configuration can be restored. These are design goals, not claims that every recovery workflow is already complete.

## Building the DAIA Laptop ISO

Loom provides a preparation workflow and a laptop ISO build entry point.

To inspect the development host's build prerequisites and available offline assets without starting an ISO build:

```bash
bash builder.sh prepare laptop
```

To initiate the laptop ISO build workflow:

```bash
bash builder.sh build laptop
```

**Status (November 18, 2026): Preparation verified; complete ISO build and physical laptop validation pending.**

The preparation workflow checks required build tools, offline assets, integrity metadata, Debian source ISO provenance, and offline Rust dependency resolution. The verified development-desktop checkpoint completed successfully.

The build workflow uses a pinned Debian 13.7.0 (Trixie) amd64 netinst ISO. When that ISO is missing, its acquisition requires locally available signed release evidence and explicit, per-invocation user approval. Network access and automatic upgrades are not silently authorized.

Acquisition of other missing application assets is not yet implemented. Those assets must already be available and pass the applicable verification checks.

The laptop ISO build path remains subject to end-to-end validation. A successful preparation check does not establish that the resulting ISO boots, installs correctly, or runs every integrated application.

The current validation target is a CPU-compatible Debian/KDE laptop appliance, including Open WebUI, Ollama, PostgreSQL/pgvector, Apache Tika, and WhichLLM. Live USB boot, installation, networking, and application integration must be verified on physical laptop hardware.

The operating system is intended to be rebuildable, with user data and configuration recoverable independently of the installed system.

## Documentation

- [Architecture documentation index](docs/architecture/README.md)
- [Repository testing playbook — online preparation, offline assets, Live USB and installation tests](docs/testing/repository-testing-playbook.md)
- [Product overview and architecture](docs/architecture/loom-daia-product-overview.md)
- [Capability inventory and verification status](docs/architecture/loom-daia-capability-inventory.md)
- [Appliance naming conventions](docs/architecture/appliance-naming-conventions.md)
- [Technical glossary and guarded hardware benchmark example](docs/glossary.md)

## Our Vision

**One Loom engine. One shared installation foundation. Many independent appliances.**

The goal is not another general-purpose Linux distribution, but a practical way to build reproducible, purpose-built Debian appliances.

## November 23, 2026 — Pragna documentation checkpoint

Pragna is Loom's Debian 13 / KDE AI appliance. Its declared stack
includes Ollama, Open WebUI, PostgreSQL/pgvector, Apache Tika,
SearXNG, and an offline embedding-model artifact.

SearXNG firstboot and recovery integration passed 599 selected
automated tests across 51 suites. A newly built ISO has not yet
been validated on the laptop.

For a plain-English explanation of the components, see the
[Pragna software stack](docs/pragna/software-stack.md).
The [engineering census](docs/project-census.md) records build
artifacts and verification evidence.
