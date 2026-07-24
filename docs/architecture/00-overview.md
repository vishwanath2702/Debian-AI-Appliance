# DAIA Architecture Overview

## 1. Introduction

DAIA is a Debian-based AI appliance designed to provide a complete,
reproducible, locally operated AI environment.

The project combines:

* Debian system installation
* hardware-aware configuration
* dependency planning
* payload assembly
* component installation
* service configuration
* verification
* runtime state management
* a future guided installation wizard

DAIA is intended to behave as an integrated appliance rather than as a loose
collection of installation scripts.

Its architecture separates user intent, planning, build operations,
installation, runtime execution, and verification into distinct subsystems.

---

## 2. Architectural Goals

The DAIA architecture is designed around the following goals:

### 2.1 Reproducibility

A DAIA system should be buildable and installable consistently from a known
configuration and set of source artifacts.

### 2.2 Separation of Responsibilities

Each subsystem should have a narrow and explicit responsibility.

For example:

* the Wizard gathers user intent;
* the Planner resolves what must be installed;
* the Builder assembles artifacts;
* the installer deploys the payload;
* runtime components manage the installed system;
* the Verifier confirms the resulting state.

### 2.3 Offline Operation

DAIA should support installation and operation without depending on continuous
internet access.

Required packages, container images, models, resources, and configuration may
be assembled into the distribution payload before installation.

### 2.4 Hardware Awareness

DAIA should detect relevant hardware capabilities and use that information
when planning and configuring the system.

Hardware information may influence:

* supported AI engines;
* acceleration capabilities;
* model selection;
* resource limits;
* service configuration;
* installation profiles.

### 2.5 Declarative State

The intended system configuration should be represented as desired state
rather than being embedded directly in user-interface or installation logic.

### 2.6 Verifiable Execution

Important lifecycle operations should produce explicit status, phase, logs,
and verification results.

### 2.7 Incremental Evolution

DAIA is under active architectural development.

Transitional paths may remain temporarily while established components are
migrated into their final subsystems. Such transitional behavior should be
documented and removed deliberately rather than hidden.

---

## 3. High-Level Architecture

The intended high-level flow is:

```text
User
  |
  v
Installation Wizard
  |
  v
Configuration and Desired State
  |
  v
Planner
  |
  v
Execution Plan
  |
  v
Builder and Payload Assembly
  |
  v
DAIA ISO
  |
  v
Debian Installation
  |
  v
Late-Install Hook
  |
  v
First-Boot Bootstrap
  |
  v
Runtime Components
  |
  v
Verification and State Recording
```

The Wizard is a future user-facing component.

The Planner, Builder, payload pipeline, installer runtime, and module framework
already exist in varying stages of maturity.

---

## 4. Major Subsystems

## 4.1 Configuration

The configuration subsystem provides the values required by DAIA components.

Configuration currently includes filesystem locations, installation settings,
enabled functionality, and shared runtime constants.

Configuration must remain separate from execution logic.

A component may read and validate configuration, but configuration files should
not directly perform installation or runtime actions.

---

## 4.2 Desired State

Desired state represents what the user or selected profile wants the completed
DAIA system to contain.

Examples may include:

* selected desktop environment;
* container runtime;
* AI engine;
* enabled services;
* installed capabilities;
* selected models;
* resource settings.

Desired state should describe the intended result without encoding the steps
required to produce it.

The Planner is responsible for translating desired state into an executable
plan.

---

## 4.3 Plugin and Capability Registries

Registries describe the components and capabilities known to DAIA.

A registry may provide information such as:

* component identity;
* dependencies;
* conflicts;
* supported hardware;
* provided capabilities;
* installation handlers;
* verification handlers.

Registries provide structured input to the Planner and should not perform the
installation themselves.

---

## 4.4 Planner

The Planner converts desired state and registry information into an ordered,
validated execution plan.

Planner responsibilities include:

* reading the selected profile;
* resolving requested capabilities;
* synchronizing capability selections;
* resolving dependencies;
* detecting conflicts;
* adapting registry data;
* constructing the execution plan.

The Planner answers:

> What must be done, and in what order?

It should not directly build images, install packages, modify services, or
execute component-specific installation logic.

Current planner components are located under:

```text
/opt/daia/planner/
```

---

## 4.5 Builder

The Builder executes the build lifecycle required to assemble DAIA artifacts.

The Builder coordinates specialized callbacks rather than embedding all build
operations in one script.

Its current lifecycle includes:

```text
initialize
  |
  v
build workspace
  |
  v
execute plugin plan
  |
  v
build image
  |
  v
finalize and clean up
```

