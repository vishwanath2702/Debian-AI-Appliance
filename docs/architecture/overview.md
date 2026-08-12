# DAIA Architecture Overview

## Purpose

Debian AI Appliance (DAIA) is a system for building purpose-specific Debian appliances from declarative capability definitions.

An appliance describes what a system should be capable of doing rather than defining a large static collection of files that must be embedded directly into an installation image.

DAIA separates:

* appliance definition;
* capability resolution;
* execution planning;
* operating-system construction;
* installation;
* post-install provisioning;
* content acquisition;
* storage selection;
* localization;
* offline operation.

This separation allows DAIA to produce relatively small installation images while supporting appliances that may ultimately contain very large local datasets and services.

## System Lifecycle

The high-level DAIA lifecycle is:

```text
Appliance Profile
        │
        ▼
Capabilities
        │
        ▼
Provider Registry
        │
        ▼
Resolver
        │
        ▼
Planner
        │
        ▼
Engine
        │
        ▼
Executor
        │
        ▼
ISO Backend
        │
        ▼
Minimal DAIA ISO
        │
        ▼
Installation
        │
        ▼
DAIA Runtime / Wizard
        │
        ├── localization
        ├── storage
        ├── content
        └── services
        │
        ▼
Configured Appliance
        │
        ▼
Offline Operation
```

Build-time and post-install provisioning are intentionally separate phases.

## Declarative Appliance Model

An appliance profile describes a type of appliance through capabilities.

For example, a future `wanderer` appliance may require capabilities associated with:

* offline maps;
* offline knowledge;
* local AI.

A storage appliance may require capabilities associated with local storage and services such as Nextcloud.

Other appliance profiles may represent:

* mail servers;
* container hosts;
* Kubernetes systems;
* developer systems;
* educational systems.

The appliance profile should describe required capabilities rather than contain the implementation of those capabilities.

Conceptually:

```text
Appliance Profile
        │
        ├── capability A
        ├── capability B
        └── capability C
```

This allows capability implementations to evolve independently from appliance definitions.

## Build-Time Architecture

DAIA's build-time architecture converts declarative appliance requirements into executable plans.

The principal flow is:

```text
ApplianceProfile
        │
        ▼
Registry
        │
        ▼
Resolver
        │
        ▼
Planner
        │
        ▼
Engine
        │
        ▼
Executor
        │
        ▼
Image construction
```

### Registry

Registries contain declarative definitions used by the engine.

These include concepts such as:

* appliance profiles;
* providers;
* package manifests;
* assets;
* future content and provisioning definitions.

### Resolver

The resolver determines which provider can satisfy a requested capability.

### Planner

The planner converts resolved capabilities into execution plans.

An appliance profile containing multiple capabilities therefore produces the plans necessary to satisfy those capabilities.

### Engine

The engine coordinates planning and execution.

It acts as the primary orchestration boundary between declarative DAIA policy and the lower-level mechanisms that modify or construct a Debian system.

### Executor

The execution layer performs planned operations.

Execution may use Rust implementations, external Debian/Linux utilities, or carefully scoped shell helpers where appropriate.

## Minimal ISO Principle

DAIA installation images should remain as small as reasonably possible.

The ISO should contain what is necessary to:

* boot;
* install Debian;
* provide the DAIA runtime;
* provide the software required for the selected appliance capabilities;
* perform post-install provisioning.

Large data resources should normally not be embedded directly into the ISO.

Examples include:

* AI model weights;
* Wikipedia datasets;
* offline map datasets;
* large documentation collections;
* user content;
* large application data;
* large container images.

This prevents appliance images from becoming unnecessarily large and avoids producing separate enormous images for every combination of content, language, region, and storage configuration.

## Installed Runtime

Installation does not necessarily represent the final configured state of a DAIA appliance.

After installation, the DAIA runtime can complete appliance-specific provisioning.

This allows the installation image to contain the **capability to acquire and configure resources** without necessarily containing the resources themselves.

Conceptually:

```text
ISO
 │
 │ contains capability
 ▼
Installed DAIA
 │
 │ provisions resources
 ▼
Configured appliance
```

## Post-Install Provisioning

Post-install provisioning allows the system to adapt to the user, location, hardware, available storage, and intended use.

The provisioning process may configure:

* appliance resources;
* storage locations;
* local language;
* locale;
* regional datasets;
* AI models;
* knowledge repositories;
* maps;
* application services.

A Wizard or equivalent management interface can guide this process.

Choices that do not need to be fixed during ISO construction should generally remain configurable after installation.

## Content Architecture

DAIA distinguishes software capability from potentially large content resources.

A content repository represents a logical collection of content.

