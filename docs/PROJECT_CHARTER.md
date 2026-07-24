# DAIA Project Charter

## Project

**DAIA — Debian AI Assistant**

## Release

**DAIA 1.0 — Pragna**

## Vision

Make private, offline artificial intelligence accessible through a familiar,
easy-to-use Linux desktop operating system.

## Mission

Build an open-source Debian-based distribution that provides a complete local
AI assistant through an XFCE desktop, requiring minimal technical knowledge
and no internet connection for normal operation.

## Primary User Experience

1. Download the DAIA ISO.
2. Install DAIA.
3. Log in to the XFCE desktop.
4. Open **DAIA AI Assistant**.
5. Begin using a local AI model.

The user should not need to understand Docker, containers, ports, systemd,
Ollama configuration, or Linux administration.

## DAIA 1.0 Scope

DAIA 1.0 will provide:

- Debian 13 base operating system
- XFCE desktop environment
- Offline installation
- Ollama local AI runtime
- Open WebUI
- One curated CPU-compatible AI model
- Desktop launcher for the AI assistant
- Automatic first-boot configuration
- Local logging and health validation
- No mandatory cloud account
- No mandatory internet connection
- No telemetry

## Out of Scope for DAIA 1.0

The following are deferred to later releases:

- Office and education software collections
- Multiple bundled AI models
- Cluster support
- Enterprise administration
- Remote fleet management
- Plugin marketplace
- Advanced GPU optimization
- Automatic online updates
- General-purpose developer workstation tooling

## Core Principles

### Offline First

A newly installed DAIA system must provide a functioning local AI assistant
without downloading additional software or models.

### Privacy First

User prompts, responses, models, and files remain on the local computer unless
the user explicitly chooses otherwise.

### Familiar Desktop

XFCE will provide a conventional desktop experience with a menu, panel,
desktop icons, file manager, settings and browser.

### Zero-Touch AI Setup

The AI stack should be installed and configured automatically.

### Open Source

DAIA must be inspectable, reproducible, modifiable and redistributable.

### Modular Engineering

The build system, payload system and runtime initialization must be divided
into independently testable components.

### Reliability

Every important operation must provide validation, logging and clear failure
reporting.

### Documentation First

Architectural decisions and implementation contracts must be recorded alongside
the code.

## North Star

When evaluating a feature, ask:

> Does this make local AI easier, safer or more accessible for the user?

Features that do not directly support that goal should be deferred.

## Target Users

DAIA is intended for:

- First-time Linux users
- Students and teachers
- Community learning centres
- Rural and low-connectivity environments
- Home users wanting private local AI
- Small organizations needing an offline assistant

## Initial Hardware Target

### Minimum

- 64-bit x86 processor
- 4 CPU threads
- 8 GB RAM
- 80 GB storage
- No dedicated GPU required

### Recommended

- 8 CPU threads
- 16 GB RAM
- 150 GB storage
- Optional supported GPU

The bundled model must remain usable on CPU-only systems.

## Success Criteria for DAIA 1.0

DAIA 1.0 is successful when a non-technical user can:

1. Install DAIA from the ISO.
2. Log in to XFCE.
3. Click the DAIA Assistant launcher.
4. Interact with the bundled local AI model.
5. Complete the entire process without internet access or terminal commands.