The Builder records lifecycle status and phase information through the Build
State component.

The Builder subsystem currently includes:

```text
build-context.sh
build-state.sh
builder.sh
logger.sh
plugin-executor.sh
workspace-builder.sh
```

The Builder has behavioural tests covering successful execution, lifecycle
preconditions, callback failures, argument validation, cleanup failure, and
state recording.

The Builder answers:

> How are the planned build operations coordinated and executed?

---

## 4.6 Payload Assembly

The payload contains the DAIA files and resources that will be delivered with
the installation media.

The generated payload workspace is assembled under:

```text
work/payload/daia/
```

Payload assembly may include:

* runtime scripts;
* libraries;
* modules;
* packages;
* container images;
* models;
* branding;
* configuration defaults;
* manifests;
* build metadata.

The primary payload assembly entry point is:

```text
build/build-payload.sh
```

Additional tooling generates payload inventories and reports.

The payload should eventually have one canonical source and one complete
assembly path.

---

## 4.7 Transitional Installer Runtime

DAIA currently has two payload sources:

```text
installer/files/
work/payload/daia/
```

`installer/files/` contains the established installer runtime, including
bootstrap scripts, libraries, configuration, modules, planner components, and
the first-boot service.

`work/payload/daia/` contains the newly assembled distribution payload.

During ISO injection:

1. the established runtime is copied from `installer/files/`;
2. the assembled payload workspace is overlaid on top of it;
3. the merged result is placed into the extracted ISO.

This preserves the proven installer while the newer payload architecture is
introduced.

This arrangement is transitional.

The migration goal is:

```text
source components
  |
  v
build/build-payload.sh
  |
  v
work/payload/daia/
  |
  v
build/inject.sh
```

Once the assembled payload contains the complete runtime,
`build/inject.sh` should no longer need to copy `installer/files/` directly.

---

## 4.8 ISO Injection

The ISO injection stage places the completed DAIA payload and installer assets
into the extracted Debian ISO workspace.

The primary injection script is:

```text
build/inject.sh
```

Its responsibilities include:

* validating the extracted ISO workspace;
* validating installer inputs;
* validating the generated payload;
* removing previously injected content;
* copying the Debian Preseed configuration;
* copying the established installer runtime;
* overlaying the generated payload;
* copying installer hooks;
* applying executable permissions;
* verifying required injected files.

The resulting DAIA tree is placed in the ISO under:

```text
/daia
```

---

## 4.9 Debian Installer Hook

During Debian installation, the late-install hook deploys DAIA from the
installation media into the target filesystem.

The hook is:

```text
installer/hooks/late-install.sh
```

Its current responsibilities are:

1. verify the staged DAIA runtime;
2. create required target directories;
3. replace `/target/opt/daia`;
4. copy the DAIA runtime into `/target/opt/daia`;
5. install `daia-firstboot.service`;
6. enable the service for the first boot;
7. create an installer-completion marker.

The hook deploys files but does not perform the complete DAIA component
installation lifecycle.

---

## 4.10 First-Boot Service

The installed system includes:

```text
/etc/systemd/system/daia-firstboot.service
```

The service starts the DAIA first-boot process after Debian installation.

It executes the installed bootstrap entry point and ensures that system-level
configuration occurs inside the newly installed operating system rather than
inside the Debian installer environment.

The first-boot service remains enabled when bootstrap fails, allowing a later
boot to retry the operation.

After successful completion, the installer runtime disables the service.

---

## 4.11 Installed Installer Entry Point

The installed entry point is:

```text
/opt/daia/install.sh
```

Its responsibilities are limited to:

* loading DAIA configuration;
* loading shared libraries;
* validating required functions and commands;
* requiring root privileges;
* executing `bootstrap.sh`;
* disabling the first-boot service after success;
* reporting final completion.

`install.sh` assumes the DAIA payload has already been deployed.

It is not responsible for copying, unpacking, or assembling the payload.

---

## 4.12 Bootstrap and Module Framework

The installed bootstrap entry point is:

```text
/opt/daia/bootstrap.sh
```

Bootstrap coordinates enabled modules through the standard module lifecycle.

The current lifecycle is:

```text
framework_validate
module_validate
module_pre_install
module_install
module_post_install
module_verify
module_cleanup
record completion
framework_cleanup
```

Primary lifecycle operations and cleanup operations are handled separately so
cleanup can be attempted after both successful and failed module execution.

Bootstrap is responsible for orchestration.

Individual modules remain responsible for their own component-specific
behaviour.

