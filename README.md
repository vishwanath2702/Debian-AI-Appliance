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
- **Networking — Setu:** DNS, DHCP, VPN, and network services.
- **Security — Raksha:** Monitoring, intrusion detection, and auditing.
- **Development — Srijana:** Development tools and environments.
- **Media — Taranga:** Media-focused services and workflows.
- **Backup and recovery — Smriti:** Backup, restoration, and recovery capabilities.

**Project status matters:** DAIA/Pragna is under active development and hardware testing. The other named appliance families are planned; listing them here does **not** mean their ISOs or capabilities are implemented or released.

## One Engine. Many Appliances.

Instead of hardcoding a separate operating-system builder for every purpose, Loom aims to reuse a common Rust engine, capability registry, dependency resolution, planning, installer, and hardware-support foundation.

Appliance-specific requirements should be expressed through separate declarative manifests and profiles. This lets each appliance remain distinct while sharing infrastructure where appropriate.

## Built on Debian. Powered by Rust.

Loom builds on Debian and its established package, boot, and system-management tools. Rust coordinates appliance modeling, capability selection, planning, execution, and installation; targeted system-integration scripts remain useful where needed.

The project follows an **offline-first** approach. Another core requirement is **recoverability**: the operating system should be replaceable while user data and configuration can be restored. These are design goals, not claims that every recovery workflow is already complete.

## Project Status

Loom's Rust engine, capability framework, ISO workflow, and installer have implementations in the repository and are under development and verification. DAIA/Pragna is the current physical-laptop validation platform.

Not every planned integration or appliance capability is operational. Source availability, passing tests, and successful real-hardware operation are separate verification milestones.

## Documentation

- [Product overview and architecture](docs/architecture/loom-daia-product-overview.md)
- [Capability inventory and verification status](docs/architecture/loom-daia-capability-inventory.md)
- [Appliance naming conventions](docs/architecture/appliance-naming-conventions.md)
- [Technical glossary and guarded hardware benchmark example](docs/glossary.md)

## Our Vision

**One Loom engine. One shared installation foundation. Many independent appliances.**

The goal is not another general-purpose Linux distribution, but a practical way to build reproducible, purpose-built Debian appliances.
