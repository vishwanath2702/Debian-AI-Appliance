# Build Debian AI Appliance (DAIA) From Scratch

Design and implement **Debian AI Appliance (DAIA)**: a framework for producing small, purpose-specific Debian appliance installation images from declarative appliance definitions.

DAIA is not merely an ISO customization script.

It is an appliance platform consisting of:

1. a declarative appliance model;
2. a capability/provider registry;
3. dependency and capability resolution;
4. deterministic execution planning;
5. Debian system construction;
6. bootable ISO generation;
7. an installed DAIA runtime;
8. post-install provisioning;
9. content acquisition and verification;
10. flexible storage management;
11. localization;
12. offline-first operation.

The architecture must remain modular, deterministic, testable, and understandable.

---

# Core Product Principle

An appliance definition describes **capabilities**, not a giant collection of files to embed into an ISO.

For example:

```yaml
name: wanderer
description: Offline knowledge and navigation appliance

capabilities:
  - local-ai
  - offline-knowledge
  - offline-maps
```

Another profile might be:

```yaml
name: storage
description: Local private storage appliance

capabilities:
  - storage-server
```

Other future appliances may include:

* developer
* mail-server
* docker
* kubernetes
* education
* research
* media
* communications

The architecture must allow new appliance types to be added primarily through declarative definitions and providers rather than modifications to the core engine.

---

# Architectural Rule: One Authority Per Responsibility

DAIA may use Rust, shell, YAML, and established Debian/Linux utilities.

However:

> Every architectural responsibility must have one authoritative owner.

Do not create parallel implementations of appliance policy.

For example, do not independently define what `wanderer` means in both Rust and shell.

Do not maintain two independent planners.

Do not maintain two independent capability registries.

---

# Rust Control Plane

Implement DAIA's structured control plane in Rust.

Rust owns:

* domain models;
* identifiers;
* appliance profiles;
* capabilities;
* providers;
* registries;
* package manifests;
* dependency resolution;
* capability resolution;
* planning;
* validation;
* execution orchestration;
* build state;
* error propagation;
* content models;
* storage models;
* provisioning state;
* Wizard/application logic.

Use strong types instead of unstructured strings wherever identity or domain boundaries matter.

Prefer small crates with explicit responsibilities.

A likely workspace architecture is:

```text
engine/
├── crates/
│   ├── model
│   ├── registry
│   ├── resolver
│   ├── planner
│   ├── executor
│   ├── inspector
│   ├── engine
│   └── cli
│
└── registry/
    ├── appliance-profiles/
    ├── providers/
    ├── package-manifests/
    └── assets/
```

Do not create unnecessary abstractions before their responsibilities are understood.

---

# Shell Boundary

Shell is permitted and may remain permanently where it is the appropriate system-integration mechanism.

Good uses include:

* small bootstrap helpers;
* recovery operations;
* filesystem/system administration sequences;
* wrappers around Linux/Debian utilities;
* tightly scoped operations where shell is substantially clearer than Rust.

Shell must not independently own:

* appliance definitions;
* capability policy;
* provider resolution;
* dependency resolution;
* planning;
* Wizard state;
* architectural configuration.

A shell helper should receive a well-defined operation from the control plane.

Conceptually:

```text
Rust policy
     │
     ▼
Execution request
     │
     ├──── Rust implementation
     │
     └──── Shell helper
                  │
                  ▼
             Debian/Linux
```

Do not rewrite a useful shell operation in Rust merely to eliminate shell.

Likewise, do not implement architectural policy in shell merely because it is faster to prototype.

---

# Use Existing Debian Tools

DAIA should orchestrate mature Debian/Linux tools instead of unnecessarily reimplementing them.

Examples include:

* mmdebstrap;
* apt;
* dpkg;
* systemctl;
* mount;
* filesystem utilities;
* mksquashfs;
* GRUB tooling;
* xorriso.

Wrap these tools behind testable execution boundaries.

Capture failures explicitly and propagate meaningful errors.

---

# Declarative Registry

DAIA configuration should be repository-driven.

Example provider:

```yaml
id: desktop
capability: desktop

steps:
  - install_package_manifest: desktop
  - enable_service: display-manager
```

Example package manifest:

```yaml
name: desktop

packages:
  - live-boot
  - live-config
  - systemd-sysv
  - linux-image-amd64
  - initramfs-tools
  - task-gnome-desktop
  - gdm3
```

Example appliance profile:

```yaml
name: desktop
description: Graphical Debian desktop appliance

capabilities:
  - desktop
```

The core engine must not need to understand the semantic meaning of every appliance.

It should resolve declarative capabilities through providers.

---

# Planning Pipeline

Implement a deterministic pipeline:

```text
ApplianceProfile
        ↓
Capabilities
        ↓
Registry
        ↓
Resolver
        ↓
Planner
        ↓
Plan
        ↓
Executor
```

A `Plan` should explicitly describe what will happen before execution begins.

Planning must be testable without modifying the host system.

Unknown capabilities must fail explicitly.

Preserve declared ordering where ordering is meaningful.

---

# Debian ISO Pipeline

Implement ISO generation as explicit stages rather than one large procedure.

Conceptually:

```text
Inspect source
      ↓
Prepare build context
      ↓
Bootstrap rootfs
      ↓
Execute appliance plan
      ↓
Generate initramfs
      ↓
Locate kernel/initramfs
      ↓
Construct live filesystem
      ↓
mksquashfs
      ↓
Generate bootloader configuration
      ↓
grub-mkrescue / xorriso
      ↓
Bootable ISO
```

Represent build state explicitly.

Each stage should:

* validate its inputs;
* produce clearly defined outputs;
* return structured errors;
* be independently testable where practical.

A failure must stop dependent stages.

---

# Minimal ISO Principle

Keep appliance ISOs as small as reasonably possible.

The ISO contains:

* Debian base system;
* DAIA runtime;
* software necessary to provide selected capabilities;
* provisioning capability;
* required boot/install infrastructure.

The ISO should normally **not** contain large datasets.

Do not routinely embed:

* AI model weights;
* Wikipedia dumps;
* map datasets;
* large documentation archives;
* user content;
* large container images.

Those belong to post-install provisioning unless an appliance explicitly requires otherwise.

---

# Build Interface

The eventual user experience should support a command conceptually similar to:

```bash
./build.sh --build wanderer
```

or:

```bash
./build.sh --build storage
./build.sh --build mail-server
./build.sh --build docker
./build.sh --build kubernetes
```

The build name maps to an appliance profile.

Avoid hard-coding appliance-specific logic into `build.sh`.

The script, if retained, should be an entry point into the authoritative DAIA engine rather than another implementation of the build policy.

---

# Installation Is Not Final Provisioning

Separate operating-system installation from large-resource provisioning.

The lifecycle is:

```text
Build
  ↓
Minimal appliance ISO
  ↓
Install
  ↓
DAIA runtime
  ↓
Post-install Wizard
  ↓
Configure locale
  ↓
Discover/select storage
  ↓
Select resources
  ↓
Acquire resources
  ↓
Verify resources
  ↓
Configure local services
  ↓
Fully provisioned appliance
```

The Wizard should allow choices to be changed later where practical.

---

# Content Model

Large resources are first-class DAIA concepts.

Model at least:

```text
ContentRepositoryId
ContentRepository

ContentSourceId
ContentSource
```

A `ContentRepository` represents a logical resource collection.

Examples:

```text
wikipedia
maps
ai-models
documentation
educational-content
```

A `ContentSource` describes where content can be acquired.

Do not initially assume that a source must be HTTP.

Future acquisition mechanisms may include:

* Internet;
* LAN mirrors;
* local servers;
* removable media;
* USB;
* pre-populated secondary disks;
* other offline transfer mechanisms.

Separate **what the content is** from **where it comes from**.

---

# Storage Model

Content location must be independent of content acquisition.

Model storage as first-class domain objects.

Start with stable identity rather than filesystem paths.

For example:

```text
StorageTargetId
StorageTarget
StorageKind
```

DAIA must eventually support:

* native/system storage;
* secondary disks;
* removable USB storage.

Do not equate a storage device with a mount path.

The lifecycle may be:

```text
Discover device
      ↓
Identify StorageTarget
      ↓
Select target
      ↓
Prepare storage
      ↓
Mount/configure
      ↓
Assign repositories
```

Hardware discovery must not require the final mount location to already exist.

---

# Offline-First Operation

A fully provisioned DAIA appliance should be capable of performing its intended function without continuous Internet connectivity.

Examples:

```text
Local AI inference
Offline Wikipedia
Offline maps
Local file storage
Local development environment
Local educational resources
```

Network connectivity may be used during provisioning.

Network connectivity must not automatically become a runtime dependency.

---

# Localization

