# Architecture Decision: Why Loom Does Not Use Debian Live-Build as Its Core Build Engine

## Status

Accepted

## Context

Loom is the Rust-based construction and lifecycle engine for Debian-based appliances. DAIA/Pragna is an appliance built using Loom, not the name of the build engine. During the architecture phase, we evaluated whether Debian's `live-build` tooling should be used as the primary ISO generation engine.

`live-build` is a mature Debian project that already provides many capabilities required for generating bootable Debian Live images, including:

* filesystem preparation
* package installation workflows
* SquashFS generation
* bootloader configuration
* initramfs handling
* BIOS and UEFI boot support
* ISO generation

From a purely functional perspective, Loom could have been implemented as a layer of configuration and automation around `live-build`.

The decision was made not to use `live-build` as the core engine.

This was not because `live-build` is insufficient. It was because the goals and abstractions of Loom are different.

---

# Decision

Loom will maintain its own appliance build engine and treat ISO creation as one backend capability rather than making Debian Live generation the foundation of the system.

Existing Linux image-building tools may still be integrated as optional backends in the future, but Loom's internal model will remain independent.

---

# Rationale

## 1. Loom supports controlled artifact transformation; live-build creates live images

The primary workflow difference is:

### live-build model

```
Configuration
      |
      v
live-build
      |
      v
Debian Live ISO
```

The user describes what should exist, and live-build produces the ISO.

### Loom source-ISO workflow

```
Existing Debian Source ISO
             |
             v
       Inspection
             |
             v
       Validation
             |
             v
       Transformation
             |
             v
         Appliance ISO
```

In Loom's source-ISO workflow, an existing image is inspected and transformed into a controlled derivative. The source ISO is a tracked, validated input rather than an opaque implementation detail. Other Loom backends may start from different sources.

---

# 2. ISO identity and provenance are first-class concepts

A general live image builder can treat the base distribution as an implementation detail.

Loom cannot.

An appliance build must answer questions such as:

* Which Debian release was used?
* Which architecture is targeted?
* Which boot modes are supported?
* Which repositories are present?
* Which source image produced this appliance?
* What transformations were applied?

This requires concepts such as:

* ISO metadata inspection
* source artifact validation
* reproducible transformation stages
* build provenance

These concepts belong in the Loom engine.

---

# 3. Purpose-specific appliances have requirements beyond traditional live images

A traditional Debian Live image focuses on:

```
Debian
+
Packages
+
Configuration
```

A purpose-specific appliance may require:

```
Debian base
+
Selected capabilities and providers
+
Application services and content
+
Hardware and firmware integration
+
Security and isolation policies
+
Offline operation
+
Data and configuration recovery
+
Lifecycle management
```

For example, DAIA/Pragna can add local inference, model management, and RAG services; other appliances can select storage, networking, media, or recovery capabilities without inheriting an AI stack.

The build process becomes closer to:

* firmware image generation
* cloud image creation
* container image pipelines
* appliance factories

rather than a traditional Live CD workflow.

---

# 4. Loom requires a programmable build graph

The Loom architecture is built around independent components:

```
model
registry
resolver
planner
executor
engine
```

The goal is to describe an appliance as a plan:

```
Requirement
      |
      v
Resolution
      |
      v
Execution
      |
      v
Artifact
```

The ISO backend is only one execution target.

Future targets may include:

* ISO images
* VM images
* disk images
* PXE images
* cloud images

Using live-build as the core abstraction would couple the system to one output format.

---

# 5. Native control improves determinism

A wrapper around live-build would inherit:

* live-build configuration conventions
* hidden internal behavior
* external tool assumptions
* Debian Live specific lifecycle

A native Loom engine provides:

* explicit stages
* controlled inputs and outputs
* deterministic execution
* testable components
* clear error handling

For an appliance factory, transparency is more valuable than hiding complexity behind a general-purpose builder.

---

# Tradeoff Accepted

Choosing a native engine means Loom must implement functionality that already exists elsewhere.

Examples:

* workspace creation
* SquashFS generation orchestration
* GRUB configuration
* ISO assembly

This increases development effort.

However, these components represent the appliance pipeline boundary, not simply duplicated functionality.

The goal is not to replace Debian Live tooling. The goal is to own the appliance compilation workflow.

---

# Future Compatibility

Loom may support multiple image-generation backends:

```
              Loom Engine
                  |
        +---------+---------+
        |                   |
   Native ISO          live-build
    backend             backend
```

A live-build backend could be useful where a standard Debian Live workflow is sufficient.

The core engine should remain independent.

---

# Conclusion

Loom did not avoid `live-build` because it could not solve the problem.

It avoided `live-build` because it solves a different problem.

`live-build` creates Debian Live images.

Loom constructs and manages purpose-specific Debian appliances.

The distinction is the architecture boundary.