Examples may include:

```text
wikipedia
maps
ai-models
documentation
educational-content
```

A content source describes where content for such a repository can be acquired.

Conceptually:

```text
ContentRepository
        │
        ▼
ContentSource
        │
        ▼
Acquire
        │
        ▼
Verify
        │
        ▼
Store
```

Content sources may eventually represent different acquisition mechanisms, including network sources and locally available media.

The content model should not assume that Internet download is the only acquisition method.

## Storage Architecture

Large local resources require flexible storage.

DAIA should support resources stored on different classes of storage, including:

* system/native storage;
* secondary disks;
* removable USB storage.

Storage discovery and storage location are separate concerns.

A physical device may be discovered before it has a final mount point or filesystem location.

For this reason, DAIA's storage model should not treat a filesystem path as the fundamental identity of a storage target.

Conceptually:

```text
Discover storage
       │
       ▼
StorageTarget
       │
       ▼
User/system selection
       │
       ▼
Prepare / mount
       │
       ▼
Store resources
```

This is particularly important for appliances containing large offline datasets.

## Localization

Localization should not require DAIA to produce a different large installation image for every language or geographic region.

Where possible, choices such as:

* interface language;
* keyboard configuration;
* locale;
* regional maps;
* regional knowledge resources;
* language-specific AI resources

should be selectable during installation or post-install provisioning.

This allows the same appliance capability model to serve users in different regions while keeping installation images small.

## Offline Operation

A central DAIA goal is that an appliance can operate without continuous Internet connectivity once its required resources have been provisioned.

Network connectivity may be used to acquire resources during setup.

Resources may also be supplied through other supported acquisition mechanisms.

After provisioning, the appliance should be capable of providing its intended local services using locally available software and content.

Examples include:

```text
Local AI
Offline Wikipedia
Offline maps
Local file services
Local development tools
Local educational resources
```

Offline capability therefore influences both content architecture and storage architecture.

## Rust and Shell Boundary

DAIA uses the appropriate implementation mechanism for each layer.

Rust is the authoritative control plane for structured DAIA policy and domain behavior.

Shell may remain appropriate for carefully scoped Linux and Debian system operations.

The detailed boundary is documented separately in:

```text
docs/architecture/shell-and-rust.md
```

The essential rule is:

> Rust and shell may coexist, but DAIA should not maintain duplicated authority for the same responsibility.

## Architectural Invariants

The following principles should guide future DAIA development.

### 1. Appliance profiles describe capabilities

Profiles should express what an appliance needs rather than directly implementing those requirements.

### 2. Capability policy has one authority

Capabilities, providers, resolution, planning, and other architectural decisions should not be independently redefined by multiple implementations.

### 3. Installation images remain small

Large resources should not be placed into the ISO unless there is a specific reason to do so.

### 4. Content and software are different concerns

Installing software that can use Wikipedia, maps, or an AI model is different from installing the datasets or model weights themselves.

### 5. Large content is normally provisioned after installation

The installed system should acquire and configure content according to user requirements and available storage.

### 6. Storage is selectable

DAIA should support native storage, secondary disks, and removable storage rather than assuming all appliance data belongs on the operating-system filesystem.

### 7. Device identity is not a mount path

Storage should be discoverable and representable before final filesystem configuration.

### 8. Localization remains flexible

Language and regional choices should be configurable without requiring unnecessary proliferation of appliance ISO images.

### 9. Provisioned appliances should support offline operation

Continuous Internet connectivity should not be required for the intended operation of an appliance whose necessary resources have been provisioned locally.

### 10. Rust and shell have explicit ownership boundaries

Rust owns structured DAIA policy. Shell may implement scoped system operations. Neither should independently redefine the other's authoritative decisions.

### 11. Existing implementations are retired deliberately

Existing behavior should not be removed merely because a newer implementation exists.

Its responsibility should first be understood, replacement behavior verified, and appropriate tests established.

## Direction

DAIA is evolving toward a system in which a command conceptually resembling:

```text
./build.sh --build <appliance>
```

selects an appliance profile and produces an installation image containing the capabilities required for that appliance.

Examples may eventually include:

```text
./build.sh --build wanderer
./build.sh --build storage
./build.sh --build mail-server
./build.sh --build docker
./build.sh --build kubernetes
```

Those images need not contain all eventual appliance data.

Instead:

```text
Build capability
        ↓
Install appliance
        ↓
Select local configuration
        ↓
Select storage
        ↓
Acquire required resources
        ↓
Configure services
        ↓
Operate locally/offline
```

This separation between **capability**, **content**, and **storage** is fundamental to DAIA's architecture.
