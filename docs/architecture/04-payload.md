# DAIA Payload Architecture

## 1. Purpose

The DAIA payload is the complete set of DAIA files and offline resources
delivered with the installation media.

It contains the material required to deploy and initialize a DAIA system,
including runtime code, configuration, manifests, packages, images, models,
services, and build metadata.

The payload is assembled before ISO injection and is later copied into the
installed system by the Debian installer hook.

The payload should be treated as a generated filesystem image, not as an
informal collection of unrelated files.

---

## 2. Architectural Role

The payload connects the build system to the installer.

Conceptually:

```text
Canonical Sources
       |
       v
Payload Assembly
       |
       v
Generated Payload
       |
       v
ISO Injection
       |
       v
DAIA Installation Media
       |
       v
Late-Install Deployment
       |
       v
Installed Runtime
```

The payload defines what DAIA ships.

The installer defines how that payload is deployed.

The bootstrap and runtime layers define how the deployed content is used.

---

## 3. Responsibilities

The payload architecture is responsible for defining:

* which files and resources ship with DAIA;
* where those files appear in the generated payload;
* where each payload item originates;
* how generated resources are assembled;
* how payload completeness is verified;
* how payload metadata is recorded;
* how payload contents are delivered to the ISO;
* how duplicate source ownership is eliminated.

The payload architecture answers:

> What does the DAIA distribution contain, and how is it assembled?

---

## 4. Non-Responsibilities

The payload does not:

* decide user intent;
* resolve dependencies;
* execute installation logic;
* configure the live system;
* manage long-term runtime state;
* replace the Planner;
* replace the Builder;
* replace the Debian installer;
* verify the final running system.

The payload is an input to installation, not the installation process itself.

---

## 5. Current Payload Sources

DAIA currently uses two payload sources:

```text
installer/files/
work/payload/daia/
```

The first is an established installer-runtime tree.

The second is a generated payload workspace.

During ISO injection, these trees are merged.

The current merge order is:

1. copy `installer/files/`;
2. overlay `work/payload/daia/`.

Conceptually:

```text
installer/files/
       |
       | copied first
       v
merged ISO payload
       ^
       | copied second
       |
work/payload/daia/
```

This allows generated files to replace legacy runtime files when their paths
overlap.

The design is transitional.

---

## 6. Target Payload Model

The target architecture has one generated payload and one canonical assembly
path.

```text
Canonical Source Components
          |
          v
build/build-payload.sh
          |
          v
work/payload/daia/
          |
          v
build/inject.sh
          |
          v
DAIA ISO
```

In the target model:

* all shipped files enter through the payload builder;
* `installer/files/` is no longer copied independently;
* every installed file has one canonical source;
* generated files are clearly distinguished from maintained source files;
* injection consumes only validated payload output.

---

## 7. Payload Workspace

The generated payload workspace is:

```text
work/payload/daia/
```

This directory is build output.

It should not be treated as a canonical source tree.

It may be deleted and recreated during each build.

No file should require manual editing inside this workspace.

Any change intended to persist must be made in a canonical source location or
in the payload assembly logic.

---

## 8. Payload Root Layout

The payload should mirror the filesystem layout expected on the installation
media.

A representative structure is:

```text
work/payload/daia/
├── etc/
│   └── systemd/
│       └── system/
│           └── daia-firstboot.service
│
├── opt/
│   └── daia/
│       ├── install.sh
│       ├── bootstrap.sh
│       ├── VERSION
│       ├── builder/
│       ├── config/
│       ├── core/
│       ├── lib/
│       ├── manifests/
│       ├── modules/
│       ├── planner/
│       ├── packages/
│       ├── images/
│       ├── models/
│       └── metadata/
│
└── manifests/
```

The exact layout should remain deliberate and documented.

Temporary build files, editor backups, tests, and development-only files should
not appear in production payload output unless explicitly required.

---

## 9. Installed Filesystem Mapping

The payload is exposed on the ISO under:

```text
/cdrom/daia/
```

The Debian installer hook copies:

```text
/cdrom/daia/opt/daia
```

to:

```text
/target/opt/daia
```

