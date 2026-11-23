# Pragna software stack

Pragna is a Debian-based, local-first AI workstation appliance
built using Loom.

This document describes the intended software architecture.
Availability in a build does not necessarily mean that the
component has passed physical laptop integration testing.

## Operating system and desktop

### Debian 13 (Trixie)
Debian is the Linux operating system on which Pragna is built.
It supplies the core system, security updates, package repositories,
and compatibility foundation for the appliance.

### KDE Plasma
KDE Plasma provides Pragna's graphical desktop. It gives users
windows, application launchers, system settings, and a familiar
environment for everyday computing and AI applications.

### NetworkManager and Plasma NM
NetworkManager manages wired and wireless network connections.
Plasma NM integrates those controls into KDE so users can select
Wi-Fi networks and manage connections from the desktop.

### Debian firmware packages
Firmware provides low-level code needed by supported hardware,
including some Wi-Fi and Bluetooth devices. Pragna includes
firmware support to improve hardware compatibility, although
individual devices still require testing.

### APT and Debian backports
APT installs, updates, and manages Debian software packages.
Backports can provide selected newer packages while retaining
Debian stable as the operating-system foundation. Availability
and use of backports depend on the build configuration.

### SDDM
SDDM is the graphical login manager used with KDE Plasma.
It presents the login screen and starts the selected desktop
session.

## Appliance construction and installation

### Loom
Loom is the Rust-based appliance builder. It combines package
definitions, configuration, services, installation workflows,
and reusable capabilities to construct Pragna and other planned
Debian appliances.

### Rust
Rust is the programming language used for Loom's engine and
installation wizard. Its type and memory-safety features support
the development of reliable system-management software.

### Ratatui 0.29
Ratatui is the terminal user-interface library used by the
installation wizard. It renders interactive screens without
requiring a graphical desktop to be running.

### Crossterm 0.28
Crossterm handles terminal input and interaction for the
installer, including keyboard events and terminal behavior.
It works alongside Ratatui.

### Loom TUI crate
This project component implements the installation wizard's
screens, navigation, and user interaction. It is internal Loom
software rather than a separate third-party application.

### Loom application crate
The application crate coordinates installer state and
configuration, connecting user decisions with the underlying
installation workflow.

### Loom model crate
The model crate defines the data structures used to represent
appliance identity, configuration, and other installation inputs.

### Loom installation engine
The installation engine plans and executes the work needed
to install the selected appliance. Its implementation and
verification are separate from the terminal interface.

## Local AI and model management

### Ollama
Ollama runs AI models locally on Pragna. It supplies model
inference to applications such as Open WebUI and can operate
without a cloud AI service when the required models are present.

### Open WebUI
Open WebUI is the browser-based interface for interacting with
local AI models. It supports conversations and document-related
workflows and connects to services such as Ollama and Pragna's
retrieval infrastructure.

### DAIA Model Manager
Model Manager is Pragna's graphical tool for working with local
AI models. Its development includes importing, inventory
management, and benchmarking so users can manage models without
relying entirely on terminal commands.

### WhichLLM
WhichLLM helps assess whether AI models are suitable for the
available hardware. Its role is advisory: users retain control
over model selection rather than having models chosen
automatically.

### GGUF
GGUF is a model-file format, not a running service. It is
important to Pragna because the model importer supports
portable GGUF model files for local use and transfer.

## Documents, knowledge retrieval, and web search

### PostgreSQL 17
PostgreSQL is a relational database system. Pragna uses it as
persistent storage for its document-retrieval infrastructure,
rather than treating all knowledge data as disposable
container state.

### pgvector
pgvector is a PostgreSQL extension that stores and searches
numerical vectors. It supports similarity searches used to
find document passages related to a user's question.

### Apache Tika 3.3.3.0
Apache Tika extracts text and metadata from documents.
It helps turn supported document formats into content that
can be processed for indexing and retrieval.

### nomic-embed-text
nomic-embed-text is an embedding model supplied as an offline
Ollama artifact. It converts text into numerical representations
that can be compared during semantic document retrieval.

### SearXNG
SearXNG is a metasearch engine that gathers web-search results
from search providers. Pragna uses it for web-search capability
through a separately controlled service, keeping that path
distinct from private document retrieval.

The currently tested web-search arrangement uses result
snippets. A secure full-page fetching proxy is deferred.

## Container and system services

### Docker
Docker runs application services in containers. Pragna uses
containerized deployment for components such as Open WebUI,
PostgreSQL/pgvector, Apache Tika, and SearXNG, with controlled
network and storage configuration.

### containerd
containerd is part of the container runtime infrastructure
used by Docker. It helps manage the underlying container
execution lifecycle.

### systemd
systemd manages Linux services and their startup ordering.
Pragna uses it to control services, firstboot operations,
and recovery-related dependencies.

### Firstboot runtime modules
These are Pragna's own initialization programs rather than
a third-party product. They configure and register managed
services after installation and enforce startup prerequisites.

### RAG recovery services
Pragna's recovery helpers check registered service identities
and required dependencies before restoring managed RAG
services. Their purpose is to recover safely without blindly
trusting arbitrary containers.

## How the software works together

For local AI conversations, Open WebUI provides the interface
and Ollama runs the selected model.

For document questions, Apache Tika extracts text, an embedding
model converts text into vectors, and PostgreSQL with pgvector
supports similarity retrieval. The retrieved information can
then help the AI model answer the question.

For web questions, SearXNG supplies a separate search path.
Private retrieval and external web search must retain their
respective security and network boundaries.

Loom builds and installs the appliance, while Debian, KDE,
Docker, and systemd provide its operating environment.

## Declared versions and verification

The Pragna configuration declares Debian 13 on amd64 with KDE,
Open WebUI `v0.13.0-slim`, pgvector `pg17`, Apache Tika
`3.3.3.0`, a pinned SearXNG image, and an offline
nomic-embed-text artifact.

Other exact versions must be verified from their respective
package definitions or installed-system evidence.

The SearXNG integration checkpoint `cb27b8df` passed
599 selected tests across 51 suites. That is repository-level
verification, not proof of complete operation after installing
a newly built ISO on the physical laptop.

This document explains the declared application and platform
stack. It is not a complete installed-system package inventory.

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