A module may declare and implement operations involving:

* packages;
* files;
* configuration;
* services;
* verification;
* cleanup.

---

## 4.13 Shared Libraries

Shared libraries provide reusable system operations to installer and runtime
components.

Current library areas include:

* common validation and utility functions;
* logging;
* packages;
* services;
* modules;
* user-interface helpers;
* hardware detection;
* filesystem operations;
* configuration handling.

Shared libraries should expose reusable operations but should not determine
high-level product policy.

---

## 4.14 Runtime Engine

The Runtime Engine is a planned subsystem.

It will coordinate DAIA operations after the initial system installation.

Its precise contract must be defined before implementation.

Expected responsibilities may include:

* reading desired and current state;
* initiating reconciliation;
* applying execution plans;
* coordinating runtime handlers;
* recording operation state;
* exposing progress and failure information;
* invoking verification.

The Runtime Engine should reuse the established Planner, Builder, state, and
logging contracts rather than introducing a separate lifecycle model.

---

## 4.15 Reconciler

The Reconciler is intended to compare desired state with current state and
identify the operations required to bring the system into alignment.

Conceptually:

```text
desired state
      +
current state
      |
      v
reconciliation
      |
      v
required changes
```

The Reconciler should determine what has changed.

The Planner should determine the valid order in which those changes are
performed.

The Runtime Engine should coordinate their execution.

---

## 4.16 Verifier

The Verifier confirms that completed operations produced the intended system
state.

Verification should be based on observable results rather than only command
exit codes.

Examples include:

* a package is installed;
* a file exists with the required properties;
* a service is enabled;
* a service is active;
* an endpoint responds;
* a model is available;
* a configuration value is effective.

Verification results should contribute to current-state records and failure
reporting.

---

## 4.17 State

DAIA separates desired, current, cached, installation, and build-related state.

Current runtime state locations include:

```text
/opt/daia/state/desired/
/opt/daia/state/current/
/opt/daia/state/cache/
```

Installation markers may also exist under:

```text
/var/lib/daia/
```

State directories contain mutable information and should not be treated as
ordinary immutable payload content.

The architecture should define:

* which subsystem owns each state file;
* when state is written;
* whether writes must be atomic;
* how corrupted state is detected;
* how schema versions are managed;
* which state is authoritative.

---

## 4.18 Installation Wizard

The Installation Wizard is a planned user-facing subsystem.

Its primary responsibility is to gather and validate user intent.

The Wizard should:

* guide the user through appliance setup;
* display detected hardware;
* present valid configuration choices;
* explain dependencies and conflicts;
* generate configuration and desired state;
* show a final review;
* submit the resulting intent to the downstream architecture;
* display installation progress and results.

The Wizard must not:

* install packages directly;
* configure services directly;
* contain component-specific installation logic;
* construct build workspaces directly;
* bypass the Planner;
* duplicate the Runtime Engine.

The architectural principle is:

> The Wizard gathers intent. The rest of DAIA performs the work.

The Wizard will be designed incrementally when its supporting subsystem
contracts are stable.

---

## 5. Current Build and Installation Flow

The current end-to-end flow is:

```text
DAIA source tree
      |
      v
build/build-payload.sh
      |
      v
work/payload/daia/
      |
      |  plus transitional installer/files/
      v
build/inject.sh
      |
      v
extracted ISO /daia
      |
      v
rebuilt DAIA ISO
      |
      v
Debian installer
      |
      v
installer/hooks/late-install.sh
      |
      v
/target/opt/daia
      |
      v
daia-firstboot.service
      |
      v
/opt/daia/bootstrap.sh
      |
      v
module lifecycle
      |
      v
verification and state recording
```

---

## 6. Architectural Boundaries

The following boundaries should be preserved.

### 6.1 Wizard and Execution

The Wizard gathers choices.

It does not execute installation logic.

### 6.2 Desired State and Planning

Desired state describes the result.

The Planner determines the operations required to reach it.

### 6.3 Planning and Execution

The Planner constructs an ordered plan.

The Builder or Runtime Engine executes that plan.

### 6.4 Payload Assembly and Installation

The build system assembles the payload.

The installer deploys it.

Bootstrap configures the installed system.

### 6.5 Orchestration and Component Logic

Framework components coordinate lifecycle operations.

Modules and handlers contain component-specific behaviour.

### 6.6 Execution and Verification

Successful command execution does not automatically prove successful system
configuration.

Verification is a separate responsibility.

---

## 7. Canonical-Source Principle

