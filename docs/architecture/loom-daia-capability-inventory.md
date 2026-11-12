# Loom & DAIA — Capability Inventory

**Version:** 0.1  
**Status:** Provisional, source-reviewed draft  
**Evidence baseline:** GitHub `master`, reviewed November 12, 2026  
**Purpose:** Establish an evidence-based baseline of implemented, incomplete, and planned capabilities.

## 1. Scope

This inventory distinguishes:

- **Source confirmed:** Implementation exists in the GitHub repository.
- **Verification pending:** Current passing test evidence has not been established in this review.
- **Deferred:** Activation or delivery is explicitly postponed.
- **Planned:** Architectural intent without implementation evidence from the files reviewed.

Source presence does not establish that a feature works end to end. No new tests were executed for this inventory.

## 2. Loom — Core Engine

| Capability | Evidence | Preliminary status |
|---|---|---|
| Rust workspace | `engine/Cargo.toml` | Source confirmed |
| Domain model | `engine/crates/model/` | Workspace confirmed |
| Capability registry | `engine/crates/registry/src/lib.rs` | Source confirmed |
| Provider resolution | `engine/crates/resolver/` | Workspace confirmed |
| Execution planning | `engine/crates/planner/src/lib.rs` | Source confirmed |
| Execution | `engine/crates/executor/` | Workspace confirmed |
| Hardware and media inspection | `engine/crates/inspector/src/lib.rs` | Source confirmed |
| CLI | `engine/crates/cli/src/main.rs` | Source confirmed |
| TUI wizard | `engine/crates/tui/src/main.rs` | Source confirmed |
| ISO workflow | `engine/crates/engine/src/workflow/iso.rs` | Source confirmed; build validation pending |

The planner source explicitly constructs plans using resolved providers and preserves declared capability order when planning an appliance profile.

## 3. Loom — Installer

The TUI source defines screens for welcome, localization, hardware checks, appliance identity, credentials, profile selection, content repositories, external content, storage, review, installation confirmation, installation progress, completion and failure.

**Assessment:** Installer and wizard implementation exists. This review has not established complete screen behavior, successful physical installation, or recovery guarantees.

## 4. DAIA — Runtime and Provisioning

| Capability | Evidence | Preliminary status |
|---|---|---|
| Runtime module dispatch | `runtime/runtime.sh` | Source confirmed |
| Validate/install/verify lifecycle | `runtime/runtime.sh` | Source confirmed |
| First-boot orchestration | `runtime/firstboot.sh` | Source confirmed |
| Runtime configuration flags | `payload/config/pragna.conf` | Source confirmed |
| Offline payload assembly | `build/build-payload.sh` | Source confirmed; release validation pending |

The first-boot source invokes the runtime orchestrator and model-realization command. Successful end-to-end provisioning is not established by this inspection.

## 5. DAIA — AI Capabilities

| Capability | Evidence | Preliminary status |
|---|---|---|
| Ollama integration | Runtime selection and Model Manager inventory | Source confirmed; integration validation pending |
| Graphical Model Manager | `runtime/daia-model-manager.py` | Source confirmed |
| Installed-model benchmarking | Model Manager benchmark controls | Source confirmed; test evidence pending |
| PostgreSQL 17/pgvector | Runtime module, authorization helper, systemd service | Source confirmed; activation deferred |
| Open WebUI | Runtime selection flag | Integration evidence incomplete |
| Model import/export | CLI inspection and architecture references | Full implementation inventory pending |
| Tika and retrieval services | Prior project scope | Repository verification pending |
| WhichLLM | Prior project scope | Repository verification pending |

The pgvector startup helper uses authorization before starting a container by immutable ID. The service file explicitly states that activation is deferred pending verification of security and persistence contracts.

## 6. Declarative Capability Architecture

The source review confirms repository-loading interfaces for appliance profiles, providers, package manifests and content repositories.

The engineering blueprint defines declarative configuration as a central architectural requirement.

**Outstanding verification:** Actual YAML schemas, package dependencies, validation behavior, provider coverage and whether all payloads follow the same declarative contract.

## 7. Storage, Recovery and Rebuildability

The architecture requires storage discovery, content management and reproducible appliance construction.

A project-level requirement is that the operating system remain replaceable while user data and configuration can be recovered.

**Assessment:** Architecture established; comprehensive backup, restore and disaster-recovery implementation verification pending.

## 8. Documentation Inconsistencies

1. The README defines Loom as a Rust-based package manager while older documents describe DAIA as the primary engine.
2. The runtime configuration retains the name *Debian AI Assistant*, whereas the current project documentation uses *Debian AI Appliance*.
3. The older architecture document describes a shell-centered build system, while the engineering blueprint and current repository include a Rust control plane.
4. The architecture documents do not consistently distinguish implemented features from future appliance categories.

These are documentation alignment issues; they are not proof of runtime defects.

## 9. Evidence and Verification Backlog

The next source review should establish:

1. Rust workspace test coverage and current CI results.
2. Installer tests and hardware installation evidence.
3. Provider and package-manifest inventories.
4. Model Importer and Model Exporter source and tests.
5. Ollama, WhichLLM and Model Manager integration.
6. PostgreSQL/pgvector startup, security and persistence verification.
7. Open WebUI, Tika and retrieval integration.
8. Backup, restore and rebuild contracts.
9. Bootable ISO and Live USB verification.
10. Differences between GitHub and the local uncommitted working tree.

## 10. Source References

- [Rust workspace](https://github.com/vishwanath2702/Debian-AI-Appliance/blob/master/engine/Cargo.toml)
- [Rust planner](https://github.com/vishwanath2702/Debian-AI-Appliance/blob/master/engine/crates/planner/src/lib.rs)
- [TUI wizard](https://github.com/vishwanath2702/Debian-AI-Appliance/blob/master/engine/crates/tui/src/main.rs)
- [ISO workflow](https://github.com/vishwanath2702/Debian-AI-Appliance/blob/master/engine/crates/engine/src/workflow/iso.rs)
- [Runtime lifecycle](https://github.com/vishwanath2702/Debian-AI-Appliance/blob/master/runtime/runtime.sh)
- [First-boot lifecycle](https://github.com/vishwanath2702/Debian-AI-Appliance/blob/master/runtime/firstboot.sh)
- [Model Manager](https://github.com/vishwanath2702/Debian-AI-Appliance/blob/master/runtime/daia-model-manager.py)
- [pgvector service](https://github.com/vishwanath2702/Debian-AI-Appliance/blob/master/runtime/services/daia-pgvector.service)
- [Engineering Blueprint](https://github.com/vishwanath2702/Debian-AI-Appliance/blob/master/docs/architecture/engineering-blueprint.md)

**Conclusion:** Loom and DAIA have substantial source-level implementation across construction, installation and runtime management. A complete capability maturity assessment requires further evidence, particularly tests, physical installation results and operational integration checks.
