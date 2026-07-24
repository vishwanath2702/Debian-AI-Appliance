# DAIA Installer Architecture

## 1. Purpose

The DAIA installer is responsible for delivering the DAIA runtime and payload
onto a newly installed Debian system and arranging for DAIA initialization to
continue on first boot.

The installer spans several stages:

1. payload assembly;
2. ISO injection;
3. Debian installation;
4. late-install deployment;
5. first-boot activation;
6. bootstrap execution;
7. module installation and verification.

The installer is not a single script.

It is a coordinated pipeline containing build-time, installer-time, and
first-boot components.

---

## 2. Architectural Role

The installer sits between the DAIA build system and the installed runtime.

Its role is to transform an assembled DAIA distribution into an installed,
bootable, initialized appliance.

Conceptually:

```text
DAIA source
    |
    v
payload assembly
    |
    v
ISO injection
    |
    v
Debian installation
    |
    v
target filesystem deployment
    |
    v
first-boot initialization
    |
    v
configured DAIA system
```

The installer should deploy and initialize DAIA.

It should not become the long-term runtime management engine.

---

## 3. Installer Goals

The installer architecture is intended to provide:

### 3.1 Deterministic Deployment

The installed DAIA filesystem should be derived from a known payload assembled
during the build process.

### 3.2 Clear Stage Boundaries

Build-time, Debian-installer-time, and first-boot responsibilities should remain
separate.

### 3.3 Safe Failure Handling

A failure should stop dependent phases, preserve useful logs and state, and
allow recovery where appropriate.

### 3.4 Offline Installation

The installation process should be capable of using packages, images, models,
configuration, and other resources included on the DAIA installation media.

### 3.5 First-Boot Retry

If first-boot initialization fails, the system should retain enough state and
service configuration to retry on a later boot.

### 3.6 Minimal Installer Environment Logic

The Debian installer environment should perform only the operations that must
occur before the target system boots.

Complex DAIA configuration should run inside the installed operating system.

---

## 4. Installer Components

The current installer pipeline includes:

```text
build/build-payload.sh
build/inject.sh
installer/hooks/late-install.sh
installer/files/opt/daia/install.sh
installer/files/opt/daia/bootstrap.sh
installer/files/etc/systemd/system/daia-firstboot.service
installer/files/opt/daia/modules/
installer/files/opt/daia/lib/
```

The generated payload is assembled under:

```text
work/payload/daia/
```

The extracted ISO workspace receives the merged runtime under:

```text
work/extract/daia/
```

The rebuilt ISO exposes the runtime under:

```text
/cdrom/daia/
```

During Debian installation, the target filesystem is mounted under:

```text
/target/
```

The installed runtime is deployed to:

```text
/target/opt/daia/
```

After reboot, it becomes:

```text
/opt/daia/
```

---

## 5. End-to-End Installation Flow

The current installation flow is:

```text
DAIA source tree
      |
      v
build/build-payload.sh
      |
      v
work/payload/daia/
      |
      |  combined with installer/files/
      v
build/inject.sh
      |
      v
work/extract/daia/
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
/target/opt/daia/
      |
      v
daia-firstboot.service enabled
      |
      v
first boot
      |
      v
/opt/daia/install.sh
      |
      v
/opt/daia/bootstrap.sh
      |
      v
module lifecycle
      |
      v
verification and completion state
```

---

## 6. Build-Time Payload Assembly

The payload assembly stage is initiated by:

```text
build/build-payload.sh
```

Its role is to construct the generated DAIA payload workspace.

The generated workspace is:

```text
work/payload/daia/
```

The payload may contain:

* packages;
* container images;
* AI models;
* manifests;
* build metadata;
* branding;
* generated configuration;
* runtime resources;
* other offline installation assets.

Payload assembly occurs before ISO injection.

The payload builder should not directly modify the extracted ISO.

That responsibility belongs to the injection stage.

---

## 7. Transitional Dual-Source Runtime

The current installer uses two runtime sources:

```text
installer/files/
work/payload/daia/
```

These sources are merged during ISO injection.

The merge order is:

1. copy `installer/files/`;
2. overlay `work/payload/daia/`.

This means files in the generated payload can replace files copied from the
legacy installer tree when their paths overlap.

Conceptually:

```text
installer/files/
       |
       | copied first
       v
merged DAIA ISO tree
       ^
       | copied second
       |
work/payload/daia/
```

This arrangement protects the established installer runtime while the newer
payload architecture is introduced.

It is transitional and should not become permanent.

---

## 8. ISO Injection

The primary ISO injection entry point is:

```text
build/inject.sh
```

Its role is to place DAIA installer assets into the extracted Debian ISO
workspace.

Its responsibilities include:

* validating required input directories;
* validating the extracted ISO workspace;
* validating the generated payload;
* removing previously injected DAIA content;
* copying installer configuration;
* copying the established installer runtime;
* overlaying the generated payload;
* copying Debian installer hooks;
* setting executable permissions;
* verifying required output files.

The injection stage produces a DAIA tree inside the extracted ISO.

Conceptually:

```text
installer/files/
       +
work/payload/daia/
       +
installer hooks
       +
Debian installer configuration
       |
       v
work/extract/
```

The resulting ISO later exposes the DAIA content to the Debian installer under:

```text
/cdrom/daia/
```

---

## 9. Debian Installer Integration

DAIA integrates with the Debian installer through preconfigured installer
settings and a late-install hook.

The late-install hook runs near the end of the Debian installation process,
while the target root filesystem is mounted at:

```text
/target/
```

The hook is:

```text
installer/hooks/late-install.sh
```

Its primary role is filesystem deployment and first-boot activation.

It should not execute the full DAIA module installation lifecycle inside the
Debian installer environment.

---

## 10. Late-Install Hook Responsibilities

The current late-install hook performs the following major operations.

### 10.1 Validate the Staged Runtime

The hook verifies that required files exist on the installation media before
modifying the target system.

This prevents partial deployment when the ISO payload is incomplete.

### 10.2 Remove the Existing Target Runtime

The hook removes:

```text
/target/opt/daia
```

before copying the new runtime.

This avoids leaving stale files from a previous installation or retry.

### 10.3 Copy the DAIA Runtime

The hook copies:

```text
/cdrom/daia/opt/daia
```

to:

```text
/target/opt/daia
```

The payload is therefore treated as a filesystem image rather than as a package
installed file by file.

### 10.4 Install the First-Boot Service

The hook copies:

```text
/cdrom/daia/etc/systemd/system/daia-firstboot.service
```

to:

```text
/target/etc/systemd/system/daia-firstboot.service
```

### 10.5 Set Required Permissions

The hook ensures that primary runtime entry points are executable.

These include:

```text
/target/opt/daia/install.sh
/target/opt/daia/bootstrap.sh
```

Other executable permissions should ideally be established during payload
assembly and verified again during installation.

### 10.6 Enable First-Boot Initialization

The hook creates the appropriate systemd enablement link for:

```text
daia-firstboot.service
```

This causes the service to run after the installed system boots.

### 10.7 Record Installer Completion

The hook creates an installer-stage marker indicating that DAIA deployment into
the target filesystem completed successfully.

Installer markers should describe completed stages, not imply that the full
DAIA runtime installation has finished.

---

## 11. Why Full Bootstrap Runs After Reboot

The Debian installer environment is temporary and restricted.

Running the complete DAIA lifecycle inside that environment would create
several problems:

* the target system is mounted under `/target`;
* systemd is not operating as it will after boot;
* services cannot be managed normally;
* hardware and runtime behavior may differ;
* logging and recovery are more difficult;
* failures can interfere with Debian installation;
* module scripts may incorrectly act on the installer environment.

DAIA therefore separates deployment from initialization.

The late-install hook deploys the runtime.

The installed system performs initialization after reboot.

---

## 12. First-Boot Service

The first-boot service is:

```text
/etc/systemd/system/daia-firstboot.service
```

Its role is to invoke the installed DAIA initialization entry point.

Conceptually:

```text
system boot
    |
    v
daia-firstboot.service
    |
    v
/opt/daia/install.sh
```

