# DAIA (Debian AI Appliance)

DAIA is a declarative build system for creating reproducible, Debian-based AI appliances.

Rather than manually installing packages, configuring services, resolving dependencies, and assembling AI software stacks, DAIA allows you to describe the capabilities you want. It transforms those high-level capability definitions into complete Debian-based AI systems, from a minimal installation to a ready-to-use AI workstation or inference server.

## Vision

DAIA is built around one guiding principle:

> **Build AI appliances, not generic Linux systems.**

Many excellent tools already exist for general-purpose server provisioning and configuration management. DAIA focuses exclusively on building reproducible AI platforms.

Its purpose is to understand AI software stacks, hardware requirements, runtime dependencies, and system configuration so that users can build complete AI environments without manually researching compatibility between operating systems, GPU drivers, CUDA versions, machine learning frameworks, and AI services.

## Design Goals

* Declarative capability-based builds
* Reproducible Debian AI appliances
* Modular architecture with clear separation of responsibilities
* Extensible provider and capability model
* Automated dependency resolution
* Automated package and service provisioning
* Hardware-aware AI runtime configuration
* Clean, testable Rust implementation

## Long-Term Goals

DAIA aims to support building a wide variety of AI systems, including:

* AI workstations
* AI inference servers
* Local LLM appliances
* GPU compute nodes
* AI development environments
* Virtual machine images
* Cloud images

* [![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/vishwanath2702/Debian-AI-Appliance)
* Bootable AI appliance images

Future capabilities will include AI runtimes, model serving, GPU acceleration, notebooks, web interfaces, and complete AI software stacks while remaining focused on one mission:

> **Creating reproducible Debian-based AI appliances.**
