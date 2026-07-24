# DAIA Module Framework Architecture

## 1. Purpose

The DAIA Module Framework provides a standard lifecycle for installing,
configuring, verifying, and cleaning up individual DAIA components.

A module encapsulates the implementation logic for one bounded capability or
system component.

Examples may include:

* container runtime;
* AI engine;
* model service;
* desktop environment;
* GPU runtime;
* monitoring service;
* storage component;
* networking component.

The Module Framework ensures that these components follow a consistent
execution contract.

---

## 2. Architectural Role

The Module Framework sits beneath bootstrap and runtime orchestration.

Conceptually:

```text
Bootstrap or Runtime Engine
          |
          v
Module Framework
          |
          +-- Module Registry
          |
          +-- Lifecycle Dispatcher
          |
          +-- Shared Libraries
          |
          +-- State Recorder
          |
          +-- Verification
          |
          v
Individual Modules
```

The orchestrator decides which modules should run.

The Module Framework determines how each selected module is invoked.

The module itself implements component-specific behavior.

---

## 3. Current Status

The Module Framework is currently used by the first-boot bootstrap process.

The established lifecycle includes:

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

The framework is therefore already part of the working installation path.

Future Runtime Engine integration may reuse or evolve this contract.

---

## 4. Responsibilities

The Module Framework is responsible for:

* discovering or loading modules;
* validating module definitions;
* enforcing lifecycle order;
* invoking lifecycle callbacks;
* passing shared context;
* preserving callback failures;
* stopping dependent phases after failure;
* attempting cleanup;
* recording module status;
* supporting verification;
* producing consistent logs.

The framework answers:

> How should DAIA component implementations participate in a common lifecycle?

---

## 5. Non-Responsibilities

The Module Framework does not:

* gather user intent;
* define desired state;
* resolve dependencies;
* generate execution plans;
* choose module order independently;
* implement every component directly;
* assemble payloads;
* build installation media;
* replace the Runtime Engine;
* treat callback success as final system verification without evidence.

Dependency ordering belongs to the Planner and execution plan.

---

## 6. Module Definition

A module is a bounded implementation unit representing one DAIA-managed
component or capability.

A module should contain:

* a stable identifier;
* lifecycle callbacks;
* metadata;
* dependency information;
* compatibility information;
* verification behavior;
* optional cleanup behavior;
* version information.

A module should have one clearly defined purpose.

Large components may be divided into several modules when they have separate
ownership, dependencies, or lifecycle behavior.

---

## 7. Example Module Layout

A representative module layout may be:

```text
modules/
└── container-runtime/
    ├── module.sh
    ├── metadata.yaml
    ├── config/
    ├── files/
    ├── templates/
    └── tests/
```

A simpler current implementation may use one script per module:

```text
modules/
├── container-runtime.sh
├── ai-engine.sh
├── model-service.sh
└── desktop.sh
```

The architecture should support gradual evolution without requiring immediate
restructuring of every existing module.

---

## 8. Module Identifier

Every module should have a stable machine-readable identifier.

Examples:

```text
container-runtime
ai-engine-ollama
model-service
gpu-runtime-nvidia
desktop-environment
```

Identifiers should:

* use a predictable naming convention;
* remain stable across releases;
* avoid display formatting;
* avoid embedding transient versions;
* be unique within the registry.

Human-readable names should be stored separately.

---

## 9. Module Metadata

A module should expose metadata describing its contract.

Recommended fields include:

```yaml
schema_version: 1
id: container-runtime
name: Container Runtime
version: 1.0.0
description: Installs and configures the supported container runtime

capabilities:
  provides:
    - container-runtime

dependencies:
  requires:
    - package-management
  optional: []

conflicts: []

platforms:
  architectures:
    - amd64
    - arm64

lifecycle:
  verify_required: true
  cleanup_required: true
```

The final metadata schema should be versioned.

---

## 10. Lifecycle Overview

The current module lifecycle is:

```text
Validate Framework
        |
        v
Validate Module
        |
        v
Pre-Install
        |
        v
Install
        |
        v
Post-Install
        |
        v
Verify
        |
        v
Cleanup
        |
        v
Record Completion
```

The framework should not advance to a dependent phase after a critical callback
fails.

---

## 11. Framework Validation

`framework_validate` checks global conditions required by the module system.