The service should run only after the required local filesystems and operating
system facilities are available.

Its ordering should be explicit enough to avoid starting before the installed
system is ready.

Network availability should only be required when the selected installation
profile genuinely depends on it.

---

## 13. First-Boot Retry Behavior

The first-boot service should remain enabled until DAIA initialization
completes successfully.

Expected behavior:

```text
first boot
    |
    v
install.sh
    |
    +-- success --> disable first-boot service
    |
    +-- failure --> leave service enabled
                         |
                         v
                    retry next boot
```

This behavior allows recovery from transient failure.

However, retry must remain safe.

Bootstrap and module operations should therefore be idempotent or be able to
detect already completed phases.

Repeated execution must not corrupt system state.

---

## 14. Installed Entry Point

The installed entry point is:

```text
/opt/daia/install.sh
```

It is responsible for preparing and initiating bootstrap.

Its current responsibilities include:

* loading DAIA configuration;
* loading shared libraries;
* validating required functions;
* validating required commands;
* verifying root privileges;
* calling `bootstrap.sh`;
* propagating bootstrap failures;
* disabling the first-boot service after success;
* reporting final completion.

It assumes the payload has already been copied into place.

It does not:

* assemble the payload;
* copy the runtime from the ISO;
* install Debian itself;
* resolve component dependencies;
* implement individual module installation logic.

---

## 15. Bootstrap Entry Point

The bootstrap entry point is:

```text
/opt/daia/bootstrap.sh
```

Bootstrap coordinates the DAIA module framework.

Its responsibilities include:

* loading configuration;
* loading shared libraries;
* determining enabled modules;
* running framework validation;
* invoking each module lifecycle;
* recording module completion;
* running framework cleanup;
* returning explicit success or failure.

Bootstrap is an orchestrator.

It should not contain detailed implementation logic for every DAIA component.

---

## 16. Module Lifecycle

The current module lifecycle is:

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

### 16.1 Framework Validation

Framework validation checks global prerequisites required before module
execution begins.

Examples may include:

* required commands;
* required directories;
* valid configuration;
* supported operating system state;
* valid module registry.

### 16.2 Module Validation

Each module validates its own inputs, prerequisites, and environmental
assumptions.

### 16.3 Pre-Install

The pre-install phase prepares the environment for the module's main operation.

It should not duplicate full installation logic.

### 16.4 Install

The install phase performs the primary component-specific changes.

### 16.5 Post-Install

The post-install phase performs required configuration or integration after the
main installation operation.

### 16.6 Verify

The verify phase confirms that the installed component is in the intended
state.

Verification should examine observable system state.

### 16.7 Cleanup

Cleanup removes temporary files and reverses temporary setup where safe.

Cleanup should be attempted after both success and failure.

### 16.8 Completion Recording

A module should only be marked complete after successful verification and
required cleanup behavior.

---

## 17. Installer State

Installer-related state may exist in several locations.

### 17.1 Installation Markers

System-level installer markers may be stored under:

```text
/var/lib/daia/
```

These markers may indicate:

* late-install deployment completed;
* first-boot initialization completed;
* a migration step completed.

### 17.2 Runtime State

Runtime state may exist under:

```text
/opt/daia/state/
```

This may include:

* desired state;
* current state;
* cached information;
* module completion state;
* phase information.

### 17.3 Logs

Installer and bootstrap logs should be retained under a predictable location,
for example:

```text
/opt/daia/logs/
```

or an appropriate system log directory.

The authoritative location and retention policy should be documented
separately.

---

## 18. Marker Semantics

Markers must be precise.

A marker should communicate exactly which stage completed.

Examples:

```text
installer-payload-deployed
firstboot-started
firstboot-complete
module-container-runtime-complete
```

A late-install marker must not imply that bootstrap or all modules completed.

A marker should preferably include metadata such as:

* schema version;
* stage name;
* completion timestamp;
* payload version;
* build identifier;
* result;
* optional checksum or plan identifier.

Markers should be written atomically.

---

## 19. Idempotency

Installer stages may be retried.

Therefore, operations should be designed to tolerate repeated execution.

