# Loom & DAIA
## Product Overview and Initial Capability Model

**Document version:** 0.1 — Initial Draft  
**Status:** Working architecture reference  
**Repository:** [Debian-AI-Appliance](https://github.com/vishwanath2702/Debian-AI-Appliance)

---

## 1. Introduction

**Loom is a Rust-based package manager for creating ready-to-use, Debian-based appliances.**

Loom is intended to assemble operating systems, packages, services, configurations, and capabilities into reproducible appliances.

**DAIA (Debian AI Appliance)** is the broader appliance platform and project from which Loom has developed.

The relationship can be understood as:

- **Loom:** The Rust-based control plane, package and capability management, planning, execution, and installation machinery.
- **DAIA:** The appliance platform, runtime environment, and collection of appliance capabilities built using that machinery.

This separation is an initial product definition and should be validated against the current implementation before becoming a formal architectural contract.

## 2. Core Philosophy

The system follows several foundational principles:

1. **Declarative configuration:** Appliance definitions describe desired capabilities instead of hardcoding individual appliance implementations.
2. **Modular construction:** Components can be added, replaced, and combined.
3. **Deterministic planning:** The system determines and validates the operations required before executing them.
4. **Offline-first operation:** Appliances should remain useful without continuous internet access.
5. **Clear responsibility boundaries:** Each architectural responsibility has one authoritative owner.
6. **Recoverability:** The operating system should be replaceable while user data and configuration remain recoverable.

The recoverability principle is an established project requirement; the reviewed architecture documents do not yet provide a complete implementation specification for it.

## 3. Conceptual Architecture

```text
                 User Requirements
                        |
                        v
                Appliance Profile
                        |
                        v
                  LOOM ENGINE
             +--------------------+
             | Capability Registry|
             | Dependency Resolver|
             | Execution Planner  |
             | Executor           |
             +--------------------+
                        |
                        v
               Debian Construction
                        |
                        v
                Installation Image
                        |
                        v
                 LOOM INSTALLER
                        |
                        v
               Installed Appliance
                        |
                        v
                  DAIA RUNTIME
                        |
             +----------+----------+
             |          |          |
          Services    Content    Hardware
             |          |          |
             +----------+----------+
                        |
                        v
               Operational Appliance
```

This diagram represents the intended system architecture, not a certification that every component is complete.

## 4. Loom Capability Domains

| Domain | Intended responsibility |
|---|---|
| Appliance profiles | Represent user-selected appliance types |
| Package management | Resolve and install packages required by selected capabilities |
| Capability registry | Map capabilities to available providers |
| Dependency resolution | Determine requirements and detect unsupported configurations |
| Execution planning | Generate an explicit sequence of operations |
| Execution engine | Coordinate operations and propagate errors |
| ISO construction | Produce bootable Debian appliance installation images |
| Hardware inspection | Discover hardware and expose facts to planning and installation |
| Installer | Configure and install the selected appliance |
| Storage management | Discover, validate, and manage installation storage |
| Provisioning | Coordinate configuration and acquisition of resources after installation |

The engineering blueprint identifies these as core responsibilities. Their implementation maturity must be documented individually.

## 5. DAIA Capability Domains

DAIA provides the environment in which selected appliance capabilities operate.

| Domain | Examples |
|---|---|
| Artificial intelligence | Ollama, Open WebUI, local models |
| Knowledge and retrieval | Offline knowledge repositories, retrieval services |
| Storage | File sharing, storage services, backup |
| Networking | DNS, DHCP, VPN, firewall services |
| Communications | Mail and related services |
| Private cloud | File synchronization and collaboration |
| Offline navigation | Maps, reference content and location resources |
| Security | Monitoring, auditing and access controls |

These represent the intended capability families described in the README. They must not all be treated as implemented features.

## 6. Configuration and Extensibility

The engineering blueprint defines a declarative approach based on appliance profiles, providers, and package manifests.

For example:

```yaml
name: ai-workstation
description: Local AI appliance

capabilities:
  - desktop
  - local-ai
```

This is a conceptual example, not a verified deployable manifest.

The goal is to allow new appliance types and capabilities to be introduced without changing the core engine for each new application.

## 7. Installation and Runtime Lifecycle

The intended lifecycle consists of distinct stages:

**Definition:** Identify appliance requirements and capabilities.

**Planning:** Resolve providers and dependencies, validate inputs, and generate an execution plan.

**Construction:** Assemble the Debian environment and required software.

**Installation:** Install and configure the appliance on target hardware.

**Provisioning:** Acquire additional content and configure selected services.

**Operation:** Run and manage the appliance through its installed runtime.

**Recovery:** Reconstruct the appliance while restoring preserved user data and configuration.

The exact ownership and order of installer and post-install configuration responsibilities require further reconciliation with the current Rust implementation.

## 8. Technology Boundaries

The engineering blueprint assigns structured policy and planning responsibilities to Rust.

Shell remains appropriate for narrow system-integration operations, including invoking Debian and Linux utilities.

The architecture should avoid duplicated planners, registries, or competing configuration authorities.

Debian tools such as APT, dpkg, GRUB, filesystem utilities, and ISO-generation tools are intended to be orchestrated rather than unnecessarily reimplemented.

## 9. Capability Maturity Classification

Every capability should eventually receive one of these classifications:

| Status | Meaning |
|---|---|
| Implemented | Production implementation exists |
| Verified | Implementation has passed defined tests |
| Experimental | Implementation exists but needs further validation |
| In development | Work is actively incomplete |
| Planned | Documented intention without confirmed implementation |
| Deferred | Intentionally postponed |

No individual capability is assigned a definitive maturity status in this initial document. That requires an implementation inventory and supporting test evidence.

## 10. Documentation Roadmap

The next documents should describe:

1. Loom architecture and internal components.
2. Appliance profiles, providers, and declarative package manifests.
3. Rust installer and wizard lifecycle.
4. DAIA runtime, service management, and provisioning.
5. AI capabilities, Model Manager, Ollama, Open WebUI, and retrieval components.
6. Storage, backup, restore, and disaster recovery.
7. Capability implementation status and verification evidence.
8. Developer workflow, testing, packaging, and release procedures.

## 11. Source Documents

This draft is based primarily on:

- [Project README](https://github.com/vishwanath2702/Debian-AI-Appliance/blob/master/README.md)
- [Engineering Blueprint](https://github.com/vishwanath2702/Debian-AI-Appliance/blob/master/docs/architecture/engineering-blueprint.md)
- [DAIA Architecture](https://github.com/vishwanath2702/Debian-AI-Appliance/blob/master/docs/architecture.md)

**Documentation note:** This is an initial conceptual baseline. It does not replace an implementation audit, and it does not assert that every described feature is operational.