Localization should be primarily configurable during installation or post-install provisioning rather than requiring separate large regional ISOs.

Support eventual selection of:

* language;
* locale;
* keyboard;
* timezone/region;
* regional maps;
* regional knowledge;
* language-specific AI resources.

The same appliance architecture should work across geographic regions.

---

# Testing Requirements

Use tests as architectural protection.

At minimum, test:

* model invariants;
* repository loading;
* malformed YAML;
* duplicate definitions;
* unknown capabilities;
* resolver behavior;
* plan generation;
* profile planning;
* execution failure propagation;
* build-stage ordering;
* external command failures;
* ISO state transitions.

Whenever fixing a meaningful bug, add a regression test when practical.

Never rely solely on a successful compilation.

Use:

```bash
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets
git diff --check
```

where appropriate.

---

# Development Method

Build DAIA incrementally.

For each architectural concept:

1. identify the responsibility;
2. define the smallest useful domain model;
3. write a test;
4. implement it;
5. run focused tests;
6. run workspace validation when appropriate;
7. inspect the diff;
8. commit the concept independently;
9. push;
10. continue to the next boundary.

Prefer small commits such as:

```text
Add appliance profile model
Add appliance profile repository
Plan appliance profile capabilities
Expose appliance profile planning through engine
Add appliance profile planning command
Add content repository identifier
Add content repository model
Add content source identifier
Add content source model
Add storage target identifier
```

Do not mix unrelated cleanup with architectural commits.

---

# Documentation Requirements

Architecture is part of the implementation.

Maintain:

```text
docs/
└── architecture/
    ├── overview.md
    ├── shell-and-rust.md
    ├── build-system.md
    ├── appliance-model.md
    └── content-and-storage.md
```

Use architectural decision records when a choice has meaningful long-term consequences.

Documentation must explain **why**, not merely repeat the code.

If implementation changes an architectural boundary, update the corresponding documentation.

At any point in the project it should be possible to answer:

* Who owns this responsibility?
* Why does this component exist?
* What consumes it?
* What does it produce?
* Is it build-time, install-time, or runtime?
* Is it policy or execution?
* What is its source of truth?

If those questions cannot be answered clearly, stop and resolve the ambiguity before extending the architecture.

---

# Architectural Invariants

Do not violate these without explicitly revisiting the architecture:

1. There is one authoritative owner for each responsibility.
2. Appliance profiles describe capabilities.
3. Providers implement capabilities.
4. Planning is separate from execution.
5. Plans are inspectable before execution.
6. Build-time and post-install provisioning are distinct.
7. Software capability and bulk content are distinct.
8. Content identity, acquisition source, and storage destination are distinct.
9. Storage device identity is not equivalent to a mount path.
10. Large content normally stays out of the ISO.
11. Provisioned appliances should support offline operation.
12. Localization should not require unnecessary ISO proliferation.
13. Rust owns structured DAIA policy.
14. Shell may perform scoped system operations.
15. Rust and shell must not independently define the same policy.
16. Existing implementations are not removed until their responsibilities are understood and replacements are verified.
17. Prefer established Debian tooling over unnecessary reimplementation.
18. Architecture must remain explainable.

---

# Engineering Priorities

When choosing between approaches, prioritize in this order:

1. correctness;
2. architectural clarity;
3. deterministic behavior;
4. recoverability;
5. testability;
6. maintainability;
7. security;
8. minimal image size;
9. extensibility;
10. convenience.

Do not optimize for fewer lines of code at the expense of architectural clarity.

Do not introduce abstractions merely because they might be useful someday.

Build the smallest correct abstraction required by the next demonstrated responsibility.

---

# Definition of Success

DAIA succeeds when a user can select an appliance such as:

```text
wanderer
storage
developer
mail-server
docker
kubernetes
```

build a small Debian installation image, install it on suitable hardware, configure storage/resources/localization through DAIA, provision the required content, and then operate the appliance locally without depending on continuous Internet connectivity.

Adding a new appliance should primarily mean defining:

```text
Appliance Profile
       +
Capabilities
       +
Providers
       +
Package manifests/assets
```

rather than writing another custom operating-system build system.

The final architecture must be understandable enough that a new contributor can trace:

```text
user intent
    ↓
appliance profile
    ↓
capabilities
    ↓
providers
    ↓
plan
    ↓
execution
    ↓
ISO
    ↓
installation
    ↓
provisioning
    ↓
running appliance
```

without discovering a second hidden source of truth elsewhere in the repository.
