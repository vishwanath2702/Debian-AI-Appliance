# DAIA Builder Architecture

## 1. Purpose

The DAIA Builder coordinates the execution of build operations required to
produce DAIA artifacts.

It provides a controlled lifecycle for:

* preparing a build workspace;
* executing planned plugin operations;
* invoking image construction;
* recording build state;
* handling failures;
* cleaning up temporary resources.

The Builder is an orchestration component.

It does not contain every build implementation directly. Instead, it invokes
specialized components through defined lifecycle contracts.

---

## 2. Architectural Role

The Builder sits between the Planner and artifact-producing implementations.

Conceptually:

```text
Planner
   |
   v
Execution Plan
   |
   v
Builder
   |
   +-- Workspace Builder
   |
   +-- Plugin Executor
   |
   +-- Image Builder
   |
   +-- Build State
   |
   +-- Logger
   |
   v
Build Artifacts
```

The Planner determines what must be built.

The Builder coordinates how the planned build is executed.

---

## 3. Responsibilities

The Builder is responsible for:

* validating build inputs;
* initializing build context;
* initializing build state;
* creating or preparing the workspace;
* executing plugin plan operations;
* invoking image-building callbacks;
* recording lifecycle phases;
* preserving failure status;
* attempting cleanup;
* reporting final success or failure.

The Builder answers:

> How should the planned build operations be coordinated?

---

## 4. Non-Responsibilities

The Builder does not:

* decide the desired system state;
* resolve dependencies;
* choose capabilities;
* construct the execution plan;
* directly implement every plugin;
* deploy the final payload into the installed system;
* perform long-term runtime reconciliation;
* interact directly with the installation Wizard.

These responsibilities belong to other DAIA subsystems.

---

## 5. Current Components

The Builder subsystem currently includes:

```text
builder/
├── build-context.sh
├── build-state.sh
├── builder.sh
├── logger.sh
├── plugin-executor.sh
└── workspace-builder.sh
```

Each component has a distinct responsibility.

---

## 6. Builder Entry Point

The primary Builder entry point is:

```text
builder/builder.sh
```

It coordinates the full Builder lifecycle.

Its responsibilities include:

* validating arguments;
* loading required components;
* preparing Builder state;
* invoking lifecycle callbacks;
* stopping dependent phases after failure;
* running cleanup;
* returning the correct final status.

The entry point should remain focused on orchestration rather than
component-specific implementation.

---

## 7. Build Context

The Build Context provides the shared configuration and paths required during a
Builder run.

It may include:

* source root;
* workspace root;
* output directory;
* execution plan path;
* temporary directories;
* build identifier;
* image destination;
* payload paths;
* log locations.

The Build Context should be initialized before build operations begin.

It should provide a stable contract for downstream Builder components.

---

## 8. Build State

Build State records the current lifecycle status of a Builder execution.

Typical state values may include:

```text
pending
running
succeeded
failed
```

The current phase should also be recorded.

Example phases include:

```text
initialize
workspace
plugins
image
cleanup
complete
```

Build State makes it possible to determine:

* whether a build started;
* which phase is currently running;
* which phase failed;
* whether cleanup was attempted;
* whether the build completed successfully.

---

## 9. Builder Lifecycle

The current high-level lifecycle is:

```text
Initialize
    |
    v
Prepare Workspace
    |
    v
Execute Plugin Plan
    |
    v
Build Image
    |
    v
Cleanup
    |
    v
Finalize State
```

Each phase must complete successfully before dependent phases begin.

Cleanup is treated separately and should be attempted after both successful and
failed primary execution.

---

## 10. Initialization Phase

Initialization prepares the Builder environment.

Typical operations include:

* validating required arguments;
* verifying required commands;
* loading shared libraries;
* initializing Build Context;
* initializing Build State;
* establishing log destinations;
* assigning a build identifier.

If initialization fails, no build phases should begin.

The failure should be recorded and returned explicitly.

---