Every DAIA component should eventually have one canonical source location.

Generated workspaces and installed files are outputs, not source locations.

The transitional duplication between `installer/files/` and the generated
payload must be removed incrementally.

A component should not be maintained independently in multiple source trees.

---

## 8. Error Handling Principles

DAIA components should:

* return explicit non-zero status on failure;
* preserve the original failure status where appropriate;
* record the lifecycle phase in which failure occurred;
* attempt safe cleanup;
* avoid continuing into dependent phases after failure;
* write actionable error messages;
* distinguish invalid input, missing resources, and execution failure;
* avoid silently ignoring partial results.

State and logs should make it possible to determine:

* what operation was attempted;
* when it started;
* which phase failed;
* what completed successfully;
* whether cleanup ran;
* what should be retried.

---

## 9. Security Principles

DAIA operates with system-level privileges during installation and runtime
management.

The architecture should therefore enforce:

* strict input validation;
* safe filesystem path handling;
* controlled privilege boundaries;
* explicit executable permissions;
* verified payload contents;
* avoidance of unsafe shell evaluation;
* safe handling of configuration values;
* predictable service ownership and permissions;
* auditable state changes.

Payload integrity, signatures, and stronger supply-chain verification are
future areas of development.

---

## 10. Testing Principles

Subsystems should be testable independently of the complete ISO build whenever
possible.

Testing should include:

* syntax validation;
* static analysis;
* unit-level behaviour;
* lifecycle behaviour;
* failure propagation;
* argument validation;
* state transitions;
* cleanup behaviour;
* integration testing;
* installation testing in a virtual machine;
* first-boot verification.

The complete ISO installation remains an important integration boundary but
should not be the only way to detect subsystem defects.

---

## 11. Current Architecture Status

| Subsystem                | Status                                |
| ------------------------ | ------------------------------------- |
| Configuration            | Implemented, evolving                 |
| Desired State            | Implemented in part                   |
| Registry components      | Implemented in part                   |
| Planner                  | Implemented                           |
| Builder                  | Implemented and behaviourally tested  |
| Payload assembly         | Implemented                           |
| ISO injection            | Implemented                           |
| Debian late-install hook | Implemented                           |
| First-boot bootstrap     | Implemented                           |
| Module framework         | Implemented                           |
| Runtime Engine           | Planned                               |
| Reconciler               | Early structure / planned development |
| Verifier                 | Early structure / planned development |
| Installation Wizard      | Planned                               |
| Documentation            | In progress                           |

---

## 12. Transitional Architecture

The most significant current transition is the payload source model.

Current state:

```text
installer/files/
       +
work/payload/daia/
       |
       v
merged ISO payload
```

Target state:

```text
canonical source components
       |
       v
build/build-payload.sh
       |
       v
work/payload/daia/
       |
       v
ISO payload
```

Migration should happen incrementally.

Each migration step should preserve:

* successful payload assembly;
* successful ISO injection;
* successful Debian installation;
* successful first boot;
* successful bootstrap;
* correct permissions;
* correct service activation.

---

## 13. Near-Term Architectural Priorities

The immediate priorities are:

1. document the implemented subsystems;
2. complete the migration away from the dual-source payload;
3. define the Runtime Engine contract;
4. define state ownership and state schemas;
5. connect planning, execution, reconciliation, and verification;
6. design the Installation Wizard when its downstream contracts are stable.

---

## 14. Documentation Map

This overview should be supported by focused documents such as:

```text
docs/architecture/
├── 00-overview.md
├── 01-installer.md
├── 02-planner.md
├── 03-builder.md
├── 04-payload.md
├── 05-runtime-engine.md
├── 06-module-framework.md
├── 07-configuration.md
└── 08-wizard.md
```

The subsystem documents should describe:

* purpose;
* responsibilities;
* non-responsibilities;
* public contracts;
* inputs and outputs;
* lifecycle;
* state;
* dependencies;
* failure behaviour;
* security considerations;
* tests;
* current limitations;
* future work.

---

## 15. Summary

DAIA is evolving into a layered appliance architecture.

Its major flow is:

```text
intent
  -> desired state
  -> planning
  -> build and payload assembly
  -> installation
  -> first-boot configuration
  -> runtime reconciliation
  -> verification
```

The current implementation already contains substantial portions of this
architecture.

The next stage is to document the existing contracts, remove transitional
duplication, and connect the completed subsystems through a clearly defined
Runtime Engine.

The Installation Wizard will eventually provide the user-facing entry point,
but it will remain a consumer of the architecture rather than becoming the
architecture itself.