Examples include:

* required commands are available;
* runtime directories exist;
* configuration can be loaded;
* module registry is valid;
* state storage is writable;
* shared libraries are compatible;
* execution context is valid;
* required privileges are available.

Framework validation should occur before module-specific work begins.

A framework validation failure should stop the entire module lifecycle.

---

## 12. Module Validation

`module_validate` checks whether one module can run in the current context.

It may validate:

* required configuration;
* hardware compatibility;
* required source files;
* required packages;
* available disk space;
* expected dependency state;
* supported operating system;
* required credentials;
* selected profile compatibility.

Validation should not make substantial system changes.

Its purpose is to fail before mutation when possible.

---

## 13. Pre-Install Phase

`module_pre_install` prepares the system for the primary module operation.

Examples include:

* creating staging directories;
* stopping a service;
* preparing package sources;
* loading required kernel modules;
* backing up managed configuration;
* validating resource checksums;
* preparing temporary files.

Pre-install should remain focused on preparation.

It should not silently contain the module's complete installation logic.

---

## 14. Install Phase

`module_install` performs the primary component-specific work.

Examples include:

* installing packages;
* copying runtime files;
* importing container images;
* installing model files;
* creating users or groups;
* registering services;
* applying primary configuration.

The install phase should:

* return zero on success;
* return non-zero on failure;
* avoid terminating the parent orchestrator;
* preserve useful diagnostic output;
* report whether it changed the system.

---

## 15. Post-Install Phase

`module_post_install` performs integration and configuration that depends on the
main installation having completed.

Examples include:

* enabling a service;
* generating final configuration;
* registering the component;
* connecting dependent services;
* refreshing caches;
* updating runtime metadata;
* applying ownership and permissions.

Post-install should not run when install fails.

---

## 16. Verification Phase

`module_verify` confirms that the module reached its intended state.

Verification may inspect:

* installed package versions;
* file checksums;
* service enablement;
* service health;
* command output;
* API availability;
* container status;
* model availability;
* effective configuration.

Verification should rely on observable state.

A successful install callback does not make the module complete until
verification passes.

---

## 17. Cleanup Phase

`module_cleanup` releases temporary resources created during execution.

Examples include:

* removing staging files;
* restoring temporary configuration;
* removing temporary credentials;
* closing mounts;
* terminating helper processes;
* deleting partial downloads;
* releasing module-specific locks.

Cleanup should be attempted after success and failure where safe.

Cleanup is not rollback.

It should not reverse valid completed installation work unless explicitly
defined.

---

## 18. Framework Cleanup

`framework_cleanup` performs global cleanup after module execution.

Examples include:

* releasing framework locks;
* removing common temporary directories;
* closing shared resources;
* flushing state;
* finalizing logs.

Framework cleanup should be attempted even if a module callback fails.

A cleanup failure should be recorded without hiding the original module
failure.

---

## 19. Optional Callbacks

Not every module requires substantial logic in every phase.

A module may provide no-op callbacks where appropriate.

However, required callback names should remain consistent so the framework does
not need module-specific branching.

A possible contract is:

```bash
module_validate
module_pre_install
module_install
module_post_install
module_verify
module_cleanup
```

Each callback must exist or be supplied by a framework default.

Missing required callbacks should fail validation before execution.

---

## 20. Callback Signature

Callbacks should receive a documented execution context.

A shell-based module may receive context through:

* function arguments;
* exported read-only environment variables;
* a context file;
* shared library access.

The interface should make available only what the callback requires.

Possible context values include:

```text
DAIA_MODULE_ID
DAIA_OPERATION_ID
DAIA_PLAN_ID
DAIA_STATE_DIR
DAIA_LOG_DIR
DAIA_PAYLOAD_DIR
DAIA_CONFIG_DIR
DAIA_WORK_DIR
DAIA_TARGET_ROOT
```

Environment values should be validated and safely quoted.

---

## 21. Callback Return Contract

Callbacks should use explicit return statuses.

Basic contract:

```text
0       success
nonzero failure
```

The framework may later define broader error categories, such as:

```text
2  invalid module input
3  unsupported platform
4  missing dependency
5  precondition failure
6  installation failure
7  verification failure
8  cleanup failure
```

The original callback result should be retained in module state and logs.

---

## 22. Callbacks Must Return