The first-boot service is copied from:

```text
/cdrom/daia/etc/systemd/system/daia-firstboot.service
```

to:

```text
/target/etc/systemd/system/daia-firstboot.service
```

After the installed system boots, these paths become:

```text
/opt/daia
/etc/systemd/system/daia-firstboot.service
```

The generated payload layout must therefore match the final installed
filesystem layout where practical.

---

## 10. Payload Categories

The payload may contain several classes of content.

### 10.1 Runtime Code

Runtime code includes:

* installer entry points;
* bootstrap scripts;
* shared libraries;
* Planner components;
* Builder components required on the installed system;
* module framework;
* runtime-engine components;
* verification components.

Only code required in the installed appliance should ship.

Development-only tests should remain outside the production payload unless the
product explicitly supports on-device diagnostics.

---

### 10.2 Configuration

Configuration payload content may include:

* default configuration;
* example configuration;
* profile definitions;
* registry data;
* service defaults;
* hardware policy;
* feature settings.

Configuration should be separated into:

* immutable defaults;
* installation-generated configuration;
* mutable runtime configuration;
* desired-state records.

The payload must not contain machine-specific secrets.

---

### 10.3 Manifests

Manifests describe the payload and its resources.

They may include:

* package inventories;
* image inventories;
* model inventories;
* file inventories;
* checksums;
* versions;
* source references;
* build identifiers;
* license information.

Manifests should be machine-readable and versioned.

---

### 10.4 Packages

Offline package content may include:

* Debian packages;
* package indexes;
* dependency metadata;
* repository metadata;
* package checksums.

The payload must make clear whether packages are:

* required;
* optional;
* profile-specific;
* architecture-specific;
* hardware-specific.

---

### 10.5 Container Images

Container image content may include:

* exported image archives;
* image manifests;
* tags;
* digests;
* runtime compatibility information;
* source registry metadata.

Images should be referenced by immutable digest where possible.

The payload should not rely solely on mutable tags.

---

### 10.6 AI Models

Model payload content may include:

* model files;
* tokenizer files;
* configuration;
* quantization metadata;
* licenses;
* checksums;
* provider information;
* hardware requirements.

Models may be large and profile-dependent.

The payload builder should include only the models required by the selected
distribution profile.

---

### 10.7 Branding and User Interface Assets

Branding may include:

* logos;
* themes;
* installer artwork;
* desktop backgrounds;
* Wizard assets;
* service icons.

Branding assets should have explicit ownership and licensing metadata.

---

### 10.8 Metadata

Payload metadata should include enough information to identify exactly what was
built.

Recommended fields include:

* DAIA version;
* payload schema version;
* build identifier;
* source revision;
* build timestamp;
* target architecture;
* target profile;
* plan identifier;
* package count;
* image count;
* model count;
* payload checksum.

---

## 11. Payload Assembly Entry Point

The primary payload assembly entry point is:

```text
build/build-payload.sh
```

It is responsible for coordinating payload generation.

Typical responsibilities include:

* initializing the payload workspace;
* validating source inputs;
* creating the directory structure;
* copying runtime files;
* collecting packages;
* collecting images;
* collecting models;
* generating manifests;
* generating metadata;
* applying permissions;
* validating the assembled output;
* publishing the completed payload.

The script should delegate specialized work to focused helpers where
appropriate.

---

## 12. Assembly Stages

A recommended payload lifecycle is:

```text
Initialize
    |
    v
Create Workspace
    |
    v
Copy Runtime
    |
    v
Collect Packages
    |
    v
Collect Images
    |
    v
Collect Models
    |
    v
Generate Manifests
    |
    v
Apply Permissions
    |
    v
Validate Payload
    |
    v
Publish
```

Dependent stages should not run after a critical failure.

Cleanup should remove incomplete temporary output without deleting useful logs
or source material.

---

## 13. Canonical Source Ownership

Every payload file should have one canonical source.

Examples:

```text
src/runtime/install.sh
    -> work/payload/daia/opt/daia/install.sh

src/systemd/daia-firstboot.service
    -> work/payload/daia/etc/systemd/system/daia-firstboot.service

src/config/daia.conf
    -> work/payload/daia/opt/daia/config/daia.conf
```

