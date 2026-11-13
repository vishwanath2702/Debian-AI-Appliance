# Loom Project Charter

## Project Identity

**Loom** is the open-source, Rust-based package, capability, and appliance-building platform for purpose-built Debian operating systems.

**DAIA (Debian AI Appliance), codename Pragna**, is the first appliance being developed and validated with Loom. DAIA/Pragna is an appliance, not another name for the Loom engine.

The repository retains its historical name, `Debian-AI-Appliance`, while the shared engine and its architecture are identified as **Loom**.

## Vision

**One Loom engine. One shared installation foundation. Many independent appliances.**

Make specialized Debian appliances reproducible, accessible, secure, offline-capable, and recoverable without maintaining a separate build engine for every appliance.

## Mission

Build a modular Rust control plane that resolves declarative appliance requirements into validated plans; assembles operating systems and software; installs them on target hardware; and supports capability management throughout the installed appliance's lifecycle.

Loom should orchestrate established Debian and Linux utilities rather than unnecessarily replace them. Appliance-specific software and policies belong in declarative profiles and providers, not in hardcoded engine branches.

## Product Boundaries

### Loom — Shared Infrastructure

Loom owns the reusable appliance-building and lifecycle machinery:

- Appliance profiles, capability and provider registries, and declarative YAML definitions
- Package and dependency resolution, version/source policy, and explicit execution plans
- Build input validation, offline payload preparation, provenance, and image generation
- Hardware and storage inspection, Rust installer and wizard workflows, and Live USB support
- Post-install capability addition, removal, replacement, verification, and repair
- Explicit execution boundaries, diagnostics, and recovery/rebuild contracts

Build-time capability selection and post-install capability management should share the same authoritative planning model. Implementations may use system packages, services, containers, or other suitable providers without making any one technology mandatory across Loom appliances.

### DAIA / Pragna — First Appliance

Pragna is the local-AI appliance built with Loom. Its intended user experience is private, approachable, and useful offline through a familiar **KDE Plasma desktop**.

Pragna-specific components may include Ollama, Open WebUI, local models, Model Manager, and a RAG stack with PostgreSQL/pgvector and Apache Tika. These are **not mandatory dependencies for Loom or other appliance families**.

Pragna's AI Librarian is an envisioned Pragna-native agent for supervising document ingestion, retrieval indexes, and the Open WebUI-based RAG workflow; it is not part of Loom's generic control plane.

## Primary User Experiences

### Building and Installing an Appliance

1. Select a declarative appliance profile and desired capabilities.
2. Inspect hardware, available software sources, and dependencies.
3. Review an explicit, validated build or installation plan.
4. Build or obtain the appropriate installation/live image.
5. Try the appliance live where supported, or install it using the guided installer.
6. Verify the installed appliance and manage capabilities later.
7. Rebuild or recover the appliance without losing preserved user data and configuration.

### Using Pragna

1. Boot or install Pragna from supported media.
2. Use the KDE Plasma desktop and a local AI interface without needing to administer Docker, systemd, ports, or model services.
3. Work with locally available AI models without a mandatory cloud account.
4. Add models and, when the RAG integrations are ready, manage and query offline knowledge through supported interfaces.

These are product goals; their inclusion here does not certify that every workflow has been implemented or verified.

## Initial Delivery Scope

### Loom Foundation

The initial shared platform targets a Debian 13 foundation, reproducible and inspectable image construction, declarative capability selection, validated execution plans, hardware-aware installation, offline provisioning, and clear diagnostics. Support for capability changes after installation and full backup/restore remains a lifecycle requirement, with implementation and verification tracked separately.

### DAIA / Pragna Validation Target

The first appliance targets:

- Debian 13 with KDE Plasma, usable on CPU-only hardware
- A local AI runtime and a user-friendly graphical interface
- Model import, export, management, and benchmarking workflows
- Offline-ready software and model availability according to the chosen release profile
- Necessary hardware firmware and network/Bluetooth integration
- Local logging, service health checks, and understandable failures
- Optional RAG services based on Open WebUI, PostgreSQL 17/pgvector, and Apache Tika as integration work progresses
- No mandatory cloud account, routine network connection, or telemetry