A module callback should return control to the framework.

It should not call:

```bash
exit
```

when running inside a shared shell process.

Direct termination would prevent the framework from:

* recording failure state;
* running cleanup;
* preserving the original result;
* processing logs;
* releasing locks.

Standalone helper processes may exit normally, but the module wrapper must
capture and return their status.

---

## 23. Failure Propagation

Expected lifecycle behavior is:

```text
callback fails
      |
      v
record module and phase
      |
      v
stop dependent phases
      |
      v
attempt cleanup
      |
      v
preserve original failure
      |
      v
return failure to orchestrator
```

For example, if `module_install` fails:

* `module_post_install` must not run;
* normal verification should not claim success;
* cleanup should be attempted;
* completion state must not be written;
* the install failure remains the principal result.

---

## 24. Cleanup Failure

If a primary callback and cleanup both fail, the primary callback failure should
remain authoritative.

Example:

```text
module_install returns 12
module_cleanup returns 19
framework returns 12
```

The cleanup failure should still be recorded separately.

If primary execution succeeds but cleanup fails, policy must determine whether
the module is considered failed or completed with warning.

For the installation lifecycle, required cleanup failure should normally make
the module incomplete.

---

## 25. Module State

Each module execution should have persistent or inspectable state.

Recommended fields include:

```yaml
schema_version: 1
module_id: container-runtime
module_version: 1.0.0
operation_id: bootstrap-001
status: running
phase: install
started_at: 2026-07-23T04:00:00Z
updated_at: 2026-07-23T04:01:30Z
```

Final state may include:

```yaml
status: verified
completed_at: 2026-07-23T04:03:00Z
changed: true
verification:
  result: passed
```

---

## 26. Module Status Values

Possible module states include:

```text
pending
validating
ready
running
failed
cleanup-failed
verified
complete
skipped
blocked
```

The exact state machine should be formally documented.

A module must not be marked complete before successful verification.

---

## 27. Completion Records

Completion records should identify:

* module identifier;
* module version;
* operation identifier;
* plan identifier;
* desired-state identifier where applicable;
* completion time;
* verification result;
* installed resource versions;
* relevant checksums;
* whether the module changed the system.

A plain empty marker file may be sufficient temporarily, but structured state
is the long-term target.

Completion records should be written atomically.

---

## 28. Dependency Handling

Modules may declare dependencies, but the Module Framework should not perform
full dependency resolution independently.

The intended flow is:

```text
Module Registry
      |
      v
Planner resolves dependencies
      |
      v
Execution Plan orders modules
      |
      v
Module Framework executes order
```

The framework may verify that declared prerequisites are already complete.

It should reject an invalid execution order rather than silently redesign it.

---

## 29. Optional Dependencies

Some modules may enhance behavior when an optional capability is present.

Optional dependencies should be represented explicitly.

A module should not fail merely because an optional dependency is absent unless
the selected configuration requires that integration.

Optional behavior should remain deterministic and visible in logs.

---

## 30. Conflicts

Modules may declare conflicts.

Examples include:

* mutually exclusive service implementations;
* incompatible GPU runtimes;
* conflicting package providers;
* duplicate ownership of the same endpoint or port;
* incompatible configuration modes.

Conflicts should normally be detected by the Planner before execution.

The framework should still reject a known conflicting active state where it can
be observed safely.

---

## 31. Capabilities

Modules should describe capabilities they provide.

Example:

```yaml
capabilities:
  provides:
    - container-runtime
    - container-image-import
```

Desired state should generally request capabilities rather than hard-code
implementation details.

The Planner selects the module implementation.

The Module Framework executes the selected module.

---

## 32. Module Registry

The Module Registry provides authoritative metadata about available modules.

It should include:

* module identifier;
* source path;
* version;
* callbacks;
* provided capabilities;
* dependencies;
* conflicts;
* supported platforms;
* verification requirements;
* enabled or disabled status.

Registry loading should be deterministic.

Filesystem traversal order must not determine module behavior.

---

## 33. Module Discovery

Automatic discovery may scan a defined module directory.

However, discovery should not automatically trust arbitrary executable files.

A discovered module should be accepted only when:

* metadata is valid;
* the identifier is unique;
* the path is within an approved root;
* required callbacks exist;
* permissions are acceptable;
* the module is enabled by registry or plan;
* integrity requirements pass.