The exact source tree may differ, but the ownership rule should not.

A file should not be maintained independently in both:

```text
installer/files/
```

and another source directory.

Generated output must not become a second source of truth.

---

## 14. Source and Output Separation

The repository should distinguish clearly between:

* maintained source;
* generated intermediate output;
* published artifacts.

Conceptually:

```text
source/
    maintained by developers

work/
    temporary generated output

dist/
    completed published artifacts
```

The actual directory names may vary.

The principle is that build output should always be reproducible from source.

---

## 15. Payload Profiles

DAIA may produce different payload variants.

Examples include:

* minimal;
* desktop;
* workstation;
* GPU-enabled;
* CPU-only;
* development;
* production;
* offline-complete;
* lightweight.

A profile may control:

* included packages;
* container images;
* models;
* modules;
* services;
* desktop components;
* hardware support;
* documentation;
* diagnostics.

Profiles should be declarative and validated before payload assembly.

---

## 16. Hardware-Specific Payloads

Some payload resources may depend on target hardware.

Examples include:

* GPU drivers;
* accelerator runtimes;
* architecture-specific packages;
* optimized container images;
* quantized model variants.

Hardware-specific payload assembly should be based on explicit target metadata,
not on accidental properties of the build host.

The build host and target machine must not be assumed to be identical.

---

## 17. Payload Manifests

The payload should include a top-level manifest.

A conceptual manifest may resemble:

```yaml
schema_version: 1
payload:
  version: 0.1.0
  build_id: daia-20260723-001
  architecture: amd64
  profile: workstation
  source_revision: abcdef1234

contents:
  runtime:
    files: manifests/runtime-files.json
  packages:
    inventory: manifests/packages.json
  images:
    inventory: manifests/images.json
  models:
    inventory: manifests/models.json

integrity:
  checksums: manifests/SHA256SUMS
```

The final schema should be formally defined and versioned.

---

## 18. File Inventory

A file inventory should record every payload file.

Recommended fields include:

* relative path;
* file type;
* size;
* permissions;
* owner;
* group;
* checksum;
* source component;
* required or optional status.

The inventory can be used to detect:

* missing files;
* unexpected files;
* permission errors;
* accidental test artifacts;
* duplicate ownership;
* post-build modification.

---

## 19. Permissions

Permissions should be applied during payload assembly.

Examples:

* executable scripts must be executable;
* configuration should not be globally writable;
* runtime code should not be writable by unprivileged users;
* state directories should have appropriate runtime ownership;
* secret locations should not contain default readable secrets.

The installer may verify critical permissions, but it should not be responsible
for reconstructing the entire permission model.

---

## 20. Mutable and Immutable Content

The payload should distinguish between immutable shipped content and mutable
runtime state.

Immutable content may include:

```text
/opt/daia/lib/
/opt/daia/modules/
/opt/daia/planner/
/opt/daia/manifests/
```

Mutable content may include:

```text
/opt/daia/state/
/opt/daia/logs/
/var/lib/daia/
```

Mutable directories may be created by installation or first boot rather than
being populated with build-host state.

The payload should never ship stale runtime state from a previous execution.

---

## 21. Empty Runtime Directories

Some runtime directories may need to exist even when initially empty.

Examples include:

```text
/opt/daia/state/desired/
/opt/daia/state/current/
/opt/daia/state/cache/
/opt/daia/logs/
```

Their creation should be intentional.

Possible approaches include:

* creating them during payload assembly;
* creating them in the late-install hook;
* creating them during first-boot initialization.

Ownership of this responsibility should be documented and consistent.

Placeholder files such as `.gitkeep` should not ship unless they have a
runtime purpose.

---

## 22. Development Artifacts

The production payload should exclude development-only artifacts.

Examples include:

```text
*.orig
*.rej
*.bak
*~
builder-test.sh
workspace-builder-test.sh
coverage output
editor metadata
temporary logs
test fixtures
```

Tests may be distributed separately as a diagnostics package if required.

They should not appear accidentally in the appliance payload.

