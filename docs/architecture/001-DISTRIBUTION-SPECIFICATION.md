# DAIA Distribution Specification

**Release:** DAIA 1.0 — Pragna
**Status:** Draft 0.1

## 1. Product Definition

DAIA is a Debian-based, offline-capable desktop distribution whose primary
purpose is to provide an accessible local AI assistant.

DAIA is a complete desktop operating system, not merely an application installer.

## 2. Base Platform

- Base operating system: Debian 13
- Architecture: amd64
- Desktop: XFCE
- Display manager: To be selected
- AI runtime: Ollama
- Primary AI interface: Open WebUI
- Container runtime: Docker Engine
- Installation method: Customized Debian Installer ISO

## 3. Required Offline Payload

The installation ISO must contain:

- Debian installer and desktop packages
- XFCE desktop dependencies
- Docker Engine packages and dependencies
- Ollama runtime
- Open WebUI container image
- One curated local AI model
- DAIA runtime scripts
- DAIA configuration
- DAIA systemd services
- Desktop launcher and branding
- Local documentation
- Validation and recovery utilities

## 4. Installation Workflow

```text
Boot DAIA ISO
      ↓
Debian Installer
      ↓
Install Debian and XFCE
      ↓
Run DAIA late-install hook
      ↓
Copy offline payload
      ↓
Install and enable first-boot service
      ↓
Reboot
