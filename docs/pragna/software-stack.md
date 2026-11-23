# Pragna software stack

Pragna is a Debian-based, local-first AI workstation appliance
built using Loom.

This document describes the intended software architecture.
Availability in a build does not necessarily mean that the
component has passed physical laptop integration testing.

## Operating system and desktop

| Component | Role |
| --- | --- |
| Debian 13 (Trixie) | Operating system foundation |
| KDE Plasma | Graphical desktop environment |
| NetworkManager / Plasma NM | Network connectivity |
| Debian firmware packages | Hardware and wireless support |
| APT and Debian backports | Package management and updates |

## Installation wizard

| Component | Role |
| --- | --- |
| Rust | Implementation language for the installation wizard and installer engine |
| Ratatui 0.29 | Terminal-based installation wizard interface |
| Crossterm 0.28 | Terminal interaction and keyboard input |
| Loom / DAIA TUI crate | Wizard screens, navigation and input handling |
| Loom application crate | Installer application state and configuration |
| Loom model crate | Appliance identity and configuration models |
| Loom installation engine | Installation planning and execution |

The installer uses a keyboard-driven terminal interface rather
than a GTK or Qt graphical wizard. Its Rust implementation is
separate from the KDE Plasma desktop environment.

## AI inference and model management

| Component | Role |
| --- | --- |
| Ollama | Local language-model inference |
| Open WebUI | Browser-based AI interface |
| DAIA Model Manager | Local model management and importing |
| WhichLLM | Hardware-aware model selection support |
| GGUF | Portable model format supported by the importer |

Model benchmarking and model portability are part of the
Pragna appliance design.

## Document processing and RAG

| Component | Role |
| --- | --- |
| PostgreSQL 17 | Persistent database |
| pgvector | Vector similarity search |
| Apache Tika | Document text and metadata extraction |
| Open WebUI Slim | User-facing document and AI interaction |

The intended integration supports document ingestion,
retrieval-augmented generation and local inference.

End-to-end document ingestion and retrieval must be verified
on the installed appliance.

## Appliance runtime

| Component | Role |
| --- | --- |
| Docker | Containerized application services |
| systemd | Service lifecycle management |
| Loom / DAIA Rust engine | Appliance build and installation |
| Firstboot runtime modules | Provisioning and registration |

Pragna uses guarded provisioning and recovery boundaries
for managed services.

## Persistence and recovery

Pragna follows a disposable-operating-system design:

- The operating system can be reinstalled or rebuilt.
- User data, models and configuration should remain recoverable.
- Backup and restore are appliance-level requirements.
- Recovery must preserve trusted service identities and data.

These are architectural requirements, not a declaration
that all recovery scenarios have passed physical testing.

## Hardware integration

Pragna targets hardware-aware operation, including:

- CPU-based inference and supported GPU acceleration.
- Wi-Fi, Ethernet and Bluetooth integration.
- Recognition of compatible removable adapters.
- Discovery of external model sources and storage devices.

Support varies by hardware and must be validated on the
target appliance.

## Implementation and validation status

The current development checkpoint includes:

- Debian-based ISO and Rust installer infrastructure.
- KDE desktop and network-management integration.
- Ollama and model-management integration work.
- Offline payload staging for PostgreSQL/pgvector,
  Apache Tika and Open WebUI.
- Guarded service provisioning and recovery logic.

The Tika lifecycle correction was verified by 38 targeted
regression tests and committed as `90d8b5ef`.

### Physical laptop validation pending

The next Pragna ISO must be installed and checked for:

1. UEFI and Legacy BIOS boot behavior.
2. Desktop, networking and hardware support.
3. PostgreSQL 17 and pgvector initialization.
4. Apache Tika startup and availability.
5. Open WebUI accessibility at `http://localhost:3000`.
6. Ollama model inference.
7. End-to-end document ingestion and vector retrieval.
8. Persistence and recovery behavior.

Passing source-level tests does not replace these checks.

## Project

See the [main README](../../README.md) for Loom and
its appliance ecosystem.

## November 23, 2026 — Pragna documentation checkpoint

Pragna combines a Debian desktop with local AI, document retrieval,
and separately controlled web search. The descriptions below explain
each component's purpose. Declared configuration does not by itself
prove successful deployment on a newly installed laptop.

### Operating system and appliance construction

| Software | Purpose in Pragna |
| --- | --- |
| **Debian 13** | Provides the Linux operating system, packages, and stable base for the appliance. |
| **KDE Plasma** | Provides the graphical desktop, system settings, and everyday user interface. |
| **Loom** | Builds and installs Pragna from appliance definitions and reusable installation components. |
| **systemd** | Manages system services, firstboot initialization, and recovery startup. |
| **NetworkManager** | Manages wired and wireless network connections on supported hardware. |

### Local AI and model management

| Software | Purpose in Pragna |
| --- | --- |
| **Ollama** | Runs local language and embedding models, allowing inference without a cloud AI service. |
| **Open WebUI** | Provides the browser-based interface for chatting with models and working with documents. |
| **Model Manager** | Provides graphical workflows for importing, managing, and benchmarking local models. |
| **WhichLLM** | Helps assess which models suit the available hardware; model selection remains user-controlled. |

### Documents, knowledge retrieval, and web search

| Software | Purpose in Pragna |
| --- | --- |
| **PostgreSQL 17** | Provides persistent database storage for the document-retrieval infrastructure. |
| **pgvector** | Extends PostgreSQL with vector similarity search for finding semantically relevant information. |
| **Apache Tika 3.3.3.0** | Extracts text and metadata from documents before they enter retrieval workflows. |
| **nomic-embed-text** | Converts text into numerical embeddings used to compare document meaning. |
| **SearXNG** | Provides web-search results through a separately controlled service, without merging web access into the internal RAG network. |
| **Docker** | Runs and isolates containerized components such as Open WebUI, PostgreSQL/pgvector, Tika, and SearXNG. |

### How the components work together

For document-based questions, Tika extracts document text, an
embedding model represents that text numerically, and pgvector
supports similarity retrieval from PostgreSQL. Open WebUI presents
the interaction while Ollama runs local AI models.

For web-based questions, SearXNG provides a separate search path.
Web search and private document retrieval have different trust and
network boundaries. The secure full-page web-fetch proxy remains
deferred; the working configuration uses search-result snippets.

### Versions and verification

The declared build configuration targets Debian 13, amd64, KDE,
Open WebUI `v0.13.0-slim`, pgvector `pg17`, Tika `3.3.3.0`,
and a pinned SearXNG image. Exact versions of other components
should be confirmed from package manifests or installed-system
evidence rather than assumed.

SearXNG firstboot provisioning and registered recovery are
implemented. The selected integration verification passed
599 tests across 51 suites. Fresh ISO construction, USB boot,
installation, and complete laptop validation remain outstanding.

This is a **declared software-stack description**, not a certified
installed-system software bill of materials.