---

## 23. Overlay Semantics

The current dual-source merge gives the generated payload precedence over the
legacy runtime.

This means a path collision is resolved in favor of:

```text
work/payload/daia/
```

Overlay behavior must be explicit.

Unreported collisions are risky because they can hide duplicate ownership.

During migration, the build should produce a collision report containing:

* duplicated path;
* legacy source;
* generated source;
* selected result;
* expected or unexpected status.

Unexpected collisions should fail payload validation.

---

## 24. Payload Validation

The completed payload should be validated before ISO injection.

Validation should confirm:

* required root directories exist;
* primary entry points exist;
* scripts have valid syntax;
* executable permissions are correct;
* systemd units exist;
* required configuration exists;
* manifests are present;
* manifest entries match actual files;
* checksums are valid;
* no prohibited files are present;
* no unexpected duplicate ownership exists;
* required package, image, and model resources are available.

A payload that fails validation must not be injected into the ISO.

---

## 25. Required Payload Files

At minimum, the current installer flow requires:

```text
opt/daia/install.sh
opt/daia/bootstrap.sh
etc/systemd/system/daia-firstboot.service
```

The bootstrap runtime also requires its referenced:

* configuration;
* libraries;
* modules;
* manifests;
* Planner components.

The exact required-file list should be generated from the runtime contract
rather than maintained only as informal knowledge.

---

## 26. Payload Integrity

Payload integrity should be verified using cryptographic checksums.

A top-level checksum inventory may be stored as:

```text
manifests/SHA256SUMS
```

Verification should occur:

1. after payload assembly;
2. after ISO injection;
3. optionally during late-install deployment;
4. optionally before first-boot execution.

Checksums detect corruption but do not prove authenticity.

Authenticity requires signatures anchored in a trusted key.

---

## 27. Payload Authenticity

Future payload releases should support signed metadata.

A possible model is:

```text
payload manifest
      |
      v
cryptographic signature
      |
      v
trusted DAIA release key
```

The installer should verify the signed manifest before deploying privileged
runtime code.

The trust and key-rotation model requires a separate security design.

---

## 28. Reproducibility

A payload build should be reproducible from:

* source revision;
* profile;
* execution plan;
* dependency versions;
* target architecture;
* build configuration.

Sources of nondeterminism should be controlled or recorded.

Examples include:

* timestamps;
* random identifiers;
* archive ordering;
* filesystem traversal order;
* package mirror changes;
* mutable container tags;
* unpinned model revisions.

Equivalent builds should produce either identical output or a documented
explanation of expected differences.

---

## 29. Atomic Publication

Payload output should be assembled in a temporary workspace.

Recommended flow:

```text
work/payload.tmp/
       |
       v
assemble and validate
       |
       v
atomic rename
       |
       v
work/payload/daia/
```

This prevents an incomplete build from being mistaken for a valid payload.

The previous valid payload should not be destroyed until the replacement passes
validation.

---

## 30. Cleanup

Payload cleanup may remove:

* temporary downloads;
* extraction directories;
* partial archives;
* intermediate manifests;
* temporary package indexes;
* incomplete output trees.

Cleanup must not remove:

* canonical source files;
* the previous valid payload without replacement;
* diagnostic logs needed to understand failure;
* shared caches unless explicitly requested.

---

## 31. Error Handling

Payload assembly should fail explicitly when:

* a required source is missing;
* a resource checksum fails;
* a package cannot be resolved;
* an image export fails;
* a model is incomplete;
* a manifest cannot be generated;
* a required permission cannot be applied;
* payload validation fails;
* unexpected source collisions exist.

Dependent stages must not continue after critical failure.

The original error status should be preserved where practical.

---

## 32. Logging

Payload logs should identify:

* build identifier;
* selected profile;
* target architecture;
* source revision;
* assembly stage;
* resource category;
* copied or generated path;
* source location;
* checksum result;
* validation result;
* final payload location.

The logs should make it possible to answer:

* Which files were included?
* Where did each file come from?
* Which packages were collected?
* Which images and models were included?
* Did any paths collide?
* Did validation pass?
* Which payload was injected into the ISO?