**Status note:** Pragna is under active development and physical-laptop validation. The PostgreSQL/pgvector, Open WebUI, Tika, RAG, and AI Librarian workstreams have distinct levels of maturity; this charter does not claim they form a completed end-to-end release.

## Other Appliance Families

Loom is intended to support independent appliance profiles beyond Pragna, including the planned **Sanchari** (offline exploration), **Nidhi** (storage), **Raksha** (security and networking), **Taranga** (media), and **Sharan** (backup and disaster recovery).

These are planned appliance families, **not released products or automatically implemented capabilities**. Each should declare only the software and services it needs.

## Core Principles

### Declarative and Modular

Capability requirements, payloads, providers, and package policies are expressed through validated manifests. One authoritative resolver and planner serve the common lifecycle; shell helpers may perform narrowly scoped system operations but must not become competing policy engines.

### Offline First and Privacy First

An appliance should operate locally without mandatory cloud services. Sensitive user content remains local unless the user explicitly chooses otherwise. Offline installation and content availability must be verified for the specific release profile.

### Disposable OS, Recoverable Data

The operating system is replaceable. User data, models where applicable, configuration, and other non-reconstructible state must be preserved or restorable. Rebuild and recovery must be planned, testable operations; merely recreating an ISO is not equivalent to restoring an appliance.

### Controlled Change and Least Privilege

Changes require explicit plans, validation, and appropriate authorization. Destructive storage operations and deletion of user data require clear confirmation. Runtime agents, including Pragna's AI Librarian, must not receive unrestricted system access by default.

### Compatibility and Provenance

Use trusted, authenticated sources and record exact selected package and artifact versions. Package policies may range from Debian Stable through verified newer packages and explicitly opted-in experimental packages; implementations and compatibility must be tested before release.

### No Mandatory Stack Across Appliances

Ollama, Open WebUI, PostgreSQL, Docker, and any other Pragna-specific choice must not become compulsory for unrelated Loom appliances.

### Testability and Honest Status

Every important operation must expose validation, errors, and meaningful logs. Distinguish **planned**, **implemented**, **tested**, and **verified on hardware**. Documentation and release descriptions must not imply a capability is operational solely because source code exists.

### Open Source and Documentation

Publish inspectable, reproducible, maintainable engineering work. Record architectural decisions, implementation contracts, limitations, and verification evidence alongside the code.

## Pragna Users and Hardware Targets

Pragna aims to serve learners, homes, community organizations, and other users who want private local AI without needing to administer Linux services.

The following are **initial design targets, not verified minimum requirements for every model and workload**:

| | Minimum target | Recommended target |
|---|---|---|
| Architecture | x86-64 | x86-64 |
| CPU | 4 threads | 8 threads |
| RAM | 8 GB | 16 GB |
| Storage | 80 GB | 150 GB |
| Dedicated GPU | Not required | Optional, if supported |

The selected local model must be evaluated on CPU-only systems. RAG workloads and optional models may require higher resources.

## Success Criteria

### Loom

Loom succeeds when a supported appliance profile can be resolved into a reproducible, inspectable, verified installation; its capabilities can be managed through authorized lifecycle operations; and its data and configuration can be preserved and recovered after system replacement or failure.

### DAIA / Pragna

Pragna succeeds when a non-technical user can install it, reach the desktop, use a local AI assistant offline, manage supported models, and receive clear feedback about optional knowledge and RAG services without having to understand the underlying infrastructure.

End-to-end RAG and AI Librarian success criteria require separate integration and quality testing; neither is declared complete by this charter.

## North Star

For **Loom**, ask:

> Does this make appliance construction, installation, lifecycle management, or recovery safer, more reusable, and more predictable?

For **Pragna**, ask:

> Does this make private local AI and offline knowledge easier, safer, and more accessible to the user?

The two missions should reinforce each other without conflating the engine with its first appliance.