## 11. Workspace Phase

The Workspace Builder prepares an isolated location for build operations.

Its responsibilities may include:

* creating the workspace directory;
* validating workspace paths;
* cleaning stale build data;
* preparing required subdirectories;
* staging source files;
* preparing image roots;
* establishing temporary storage.

The workspace should be deterministic and isolated from unrelated source files.

A failed workspace phase prevents plugin execution.

---

## 12. Plugin Execution Phase

The Plugin Executor runs the plugin operations described by the execution plan.

Conceptually:

```text
Execution Plan
      |
      v
Plugin Executor
      |
      +-- Plugin A
      +-- Plugin B
      +-- Plugin C
```

The Plugin Executor should:

* process plugins in plan order;
* validate required plugin callbacks;
* invoke the correct lifecycle operation;
* stop on critical failure;
* preserve the plugin failure status;
* identify the failing plugin;
* record execution progress.

The Plugin Executor must not independently reorder the plan.

---

## 13. Plugin Callback Contract

Builder plugins are invoked through a defined callback contract.

A plugin callback should:

* accept documented inputs;
* return zero on success;
* return non-zero on failure;
* avoid modifying unrelated state;
* write meaningful log messages;
* use Builder-provided paths;
* remain safe when retried where practical.

Callbacks should not terminate the entire Builder process directly.

They should return control to the Builder so the Builder can record failure and
perform cleanup.

---

## 14. Image Build Phase

After workspace and plugin execution complete, the Builder invokes the image
construction phase.

The image-building implementation may produce:

* filesystem images;
* ISO staging trees;
* appliance images;
* package bundles;
* other deployable artifacts.

The Builder should treat image construction as a callback or delegated
operation.

This preserves separation between orchestration and image-format-specific
implementation.

---

## 15. Cleanup Phase

Cleanup runs after primary build execution.

It should be attempted whether the build succeeds or fails.

Cleanup may include:

* removing temporary files;
* unmounting filesystems;
* deleting transient workspace content;
* releasing locks;
* terminating helper processes;
* closing temporary resources.

Cleanup must not erase logs or failure evidence required for diagnosis.

---

## 16. Primary Failure and Cleanup Failure

The Builder distinguishes between:

* primary lifecycle failure;
* cleanup failure.

If a primary phase fails and cleanup also fails, the original primary failure
should remain the principal result.

Conceptually:

```text
primary phase fails with status A
          |
          v
cleanup runs and fails with status B
          |
          v
Builder returns status A
```

The cleanup failure should still be logged and recorded.

Preserving the original failure prevents cleanup behavior from hiding the real
cause of the build failure.

---

## 17. Lifecycle Preconditions

Builder phases have explicit preconditions.

Examples:

* workspace preparation requires successful initialization;
* plugin execution requires a valid workspace;
* image building requires successful plugin execution;
* final success requires successful primary phases and acceptable cleanup.

The Builder should reject invalid direct phase transitions.

This prevents callers from invoking later phases against incomplete state.

---

## 18. Failure Propagation

The Builder must preserve meaningful failure status.

Expected behavior:

```text
callback returns non-zero
        |
        v
record failing phase
        |
        v
stop dependent phases
        |
        v
attempt cleanup
        |
        v
return original failure status
```

The Builder should not replace all failures with a generic status unless a
documented status-mapping contract requires it.

---

## 19. Logging

The Builder Logger records lifecycle events.

Useful log fields include:

* timestamp;
* build identifier;
* component;
* phase;
* plugin name;
* action;
* result;
* exit status;
* message.

The logs should make it possible to answer:

* When did the build begin?
* Which workspace was used?
* Which plugins ran?
* Which plugin failed?
* Was image construction attempted?
* Did cleanup run?
* What final status was returned?

Sensitive values should not be logged.

---

## 20. State and Logging Relationship

Logs and state serve different purposes.

Logs provide a chronological execution record.

Build State provides a compact representation of current or final status.

Example:

```text
Build State:
    status: failed
    phase: plugins
    plugin: container-runtime
    exit_status: 12
```

The corresponding log should contain the detailed sequence leading to that
failure.

Neither should be treated as a substitute for the other.

---

## 21. Determinism

Given the same:

* execution plan;
* source tree;
* configuration;
* dependency versions;
* build environment;

the Builder should produce equivalent results.

Sources of nondeterminism should be minimized or recorded.

Examples include:

* unordered filesystem traversal;
* unpinned remote resources;
* current timestamps embedded in artifacts;
* random identifiers;
* environment-dependent paths.

When nondeterministic values are necessary, they should not affect functional
artifact content unless explicitly intended.

---

## 22. Idempotency

Some Builder operations may be retried.

Builder components should therefore:

* detect existing workspace state;
* reject unsafe reuse;
* clean stale transient data;
* avoid silently combining unrelated builds;
* use unique build identifiers;
* validate outputs before considering them complete.

A retry should either reuse an explicitly resumable build or begin from a
known clean state.

The behavior must not be ambiguous.

---

## 23. Concurrency

Concurrent Builder executions may compete for:

* shared output paths;
* temporary directories;
* mounted filesystems;
* build caches;
* package caches;
* image destinations.

The architecture should prevent unsafe concurrent access.

Possible controls include:

* per-build workspace directories;
* lock files;
* unique build identifiers;
* atomic output publication;
* isolated temporary directories.

A partially written artifact must not appear as a completed artifact.

---

## 24. Output Publication

Final artifacts should be published only after successful completion.

Recommended flow:

```text
temporary output
      |
      v
build and verify
      |
      v
atomic rename or publish
      |
      v
final output
```

This prevents downstream components from consuming incomplete images.

Artifact metadata should identify:

* build identifier;
* version;
* creation time;
* source revision;
* plan identifier;
* checksum;
* result.

---

## 25. Security Considerations

Builder operations may process privileged filesystem trees and executable
content.

The Builder should enforce:

* validated workspace paths;
* controlled deletion;
* safe temporary directories;
* explicit executable permissions;
* trusted plugin sources;
* avoidance of unsafe shell evaluation;
* validation of plan data;
* restricted handling of secrets;
* safe archive extraction;
* checksum verification for imported resources.

Plugin callbacks should not receive more privilege than required.

---

## 26. Builder Testing

The Builder subsystem has behavioural tests covering the main lifecycle and
failure paths.

Current Builder testing includes:

* successful full execution;
* argument validation;
* initialization failure;
* lifecycle precondition enforcement;
* workspace callback failure;
* plugin callback failure;
* image callback failure;
* cleanup failure;
* cleanup after primary failure;
* original failure preservation;
* state recording;
* successful final state;
* invalid phase transitions;
* missing callback validation;
* final status propagation.

The current behavioural test suite contains 15 passing tests.

---

## 27. Static Validation

Builder shell scripts should pass:

```bash
bash -n
```

and:

```bash
shellcheck
```

Static validation is required but does not replace behavioural testing.

Syntax checks confirm that scripts can be parsed.

ShellCheck identifies common shell defects.

Behavioural tests confirm that the lifecycle contract actually works.

---

## 28. Test Isolation

Builder tests should avoid depending on:

* root privileges;
* real image construction;
* real package installation;
* production workspaces;
* external networks.

Callbacks should be replaceable with test doubles.

A test should be able to simulate:

* success;
* known failure status;
* partial output;
* cleanup failure;
* invalid input.

This allows lifecycle logic to be tested independently of heavy build
operations.

---

## 29. Integration Testing

Beyond behavioural tests, Builder integration testing should verify:

1. a real execution plan can be loaded;
2. a workspace is assembled correctly;
3. plugins operate in plan order;
4. expected payload files are produced;
5. image-building integration receives correct paths;
6. output metadata is written;
7. failed builds are not published;
8. cleanup leaves no unsafe mounts or locks.