Explicit registry entries are safer than unrestricted script discovery.

---

## 34. Module Loading

Shell modules may be loaded using:

```bash
source
```

or executed in isolated processes.

Sourcing allows direct function callbacks but creates shared-process risks:

* global variable collisions;
* accidental shell-option changes;
* unexpected function overrides;
* direct process termination;
* shared working-directory changes.

Process isolation reduces these risks but requires a stronger data exchange
contract.

The current framework may use sourced callbacks, but modules should be written
as though shared state is hazardous.

---

## 35. Namespace Discipline

When shell modules are sourced, they should avoid generic global names.

Instead of:

```bash
validate
install
cleanup
```

the framework may use standardized lifecycle functions loaded one module at a
time, or namespaced functions such as:

```bash
container_runtime_validate
container_runtime_install
container_runtime_cleanup
```

The selected strategy should prevent functions from one module leaking into the
next module execution.

The framework should unset or isolate callbacks after use where appropriate.

---

## 36. Shell Option Isolation

A module should not permanently change parent-shell settings.

Potentially dangerous changes include:

```bash
set +e
set +u
set +o pipefail
shopt changes
IFS changes
umask changes
cd to another directory
```

The framework should either:

* execute modules in subshells;
* save and restore shell state;
* enforce strict callback rules.

A module should not rely on undocumented parent-shell behavior.

---

## 37. Environment Isolation

Modules should receive a controlled environment.

The framework should avoid exposing:

* unrelated secrets;
* build-host credentials;
* arbitrary user environment variables;
* unsafe `PATH` entries;
* uncontrolled locale behavior.

A predictable environment improves security and reproducibility.

---

## 38. Working Directories

Each module should have a dedicated working directory.

Example:

```text
/opt/daia/state/work/container-runtime/
```

or transiently:

```text
/run/daia/modules/container-runtime/
```

The framework should make clear whether a directory is:

* persistent;
* temporary;
* cache;
* log;
* installed state.

Modules should not use the repository or payload directory as mutable working
space.

---

## 39. Shared Libraries

Modules may use shared DAIA libraries for common behavior.

Examples include:

* logging;
* state access;
* command execution;
* filesystem operations;
* package management;
* service management;
* checksum verification;
* error formatting;
* configuration loading.

Shared libraries should provide stable interfaces.

Modules should not duplicate security-sensitive implementation when a trusted
shared helper exists.

---

## 40. Logging Contract

Every module should log through the shared logger where possible.

Useful fields include:

* timestamp;
* operation identifier;
* module identifier;
* module version;
* phase;
* action;
* resource;
* result;
* exit status;
* duration.

Module logs should make it possible to answer:

* which callback ran;
* which resource was changed;
* what failed;
* whether cleanup ran;
* whether verification passed;
* whether the module changed the system.

Sensitive data should be redacted.

---

## 41. Progress Events

Modules should expose progress through the framework rather than printing
interface-specific messages.

Possible events include:

```text
module-started
module-validation-succeeded
module-pre-install-started
module-install-started
module-post-install-started
module-verification-started
module-cleanup-started
module-completed
module-failed
```

A Wizard or command-line interface can translate these events into user-facing
progress.

---

## 42. Idempotency

Modules should be idempotent where practical.

Before applying a change, a module should inspect current state.

Examples:

* skip installing an already correct package;
* avoid rewriting an identical file;
* recognize an already enabled service;
* avoid importing an existing container image;
* verify an existing model before downloading or copying it again.

Idempotency should detect desired state, not suppress unexplained drift.

---

## 43. Retry

A failed module may be retried.

Retry behavior should account for:

* completed phases;
* partial installation;
* temporary files;
* changed configuration;
* service state;
* previous cleanup result;
* current verification evidence.

The safest retry pattern is usually:

```text
inspect current state
      |
      v
validate again
      |
      v
perform only remaining required work
      |
      v
verify
```

Blindly replaying all commands should be avoided.

---

## 44. Upgrade Behavior

A module may eventually support:

* initial installation;
* configuration update;
* software upgrade;
* repair;
* removal;
* verification-only execution.

These actions should be explicit.

A module should not infer an upgrade merely because files already exist.

Possible future lifecycle actions include:

```text
install
update
repair
remove
verify
```

The initial framework may support only installation and verification.

---

