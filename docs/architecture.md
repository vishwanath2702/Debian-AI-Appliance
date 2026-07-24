# DAIA Architecture

## Overview

DAIA (Debian AI Appliance) is an offline-first AI appliance built on Debian.

The project consists of two major parts:

1. Build System
2. Runtime Framework

---

# Build System

The build system is responsible for creating the installation ISO.

```
Developer
    │
    ▼
check.sh
    │
clean.sh
    │
extract.sh
    │
inject.sh
    │
verify.sh
    │
build.sh
    │
rebuild.sh
    │
    ▼
DAIA ISO
```

The build pipeline must always fail early when validation fails.

---

# Runtime

After installation:

systemd

↓

daia-firstboot.service

↓

install.sh

↓

bootstrap.sh

↓

DAIA Modules

---

# Runtime Modules

bootstrap.sh contains no business logic.

It only coordinates modules.

```
bootstrap.sh

↓

Hardware

↓

Resources

↓

Configuration

↓

Packages

↓

Models

↓

Plugins

↓

Cleanup
```

---

# Project Layout

```
Debian-AI-Appliance/

build/

docs/

installer/

tests/

iso/

output/

work/
```

---

# Installer Payload

```
/opt/daia

install.sh

bootstrap.sh

config/

lib/

ai/

plugins/

share/
```

---

# Library Rules

Libraries never

- print to console
- write logs
- ask questions

Libraries only

- return information
- perform work
- return success/failure

---

# Hardware Layer

Hardware module responsibilities

Collect

Evaluate

Recommend

Export

No installation decisions are made here.

---

# Resource Layer

Responsible for

- detecting external drives

- verifying checksum

- locating offline models

- locating offline packages

---

# AI Layer

Responsible only for AI software.

Examples

Ollama

Open WebUI

CUDA

ROCm

Model installation

---

# Plugin Layer

Each plugin is self-contained.

```
plugin/

manifest.yaml

install.sh

remove.sh

files/
```

Plugins should never modify core DAIA files directly.

---

# Logging

System logs

```
/var/log/daia/
```

Runtime state

```
/var/lib/daia/
```

Configuration

```
/etc/daia/
```

Application

```
/opt/daia/
```

---

# Development Philosophy

Fail early.

Keep modules small.

One responsibility per module.

No duplicated logic.

No hard-coded values.

Offline first.

Everything testable.

Everything replaceable.