---

## 33. Security Considerations

The payload contains privileged executable code.

The assembly process must therefore guard against:

* unsafe source paths;
* path traversal in archives;
* symbolic-link attacks;
* world-writable executable files;
* unverified downloads;
* malicious package contents;
* mutable container references;
* unexpected setuid or setgid files;
* embedded credentials;
* accidental private keys;
* untrusted configuration evaluation.

Archive extraction should reject paths that escape the intended workspace.

Generated shell configuration should never interpolate untrusted values as
code.

---

## 34. Licenses and Attribution

Packages, container images, models, artwork, and third-party runtime components
may have distinct licenses.

The payload should include machine-readable and human-readable licensing
information where required.

Model inventories should record:

* model name;
* source;
* license;
* usage restrictions;
* redistribution status.

The payload builder should reject resources that cannot legally be distributed
in the selected artifact.

---

## 35. Size Management

DAIA payloads may become large because of:

* Debian packages;
* container images;
* AI models;
* desktop environments;
* GPU runtimes.

Payload reports should record size by category.

Example:

```text
Runtime code:       25 MiB
Packages:          3.2 GiB
Container images:  8.4 GiB
Models:           28.0 GiB
Branding:          12 MiB
Total:            39.7 GiB
```

Profiles should allow large optional resources to be excluded.

ISO format and target-media size limits must be considered during planning.

---

## 36. Compression

Large payload resources may be compressed for distribution.

Compression design should consider:

* build time;
* ISO size;
* installation time;
* target disk space;
* memory usage;
* random-access requirements;
* checksum semantics.

Compression should not obscure resource identity or prevent integrity
verification.

The manifest should state whether checksums apply to compressed or extracted
content.

---

## 37. Payload and Planner Relationship

The Planner determines which resources are required.

The payload builder assembles those resources.

Conceptually:

```text
Desired State
      |
      v
Planner
      |
      v
Execution Plan
      |
      v
Payload Builder
      |
      v
Distribution Payload
```

The payload builder should not independently select substitute components
without Planner input.

If a requested resource is unavailable, assembly should fail or return the
problem to planning.

---

## 38. Payload and Builder Relationship

The Builder coordinates payload-producing operations.

The payload architecture defines:

* layout;
* content classes;
* ownership;
* validation;
* publication.

The Builder defines:

* lifecycle;
* callbacks;
* state;
* cleanup;
* failure propagation.

The payload builder may be invoked by the Builder as one specialized build
operation.

---

## 39. Payload and Installer Relationship

The payload is produced before installation.

The installer consumes it.

The installer should not need to understand how every payload file was built.

It should rely on:

* a valid payload layout;
* a manifest;
* required entry points;
* verified integrity;
* documented deployment mappings.

This keeps installer logic small and predictable.

---

## 40. Payload and Runtime Relationship

The installed runtime originates from the payload.

Runtime components may read shipped:

* manifests;
* version metadata;
* registry data;
* default configuration;
* package inventories;
* model inventories.

Runtime-generated state must remain separate from shipped payload metadata.

The runtime should be able to identify the exact payload version from which the
system was installed.

---

## 41. Payload Versioning

The payload should have an independent schema version and a product version.

Example:

```yaml
product_version: 0.4.0
payload_schema_version: 2
```

The product version identifies the DAIA release.

The schema version identifies the structure and interpretation of payload
metadata.

Runtime components should reject unsupported future schema versions rather than
silently misreading them.

---

## 42. Upgrade Payloads

Future DAIA upgrades may reuse the payload architecture.

Possible upgrade payloads include:

* complete replacement payloads;
* delta payloads;
* module-specific bundles;
* model packs;
* security updates.

Upgrade design must preserve:

* current-state awareness;
* compatibility checks;
* rollback safety;
* integrity verification;
* atomic publication;
* migration sequencing.

The current installation payload should not be assumed automatically suitable
for live upgrades without additional contracts.

---

## 43. Testing Strategy

Payload testing should include several levels.

### 43.1 Script Validation

Run syntax and static checks on payload assembly scripts.