## 45. Removal

Module removal requires a stronger ownership contract than installation.

Before removing a resource, the module must know:

* DAIA owns the resource;
* no other active module depends on it;
* user data will not be deleted unexpectedly;
* shared packages or services remain needed or not;
* rollback or backup requirements are satisfied.

Removal should not be implemented as a simple reversal of install commands.

---

## 46. Verification Independence

Verification should ideally be separable from installation.

This allows DAIA to:

* verify after reboot;
* detect drift;
* run diagnostics;
* confirm recovery;
* inspect already installed systems.

A module's verification callback should therefore avoid relying only on
temporary install-phase variables.

It should be able to inspect persistent system state.

---

## 47. Module Configuration

Modules should receive validated configuration.

Configuration may come from:

* immutable defaults;
* selected profile;
* generated desired state;
* hardware facts;
* Planner-selected implementation parameters;
* runtime overrides.

Modules should not read arbitrary unrelated configuration files.

A module should reject unknown critical values rather than guessing.

---

## 48. Secrets

A module may occasionally require credentials or tokens.

Secrets should:

* not be embedded in module source;
* not be stored in payload defaults;
* not be printed in logs;
* not be passed in command-line arguments where avoidable;
* be provided through a controlled secret interface;
* be deleted from temporary storage during cleanup.

Secret management requires a separate architecture contract.

---

## 49. Resource Ownership

Modules should record resources they manage.

Examples include:

* files;
* directories;
* packages;
* service units;
* users;
* groups;
* ports;
* container images;
* containers;
* models.

Ownership records should support:

* verification;
* drift detection;
* safe upgrades;
* safe removal;
* conflict detection;
* audit history.

Two modules should not claim exclusive ownership of the same resource without a
shared-resource policy.

---

## 50. Security Requirements

Modules execute privileged operations and must be treated as trusted code.

Security requirements include:

* validated module sources;
* controlled registry entries;
* safe path handling;
* no untrusted shell evaluation;
* quoted variable use;
* safe temporary files;
* explicit permissions;
* verified external resources;
* restricted environment;
* no embedded credentials;
* controlled command execution;
* least privilege where possible.

A module should not execute arbitrary content from configuration.

---

## 51. File Operations

Modules should use safe file-update patterns.

Recommended flow:

```text
create temporary file
      |
      v
validate content
      |
      v
apply owner and permissions
      |
      v
atomic rename
```

Directly truncating critical configuration files should be avoided.

Existing files should be backed up only when policy requires it, and backups
must be managed rather than accumulated indefinitely.

---

## 52. Package Operations

Modules that install packages should use shared package abstractions where
possible.

They should record:

* requested package;
* resolved package version;
* package source;
* installation result;
* verification result.

Modules should not independently reconfigure package sources without declaring
that behavior.

Offline package handling should remain compatible with the Payload architecture.

---

## 53. Service Operations

Service-related modules should distinguish:

* installed;
* enabled;
* disabled;
* active;
* inactive;
* failed;
* healthy.

A service command returning zero does not prove service health.

Verification should inspect the actual state and, where appropriate, the
service's functional endpoint.

---

## 54. Container Operations

Container-related modules should record immutable image identity.

Prefer:

```text
repository@sha256:digest
```

over mutable tags alone.

Verification may inspect:

* image presence;
* image digest;
* container configuration;
* running state;
* health state;
* mounted resources;
* exposed endpoints.

---

## 55. Model Operations

Model modules should validate:

* model identity;
* version or revision;
* checksum;
* expected file size;
* provider;
* license;
* hardware requirements;
* disk-space requirements;
* runtime compatibility.

A model should not be marked installed merely because a directory exists.

---

## 56. Hardware Awareness

Modules may use hardware facts provided by the Planner or execution context.

Examples include:

* CPU architecture;
* GPU vendor;
* accelerator model;
* available memory;
* storage capacity;
* virtualization support.

A module should not independently select a different implementation when the
Planner has already chosen one.

It may reject execution if the actual hardware contradicts the plan.

---

## 57. Platform Compatibility

Module metadata should state supported platforms.

Possible compatibility dimensions include:

* operating system;
* operating-system version;
* architecture;
* kernel version;
* GPU vendor;
* GPU generation;
* required instruction sets;
* container runtime.

Compatibility should be checked before system mutation.