Complete ISO generation is an important integration boundary.

---

## 30. Builder Contract

The Builder accepts:

* a valid Build Context;
* a validated execution plan;
* registered lifecycle callbacks;
* source inputs;
* output destinations.

The Builder produces:

* prepared workspace state;
* plugin execution results;
* build artifacts;
* build logs;
* build-state records;
* explicit final status.

Successful Builder completion means:

1. initialization succeeded;
2. workspace preparation succeeded;
3. all required plugin operations succeeded;
4. image construction succeeded;
5. required cleanup completed;
6. final state was recorded;
7. completed artifacts were published.

---

## 31. Relationship to Payload Assembly

The Builder and payload assembly are related but not identical.

The Builder coordinates execution.

Payload assembly determines the contents and layout of the DAIA distribution
payload.

Conceptually:

```text
Execution Plan
      |
      v
Builder
      |
      v
Payload Assembly Operations
      |
      v
work/payload/daia/
```

The Builder may invoke payload-related plugins or callbacks, but payload layout
and canonical-source rules belong to the Payload architecture.

---

## 32. Relationship to the Runtime Engine

The Builder is focused on artifact-producing build workflows.

The Runtime Engine will coordinate changes on an installed DAIA system.

They may share architectural patterns:

* lifecycle phases;
* state recording;
* logging;
* callback execution;
* cleanup;
* failure propagation.

However, they should not be treated as the same component.

The Runtime Engine must account for live-system state, reconciliation,
availability, rollback, and ongoing operations.

---

## 33. Relationship to the Installation Wizard

The Installation Wizard should not invoke internal Builder operations directly.

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
Builder
```

The Wizard gathers user intent.

The Planner translates that intent.

The Builder executes the resulting build plan.

This separation prevents user-interface logic from becoming build logic.

---

## 34. Current Limitations

Known areas requiring further work include:

### 34.1 Formal Plan Schema

The Builder should consume a versioned and formally documented execution-plan
schema.

### 34.2 Build-State Schema

Build State requires a documented persistent format and ownership model.

### 34.3 Artifact Manifest

Published artifacts should have a consistent manifest and checksum contract.

### 34.4 Concurrency Controls

Locking and concurrent-build behavior should be formally defined.

### 34.5 Resume Semantics

The distinction between retrying, resuming, and restarting a build needs a clear
contract.

### 34.6 Integration Coverage

Behavioural lifecycle testing is complete, but broader payload and ISO
integration coverage should continue to grow.

---

## 35. Future Enhancements

Potential Builder enhancements include:

* resumable builds;
* artifact caching;
* incremental rebuilds;
* parallel plugin execution where dependencies allow;
* signed artifact manifests;
* remote build workers;
* reproducibility reports;
* richer progress events;
* structured machine-readable logs;
* build-plan visualization.

These enhancements should preserve the existing lifecycle and failure-handling
principles.

---

## 36. Design Principles

The Builder architecture is guided by the following principles:

1. planning and execution remain separate;
2. orchestration and implementation remain separate;
3. lifecycle phases are explicit;
4. failures stop dependent work;
5. the original failure is preserved;
6. cleanup is always attempted where safe;
7. state and logs are both recorded;
8. outputs are published only after success;
9. tests validate behavior, not only syntax;
10. callbacks return control rather than terminating the process.

---

## 37. Summary

The DAIA Builder is the execution coordinator for artifact-producing build
operations.

Its lifecycle is:

```text
initialize
   -> prepare workspace
   -> execute plugin plan
   -> build image
   -> clean up
   -> finalize state
```

The Builder does not decide what should be built.

It receives that decision from the Planner and coordinates specialized
implementations through explicit callbacks.

The subsystem is currently implemented, statically validated, and covered by
15 passing behavioural tests.

Its next architectural improvements should focus on formalizing execution-plan
schemas, build-state formats, artifact publication, and integration testing
with the complete payload and ISO pipeline.