Examples include:

* replacing `/opt/daia` rather than merging stale files;
* creating directories with idempotent operations;
* enabling an already enabled service safely;
* skipping already verified modules;
* rewriting configuration deterministically;
* validating before destructive operations;
* preserving completed state when safe.

Idempotency does not mean ignoring errors.

A repeated operation should either:

* confirm the intended state already exists; or
* bring the system into that state safely.

---

## 20. Error Handling

Each installer stage should fail explicitly.

### 20.1 Build-Time Failure

If payload assembly fails:

* ISO injection must not continue;
* incomplete payload output should not be treated as valid;
* logs should identify the failed assembly phase.

### 20.2 Injection Failure

If ISO injection fails:

* ISO rebuilding must stop;
* incomplete injected content should not be published;
* required missing files should be named clearly.

### 20.3 Late-Install Failure

If the late-install hook fails:

* Debian installer failure should be explicit;
* the first-boot service should not be enabled unless the runtime is complete;
* partially copied files should not be treated as a valid installation.

### 20.4 First-Boot Failure

If `install.sh` or `bootstrap.sh` fails:

* the original failure status should be preserved;
* the first-boot service should remain enabled;
* logs and state should identify the failing phase;
* dependent operations should stop;
* cleanup should be attempted where safe.

---

## 21. Logging

Each stage should log enough information to reconstruct the installation
timeline.

A useful log entry should include:

* timestamp;
* component;
* stage;
* phase;
* action;
* result;
* error status;
* relevant resource or module identifier.

The logs should make it possible to answer:

* Was the payload assembled?
* Was the ISO injection complete?
* Did the late-install hook run?
* Was `/opt/daia` copied?
* Was the first-boot service enabled?
* Did first boot begin?
* Which module failed?
* Did verification complete?
* Was the first-boot service disabled?

Sensitive values should not be written to logs.

---

## 22. Security Boundaries

The installer runs with elevated privileges.

The following controls are therefore required.

### 22.1 Trusted Payload Source

The installer must only deploy content from the expected DAIA media path.

### 22.2 Path Validation

Scripts should avoid unsafe path expansion and should validate source and
destination paths before destructive operations.

### 22.3 Controlled Deletion

Operations such as:

```text
rm -rf /target/opt/daia
```

must be guarded by strict validation of the target root and destination path.

### 22.4 Permissions

Executable and writable permissions should be explicit.

Runtime code should not be unnecessarily writable by unprivileged users.

### 22.5 Input Validation

Configuration and installer values must be validated before use.

Shell values should not be evaluated as code.

### 22.6 Service Hardening

The first-boot systemd service should eventually include appropriate hardening
directives where compatible with its responsibilities.

---

## 23. Verification

Installation is not complete merely because files were copied.

Verification should occur at multiple boundaries.

### 23.1 Payload Verification

Before ISO injection:

* required paths exist;
* required manifests exist;
* expected entry points are executable;
* payload metadata is valid.

### 23.2 Injection Verification

After injection:

* expected DAIA files exist in the extracted ISO;
* hooks exist and are executable;
* first-boot service exists;
* runtime entry points exist.

### 23.3 Late-Install Verification

Before completing the hook:

* `/target/opt/daia` exists;
* required scripts exist;
* required permissions are present;
* the systemd unit exists;
* the enablement link is correct;
* the installer marker can be written.

### 23.4 First-Boot Verification

Before disabling the service:

* bootstrap returned success;
* required modules passed verification;
* required state was recorded;
* no critical installation phase remains incomplete.

---

## 24. Testing Strategy

The installer should be tested at several levels.

### 24.1 Syntax Validation

Run syntax checks on every shell script.

Example:

```bash
bash -n build/inject.sh
bash -n installer/hooks/late-install.sh
bash -n installer/files/opt/daia/install.sh
bash -n installer/files/opt/daia/bootstrap.sh
```

### 24.2 Static Analysis

Use ShellCheck on installer scripts.

### 24.3 Isolated Script Tests

Test:

* missing source directories;
* missing required files;
* invalid destination roots;
* failed copy operations;
* permission failures;
* failed bootstrap execution;
* service disablement behavior.