---

## 58. Module Versioning

Module implementation versions should be explicit.

Module versioning enables:

* state comparison;
* upgrade planning;
* compatibility checks;
* reproducibility;
* troubleshooting.

A module version is distinct from the version of the software it installs.

Example:

```yaml
module_version: 2.1.0
managed_software_version: 27.0.3
```

---

## 59. Metadata Schema Versioning

Module metadata should have a schema version.

Example:

```yaml
schema_version: 1
```

The framework should reject unsupported future schema versions.

Schema migration should be explicit.

The framework must not silently reinterpret unknown fields that affect
execution.

---

## 60. Testing Strategy

Module testing should occur at several levels.

### 60.1 Syntax Validation

Shell modules should pass:

```bash
bash -n
```

### 60.2 Static Analysis

Modules should pass ShellCheck or have documented suppressions.

### 60.3 Callback Contract Tests

Every module should be tested for:

* callback existence;
* successful lifecycle;
* validation failure;
* install failure;
* verification failure;
* cleanup execution;
* original failure preservation;
* no direct process termination.

### 60.4 Idempotency Tests

Run the module twice and confirm the second execution:

* succeeds;
* makes no unnecessary changes;
* still verifies desired state.

### 60.5 Failure Injection

Tests should simulate failures in:

* pre-install;
* install;
* post-install;
* verify;
* cleanup.

### 60.6 Integration Tests

Integration tests should run against realistic filesystem, package, service, or
container environments.

### 60.7 Virtual Machine Tests

System-level modules should be tested in disposable virtual machines.

---

## 61. Framework Test Doubles

The Module Framework should support test modules and fake callbacks.

A test module should be able to:

* record callback order;
* return configured statuses;
* simulate changed or unchanged state;
* simulate cleanup failure;
* expose verification evidence.

This allows framework behavior to be tested without performing real system
changes.

---

## 62. Contract Test Suite

A reusable module contract suite should verify that every module:

1. exposes valid metadata;
2. has a unique identifier;
3. implements required callbacks;
4. returns rather than exits;
5. preserves strict shell behavior;
6. handles missing configuration;
7. logs through the shared interface;
8. performs cleanup;
9. verifies observable state;
10. behaves safely when retried.

New modules should not be accepted without passing the common contract suite.

---

## 63. Current Installation Integration

During first boot, bootstrap performs the module lifecycle.

Conceptually:

```text
daia-firstboot.service
        |
        v
install.sh
        |
        v
bootstrap.sh
        |
        v
Module Framework
        |
        v
Enabled Modules
```

Bootstrap should receive the selected and ordered module set from validated
configuration or planning output.

It should not depend on accidental filesystem order.

---

## 64. Future Runtime Integration

The Runtime Engine may eventually invoke module actions as part of live-system
reconciliation.

Possible flow:

```text
Reconciler
    |
    v
Planner
    |
    v
Execution Plan
    |
    v
Runtime Engine
    |
    v
Module Action
```

The existing installation lifecycle should not be replaced until runtime
execution contracts are stable.

The framework may need to evolve from installation-only callbacks to explicit
actions such as install, update, remove, repair, and verify.

---

## 65. Relationship to the Planner

The Planner:

* selects modules;
* resolves dependencies;
* detects conflicts;
* establishes order;
* chooses implementations.

The Module Framework:

* validates module availability;
* executes callbacks;
* records status;
* verifies results;
* performs cleanup.

The framework must not reinterpret Planner decisions.

---

## 66. Relationship to the Builder

Builder plugins and runtime modules share similar lifecycle concepts.

Both may use:

* callbacks;
* shared context;
* logging;
* state;
* cleanup;
* failure propagation.

However, Builder plugins create artifacts.

Runtime modules mutate an installed or target system.

They should not be merged merely because their shell structures look similar.

Shared abstractions should be extracted only where semantics match.

---

## 67. Relationship to the Payload

Module code and static resources may ship inside the payload.

The payload determines:

* which module implementations are available offline;
* where module files are installed;
* module integrity;
* module versions;
* accompanying package, image, or model resources.

The Module Framework consumes installed module content.

It should be able to verify that the module matches the payload manifest.

---

## 68. Relationship to Verification

Module verification may be implemented as a module callback initially.

Long term, verification should integrate with a broader Verifier subsystem.