### 43.2 Layout Tests

Verify expected files appear at expected relative paths.

### 43.3 Manifest Tests

Verify:

* every manifest entry exists;
* every tracked file has the expected checksum;
* required metadata fields exist;
* schema versions are supported.

### 43.4 Exclusion Tests

Verify prohibited artifacts do not ship.

Examples include:

* editor backups;
* test outputs;
* source-control metadata;
* secrets;
* temporary files.

### 43.5 Collision Tests

Verify all overlays and duplicate paths are intentional.

### 43.6 Reproducibility Tests

Build the same payload twice and compare outputs.

### 43.7 ISO Integration Tests

Verify the payload survives injection without missing or changed files.

### 43.8 Installation Tests

Install from the generated ISO and verify that the deployed runtime matches the
payload manifest.

---

## 44. Current Limitations

The current payload architecture has several known limitations.

### 44.1 Dual Source Trees

The runtime is still assembled from both:

```text
installer/files/
work/payload/daia/
```

### 44.2 Incomplete Canonical Ownership

Not every shipped path has one documented canonical source.

### 44.3 Informal Required-File Contract

The required runtime file set needs a generated, versioned contract.

### 44.4 Integrity Model

Checksums and signatures require further formalization.

### 44.5 Manifest Schema

Payload metadata and inventory schemas require final definitions.

### 44.6 Development Artifact Exclusion

Production payload validation should enforce explicit exclusion rules.

### 44.7 Profile Model

Payload profiles and hardware-specific variants require a stable declarative
schema.

---

## 45. Migration Strategy

The dual-source architecture should be removed incrementally.

Recommended migration order:

1. migrate `install.sh` into canonical payload assembly;
2. migrate `bootstrap.sh`;
3. migrate `daia-firstboot.service`;
4. migrate shared libraries;
5. migrate modules;
6. migrate Planner components;
7. migrate configuration;
8. migrate manifests;
9. migrate remaining runtime files;
10. validate source ownership;
11. remove the `installer/files/` copy step from `build/inject.sh`;
12. remove obsolete duplicate files;
13. remove `installer/files/` when empty and no longer referenced.

Each step should be independently testable.

---

## 46. Migration Validation

After each migrated component:

1. build the payload;
2. inspect the resulting path;
3. verify permissions;
4. generate the file inventory;
5. check for duplicate ownership;
6. inject the ISO;
7. inspect the ISO;
8. install in a virtual machine;
9. verify first-boot behavior;
10. compare installed files with the payload manifest.

Migration should stop if any established installation behavior regresses.

---

## 47. Payload Contract

The payload builder accepts:

* canonical source files;
* selected profile;
* target architecture;
* execution plan;
* package sources;
* image sources;
* model sources;
* build configuration.

It produces:

* a complete filesystem-layout payload;
* version metadata;
* file inventories;
* resource manifests;
* checksums;
* validation results;
* explicit final status.

A successful payload build means:

1. all required content was assembled;
2. every shipped path has known ownership;
3. required permissions were applied;
4. manifests match the output;
5. integrity checks passed;
6. prohibited files are absent;
7. the payload was atomically published;
8. the payload is ready for ISO injection.

---

## 48. Design Principles

The Payload architecture follows these principles:

1. one canonical source per file;
2. generated workspaces are not source trees;
3. payload layout mirrors deployment layout;
4. content selection is plan-driven;
5. required resources are available offline;
6. manifests describe all shipped content;
7. validation occurs before ISO injection;
8. incomplete payloads are never published;
9. runtime state is not shipped as build content;
10. migration removes duplication incrementally.

---

## 49. Summary

The DAIA payload is the generated distribution filesystem delivered through the
installation media.

Its intended flow is:

```text
canonical sources
    -> payload assembly
    -> validation
    -> generated payload
    -> ISO injection
    -> installer deployment
    -> installed runtime
```

The current implementation already has a working payload pipeline, but it still
merges an established runtime tree with a generated payload workspace.

The immediate goal is therefore not to invent a new payload system.

It is to complete the existing migration toward one canonical, validated,
generated payload that contains every file required by the DAIA appliance.