### 24.4 Payload Integration Tests

Verify that payload assembly produces all required installer inputs.

### 24.5 ISO Inspection Tests

After ISO generation, verify that expected files are present at the correct
paths.

### 24.6 Virtual Machine Installation Tests

A complete VM test should verify:

1. Debian installation succeeds;
2. the late-install hook runs;
3. `/opt/daia` is deployed;
4. the first-boot service is enabled;
5. first boot invokes `install.sh`;
6. bootstrap runs;
7. modules complete;
8. verification passes;
9. the first-boot service is disabled;
10. a second boot does not repeat completed installation unexpectedly.

### 24.7 Failure and Retry Tests

VM testing should also simulate:

* module installation failure;
* verification failure;
* power interruption;
* missing payload content;
* corrupted state;
* first-boot retry.

---

## 25. Current Limitations

The current installer architecture has several known limitations.

### 25.1 Dual Runtime Sources

Installer runtime files are currently assembled from both:

```text
installer/files/
work/payload/daia/
```

This creates transitional duplication.

### 25.2 Canonical Ownership

Not every installed file yet has one clearly documented canonical source.

### 25.3 State Contract

Installer markers and runtime state ownership require a formal schema.

### 25.4 Integrity Verification

Payload integrity and signature verification require further design.

### 25.5 Recovery Model

Retry behavior exists conceptually, but failure recovery and partial completion
semantics need formal documentation and testing.

### 25.6 Installer and Runtime Boundary

Some existing files may still reflect historical overlap between installation
and long-term runtime responsibilities.

---

## 26. Migration Target

The installer should eventually consume one generated payload.

Target architecture:

```text
canonical source files
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
        |
        v
late-install.sh
        |
        v
/opt/daia
```

In the target state:

* `installer/files/` is no longer copied as an independent runtime source;
* all runtime content is assembled through the payload builder;
* every installed file has one canonical source;
* injection consumes only generated installer outputs;
* payload validation occurs before injection.

---

## 27. Recommended Migration Order

The transitional runtime should be migrated incrementally.

Recommended order:

1. `install.sh`;
2. `bootstrap.sh`;
3. `daia-firstboot.service`;
4. shared libraries;
5. module framework;
6. planner runtime files;
7. configuration defaults;
8. manifests;
9. remaining runtime directories;
10. remove the `installer/files/` copy step;
11. delete obsolete duplicate sources.

After each migration step:

1. assemble the payload;
2. inspect the generated output;
3. inject the ISO;
4. verify expected paths;
5. install in a VM;
6. verify first boot;
7. verify retry behavior where relevant.

---

## 28. Non-Responsibilities

The installer should not:

* implement the Installation Wizard;
* decide arbitrary user preferences;
* replace the Planner;
* contain application-specific business logic;
* perform long-term runtime reconciliation;
* duplicate the Builder;
* silently fetch uncontrolled resources;
* consider file copying alone to be installation success;
* disable retry before verification completes.

---

## 29. Installer Contract Summary

The installer accepts:

* a valid assembled payload;
* Debian installation media;
* installer configuration;
* installer hooks;
* a supported target environment.

The installer produces:

* `/opt/daia`;
* the first-boot systemd service;
* service enablement;
* installer-stage state;
* first-boot bootstrap execution;
* module installation results;
* verification state;
* installation logs.

Successful installation means:

1. the payload was deployed completely;
2. first-boot initialization ran;
3. required modules completed;
4. verification succeeded;
5. completion state was recorded;
6. the first-boot service was disabled.

---

## 30. Summary

The DAIA installer is a multi-stage deployment pipeline.

Its current flow is:

```text
payload assembly
    -> ISO injection
    -> Debian late-install deployment
    -> first-boot service
    -> install.sh
    -> bootstrap.sh
    -> module lifecycle
    -> verification
```

The current implementation already provides this end-to-end path.

Its main architectural issue is not the absence of a payload system, but the
temporary existence of two payload sources.

The immediate installer priority is to complete the migration to one canonical
generated payload while preserving the proven installation and first-boot
behavior.