A module may declare verification requirements while a shared Verifier performs
standard resource checks.

This avoids every module reimplementing package, file, service, and container
inspection.

---

## 69. Relationship to State Management

The framework should use a State Manager rather than writing arbitrary marker
files.

The State Manager should provide:

* atomic writes;
* schema validation;
* locking;
* version handling;
* corruption detection;
* migration;
* consistent paths.

The framework owns module lifecycle state.

Individual modules may contribute resource details through the framework
interface.

---

## 70. Relationship to the Wizard

The Wizard should not invoke modules directly.

The intended flow is:

```text
Wizard
   |
   v
Desired State
   |
   v
Planner
   |
   v
Execution Plan
   |
   v
Bootstrap or Runtime Engine
   |
   v
Module Framework
```

The Wizard may display module progress events but should remain unaware of
module implementation details.

---

## 71. Current Limitations

Known limitations include:

### 71.1 Installation-Oriented Lifecycle

The current lifecycle is primarily designed around initial installation.

### 71.2 Informal Metadata

Module metadata and registry schemas require formalization.

### 71.3 State Format

Completion markers require migration toward structured state.

### 71.4 Callback Isolation

Sourced shell callbacks may share mutable process state.

### 71.5 Ownership Records

Managed resource ownership is not yet fully formalized.

### 71.6 Shared Verification

Standard verification logic requires broader framework support.

### 71.7 Action Model

Update, removal, repair, and reconciliation actions are not yet fully defined.

---

## 72. Recommended Near-Term Work

Recommended next steps are:

1. inventory existing modules;
2. document all current callbacks;
3. define the module metadata schema;
4. define the module registry schema;
5. define callback return behavior;
6. add common callback contract tests;
7. formalize module state;
8. ensure callback cleanup after failure;
9. detect direct `exit` usage;
10. add idempotency tests;
11. document resource ownership;
12. separate standard verification helpers.

---

## 73. Proposed Internal Structure

A possible future structure is:

```text
modules/
├── registry.yaml
├── framework/
│   ├── module-loader.sh
│   ├── module-runner.sh
│   ├── module-state.sh
│   ├── module-validator.sh
│   └── module-events.sh
│
└── implementations/
    ├── container-runtime/
    │   ├── metadata.yaml
    │   ├── module.sh
    │   ├── files/
    │   └── tests/
    │
    └── ai-engine/
        ├── metadata.yaml
        ├── module.sh
        ├── files/
        └── tests/
```

This layout is illustrative.

Migration should preserve the working bootstrap path.

---

## 74. Implementation Principles

The Module Framework should follow these principles:

1. one module has one bounded purpose;
2. module identifiers are stable;
3. lifecycle order is explicit;
4. callbacks return control;
5. failures stop dependent phases;
6. cleanup is attempted after failure;
7. original failures are preserved;
8. verification precedes completion;
9. dependency resolution remains in the Planner;
10. modules do not directly serve the user interface;
11. retry is based on observed state;
12. resource ownership is recorded;
13. shared code uses stable libraries;
14. metadata and state are versioned;
15. modules are tested through a common contract.

---

## 75. Module Contract Summary

A valid module provides:

* unique identifier;
* supported metadata schema;
* lifecycle callbacks;
* compatibility data;
* provided capabilities;
* dependency declarations;
* conflict declarations;
* verification behavior;
* version information.

The Module Framework provides:

* context;
* callback dispatch;
* lifecycle ordering;
* logging;
* state recording;
* cleanup coordination;
* failure propagation;
* progress events.

A module is complete only when:

1. framework validation passed;
2. module validation passed;
3. required installation phases succeeded;
4. verification passed;
5. required cleanup completed;
6. completion state was written.

---

## 76. Summary

The DAIA Module Framework standardizes how individual components participate in
installation and future runtime management.

Its current lifecycle is:

```text
framework validation
    -> module validation
    -> pre-install
    -> install
    -> post-install
    -> verify
    -> cleanup
    -> completion state
```

The framework separates orchestration from component-specific implementation.

This allows DAIA modules to remain independently testable while following a
consistent contract for:

* validation;
* execution;
* verification;
* cleanup;
* state;
* failure handling.

The immediate architectural priority is to formalize the existing working
framework through versioned metadata, structured state, callback contract
tests, and resource-ownership rules.
