# DAIA Runtime Engine Architecture

## 1. Purpose

The DAIA Runtime Engine is the planned orchestration subsystem responsible for
applying changes to an installed DAIA system.

It will coordinate the transition from the system's current state to its
desired state by using the Planner, execution handlers, verification
components, and state-management facilities.

The Runtime Engine is not the installer.

The installer initializes a new DAIA appliance.

The Runtime Engine manages that appliance after installation.

---

## 2. Architectural Role

The Runtime Engine sits between planning and live-system execution.

Conceptually:

```text
Desired State
      +
Current State
      |
      v
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
      +-- Resource Handlers
      +-- Module Operations
      +-- Service Operations
      +-- File Operations
      +-- Package Operations
      +-- Verification
      +-- State Recording
      |
      v
Updated System State
```

The Runtime Engine coordinates execution.

It should not independently decide which system state is desired.

---

## 3. Status

The Runtime Engine is a planned subsystem.

Its implementation should begin only after its contracts with the following
components are defined:

* Desired State;
* Current State;
* Reconciler;
* Planner;
* execution plan;
* resource handlers;
* Verifier;
* state storage;
* logging.

This document defines the intended architecture.

It does not describe a completed implementation.

---

## 4. Responsibilities

The Runtime Engine is expected to be responsible for:

* accepting a validated execution plan;
* validating runtime preconditions;
* acquiring an operation lock;
* initializing operation state;
* executing planned operations in order;
* invoking the correct resource handlers;
* recording operation progress;
* preserving failure information;
* invoking verification;
* updating current state;
* attempting cleanup;
* reporting final status;
* supporting safe retry where possible.

The Runtime Engine answers:

> How are planned changes safely applied to the live DAIA system?

---

## 5. Non-Responsibilities

The Runtime Engine should not:

* gather user choices directly;
* replace the Installation Wizard;
* define desired state;
* resolve dependencies independently;
* alter execution-plan ordering;
* assemble ISO images;
* build distribution payloads;
* perform Debian installation;
* embed every resource implementation;
* consider command success alone to be verification;
* silently ignore drift or partial failure.

Those responsibilities belong to other subsystems.

---

## 6. Inputs

The Runtime Engine should accept a defined set of inputs.

Typical inputs include:

* validated execution plan;
* operation identifier;
* desired-state identifier;
* current-state snapshot;
* runtime configuration;
* handler registry;
* verification requirements;
* execution policy;
* cancellation or retry policy.

Inputs should be treated as immutable for the duration of one operation.

---

## 7. Outputs

The Runtime Engine should produce:

* operation status;
* phase status;
* per-step results;
* verification results;
* updated current state;
* logs;
* audit information;
* cleanup results;
* explicit exit status.

A successful operation means more than successful command execution.

It means that the planned changes were applied and verified.

---

## 8. Runtime Operation Lifecycle

A proposed Runtime Engine lifecycle is:

```text
Initialize
    |
    v
Acquire Lock
    |
    v
Load and Validate Plan
    |
    v
Capture Current State
    |
    v
Validate Preconditions
    |
    v
Execute Plan Steps
    |
    v
Verify Results
    |
    v
Update Current State
    |
    v
Cleanup
    |
    v
Finalize Operation
```

Failure in a required phase should prevent dependent phases from running.

Cleanup should be attempted after both success and failure where safe.

---

## 9. Initialization

Initialization prepares a runtime operation.

Expected tasks include:

* validate arguments;
* load configuration;
* verify root or required privileges;
* initialize logging;
* assign an operation identifier;
* initialize operation state;
* validate runtime directories;
* verify required commands;
* load handler and verifier registries.

If initialization fails, no system-changing operation should begin.

---

## 10. Operation Locking

Only one conflicting system mutation should run at a time.

The Runtime Engine should acquire a lock before changing the live system.

A lock should identify:

* operation identifier;
* process identifier;
* start time;
* operation type;
* owning component;
* optional plan identifier.

Possible lock location:

```text
/run/daia/runtime.lock
```

or:

```text
/var/lib/daia/locks/runtime.lock
```

The final path should be defined by the state architecture.

Lock acquisition must distinguish:

* active operation;
* stale lock;
* abandoned operation;
* non-conflicting concurrent operation.

A stale lock must not be removed without validation.

---

## 11. Execution Plan Validation

The Runtime Engine should validate the execution plan before applying it.

Validation should confirm:

* supported schema version;
* valid plan identifier;
* complete operation identifiers;
* valid handler references;
* valid dependency ordering;
* valid preconditions;
* valid verification requirements;
* valid resource paths;
* no unknown critical fields;
* no unsupported operation types.

The Runtime Engine should not repair an invalid plan silently.

An invalid plan should be rejected and returned to the planning layer.

---

## 12. Current-State Snapshot

Before execution begins, the Runtime Engine should capture the relevant current
state.

The snapshot may include:

* installed packages;
* enabled services;
* active services;
* managed files;
* configuration versions;
* container images;
* running containers;
* installed models;
* hardware state;
* previous operation state;
* resource ownership records.

The snapshot provides:

* precondition validation;
* drift detection;
* rollback information;
* verification comparison;
* audit history.

The scope of the snapshot should match the operation.

---

## 13. Preconditions

Each operation may declare preconditions.

Examples include:

* required package manager available;
* minimum disk space;
* required service stopped;
* resource not currently locked;
* expected file checksum;
* expected current version;
* supported hardware;
* required dependency already complete;
* network available when explicitly required.

Preconditions should be checked immediately before the relevant operation.

A precondition failure should stop that operation before mutation begins.

---

## 14. Execution Steps

An execution plan consists of ordered steps.

Each step should contain enough information for the Runtime Engine to invoke the
correct handler.

A conceptual step may include:

```yaml
id: install-container-runtime
type: package
action: install
handler: apt-package
resource: docker-ce
depends_on:
  - configure-package-source
preconditions:
  - package_manager_available
verify:
  - package_installed
```

The final schema should be versioned and documented separately.

---

## 15. Step States

Each execution step should have an explicit state.

Possible states include:

```text
pending
blocked
running
succeeded
failed
skipped
verified
rollback-pending
rolled-back
rollback-failed
```

The distinction between `succeeded` and `verified` is important.

A handler may report success before the Verifier confirms the intended state.

---

## 16. Handler Registry

The Runtime Engine should invoke operations through a Handler Registry.

A handler registry maps operation types to implementations.

Conceptually:

```text
package.install   -> package handler
file.write        -> file handler
service.enable    -> service handler
service.start     -> service handler
container.load    -> container handler
model.install     -> model handler
command.run       -> command handler
```

The Runtime Engine should not contain large inline implementations for every
resource type.

---

## 17. Handler Contract

A runtime handler should:

* validate its inputs;
* inspect relevant current state;
* perform one defined operation;
* return structured status;
* avoid terminating the Runtime Engine process;
* preserve useful failure output;
* support verification;
* support retry where practical;
* avoid modifying unrelated resources;
* use safe temporary files;
* report whether a change was made.

A handler should distinguish:

* already in desired state;
* changed successfully;
* failed before mutation;
* failed after partial mutation;
* unsupported operation.

---

## 18. Idempotency

Runtime operations must be safe to repeat where practical.

A handler should first determine whether the desired condition already exists.

Examples:

* an installed package should not be reinstalled unnecessarily;
* an enabled service should not be treated as failure;
* an identical file should not be rewritten;
* an already loaded container image should be recognized;
* an already installed model should be verified rather than duplicated.

Idempotency should not mean ignoring unexpected differences.

A handler should distinguish desired existing state from uncontrolled drift.

---

## 19. Resource Ownership

The Runtime Engine needs a clear ownership model.

DAIA-managed resources may include:

* files;
* packages;
* service units;
* configuration fragments;
* containers;
* models;
* directories;
* users;
* groups.

Ownership records should identify:

* resource identifier;
* owning DAIA component;
* source plan;
* desired-state version;
* installed version;
* last verified time;
* checksum or digest;
* whether external modification is allowed.

The Runtime Engine should not remove or overwrite externally owned resources
without an explicit policy.

---

## 20. Resource Handlers

Expected handler categories include:

### 20.1 Package Handler

Responsible for:

* package installation;
* package removal;
* version validation;
* repository use;
* package verification.

### 20.2 File Handler

Responsible for:

* file creation;
* atomic replacement;
* permissions;
* ownership;
* checksum validation;
* removal of managed files.

### 20.3 Directory Handler

Responsible for:

* directory creation;
* ownership;
* permissions;
* safe removal where allowed.

### 20.4 Service Handler

Responsible for:

* enablement;
* disablement;
* start;
* stop;
* restart;
* reload;
* active-state verification.

### 20.5 Container Handler

Responsible for:

* image import;
* image validation;
* container creation;
* runtime configuration;
* lifecycle management.

### 20.6 Model Handler

Responsible for:

* model installation;
* model verification;
* metadata registration;
* provider-specific import;
* disk-space validation.

### 20.7 Command Handler

Responsible for controlled execution of commands that do not fit a more
specific resource handler.

The command handler should be used sparingly because arbitrary commands are
harder to reason about and verify.

---

## 21. Execution Ordering

The Runtime Engine must preserve Planner-defined ordering.

A step may run only when:

* all required dependencies succeeded;
* all required dependencies were verified;
* its preconditions pass;
* no conflicting operation is active;
* execution policy permits it.

The Runtime Engine should not reorder steps for convenience.

Future parallel execution may be allowed only when the plan explicitly proves
independence.

---

## 22. Failure Propagation

When a required step fails:

1. record the step failure;
2. stop dependent steps;
3. preserve the original status;
4. identify whether partial mutation occurred;
5. attempt defined cleanup;
6. invoke rollback only when supported;
7. record final operation failure;
8. leave enough state for diagnosis and retry.

Unrelated independent branches may eventually continue under a defined policy,
but the initial implementation should prefer predictable sequential failure
behavior.

---

## 23. Partial Failure

A live-system operation may fail after changing the system partially.

The Runtime Engine should record:

* which actions completed;
* which action failed;
* whether the failing action changed state;
* which dependent actions were blocked;
* whether cleanup succeeded;
* whether rollback is available;
* whether manual intervention is required.

Partial failure must never be represented as a clean success.

---

## 24. Cleanup

Cleanup is distinct from rollback.

Cleanup removes temporary execution resources.

Examples include:

* temporary files;
* staging directories;
* locks;
* transient mounts;
* helper processes;
* temporary credentials;
* package-manager locks acquired by DAIA.

Cleanup should not reverse valid completed system changes unless that behavior
is part of an explicit rollback.

---

## 25. Rollback

Rollback is a future capability and should be introduced carefully.

Not every resource operation is safely reversible.

Possible rollback examples include:

* restoring a previous file;
* reverting a symlink;
* disabling a newly enabled service;
* restoring previous configuration;
* removing a newly created container.

Potentially unsafe rollback examples include:

* downgrading packages;
* deleting user data;
* reverting database migrations;
* removing models used by another component;
* reversing externally modified resources.

Each handler should declare its rollback capability.

The Runtime Engine should never claim transactional behavior across operations
that cannot actually be reversed.

---

## 26. Verification

Verification confirms that execution produced the intended result.

The Runtime Engine should invoke verification:

* after individual critical steps;
* after groups of related steps;
* at final operation completion.

Verification examples include:

* package version installed;
* file checksum correct;
* service enabled;
* service active;
* endpoint healthy;
* container running;
* model available;
* configuration effective.

Verification failure should cause operation failure even when the handler
returned success.

---

## 27. Verifier Contract

A Verifier should:

* accept a resource and expected state;
* inspect observable system state;
* return structured success or failure;
* provide evidence;
* avoid changing the system;
* distinguish unavailable evidence from negative evidence;
* support timeout where required.

Verification should be read-only wherever possible.

---

## 28. Current-State Update

Current state should be updated only after verification.

Conceptually:

```text
execute operation
      |
      v
handler success
      |
      v
verification success
      |
      v
record current state
```

If verification fails, the desired result must not be recorded as achieved.

State writes should be atomic.

---

## 29. Desired-State Immutability During Execution

The desired-state version associated with an operation should remain fixed
during execution.

If desired state changes while an operation is running:

* the running operation should continue against its original snapshot;
* the new desired state should produce a later reconciliation;
* the running plan should not be modified in place.

This prevents unpredictable mid-operation behavior.

---

## 30. Operation State

A runtime operation should have a persistent state record.

Recommended fields include:

```yaml
schema_version: 1
operation_id: op-20260723-001
plan_id: plan-20260723-001
desired_state_id: desired-42
status: running
phase: execute
current_step: install-container-runtime
started_at: 2026-07-23T04:00:00Z
updated_at: 2026-07-23T04:02:30Z
```

The final schema should also support:

* failure status;
* cleanup status;
* verification status;
* retry status;
* completion time;
* actor;
* source request.

---

## 31. Runtime State Locations

Possible runtime state locations include:

```text
/opt/daia/state/desired/
/opt/daia/state/current/
/opt/daia/state/cache/
```

System-level mutable state may be better placed under:

```text
/var/lib/daia/
```

Transient state may belong under:

```text
/run/daia/
```

The final state architecture should distinguish:

* immutable shipped metadata;
* persistent system state;
* transient operation state;
* cache;
* logs;
* locks.

---

## 32. Logging

The Runtime Engine should produce structured operation logs.

Useful fields include:

* timestamp;
* operation identifier;
* plan identifier;
* desired-state identifier;
* step identifier;
* resource identifier;
* handler;
* phase;
* action;
* result;
* exit status;
* verification result;
* duration.

Logs should make it possible to reconstruct the complete operation timeline.

Sensitive data should be redacted.

---

## 33. Progress Reporting

The Runtime Engine should expose progress independently of its user interface.

Progress events may include:

```text
operation-started
phase-started
step-started
step-skipped
step-succeeded
step-failed
verification-started
verification-succeeded
cleanup-started
operation-completed
operation-failed
```

The Installation Wizard, command-line tools, and future management interfaces
should consume the same progress events.

The Runtime Engine should not contain user-interface formatting logic.

---

## 34. Cancellation

Cancellation is a future capability.

Cancellation should distinguish:

* cancellation before mutation;
* cancellation between steps;
* cancellation during a handler;
* cancellation during verification;
* cancellation during cleanup.

A handler should declare whether it supports interruption.

The Runtime Engine should never terminate a system-changing process abruptly
without understanding the possible resulting state.

Initial implementations may support safe cancellation only between steps.

---

## 35. Retry

A failed operation may be retried.

Retry behavior should determine:

* whether the same plan remains valid;
* whether current state changed;
* whether completed steps should be skipped;
* whether failed steps are idempotent;
* whether cleanup completed;
* whether a new plan is required.

A retry should not simply rerun every command blindly.

The preferred flow is:

```text
capture current state
      |
      v
reconcile again
      |
      v
generate fresh plan
      |
      v
execute remaining required work
```

---

## 36. Recovery After Restart

The Runtime Engine should survive process or system interruption.

At startup, it should be able to detect:

* an operation marked running;
* a stale lock;
* incomplete step state;
* unfinished cleanup;
* partially written state;
* unknown handler outcome.

Recovery should be conservative.

An interrupted operation should not automatically be marked failed or
successful without inspecting observable state.

---

## 37. Concurrency

The initial Runtime Engine should prefer serialized system mutations.

Future concurrency may be supported for proven independent resources.

Concurrency rules must consider:

* package-manager locks;
* service dependencies;
* shared files;
* disk bandwidth;
* GPU resources;
* container runtime locks;
* state-store writes;
* handler thread safety.

Parallel execution must be explicitly allowed by the plan.

---

## 38. Security

The Runtime Engine will perform privileged live-system operations.

Security controls should include:

* strict plan validation;
* trusted handler registry;
* safe command execution;
* no arbitrary shell evaluation;
* path validation;
* privilege minimization;
* safe temporary files;
* controlled environment variables;
* explicit resource ownership;
* secret redaction;
* audit logging;
* integrity verification.

Handlers should receive only the privileges and inputs they require.

---

## 39. Command Execution

Where commands must be executed, the Runtime Engine should:

* pass arguments as structured arrays where possible;
* avoid string-based shell evaluation;
* define the execution environment;
* set timeouts where appropriate;
* capture standard output and error;
* preserve exit status;
* limit inherited file descriptors;
* avoid exposing secrets in process arguments.

Commands should be wrapped by resource-specific handlers rather than being
scattered across the orchestration layer.

---

## 40. Runtime Engine and Planner

The Planner and Runtime Engine must remain separate.

The Planner determines:

> What operations are required and in what order?

The Runtime Engine determines:

> How are those operations safely coordinated on the live system?

The Runtime Engine should reject an invalid plan rather than reinterpret it.

---

## 41. Runtime Engine and Reconciler

The Reconciler compares desired state with current state.

It identifies the differences requiring action.

Conceptually:

```text
Desired State
      +
Current State
      |
      v
Reconciler
      |
      v
Required Changes
      |
      v
Planner
      |
      v
Execution Plan
      |
      v
Runtime Engine
```

The Runtime Engine should not duplicate reconciliation logic inside handlers.

---

## 42. Runtime Engine and Builder

The Builder and Runtime Engine share orchestration principles but operate in
different environments.

The Builder coordinates artifact-producing operations.

The Runtime Engine coordinates live-system mutations.

Shared concepts may include:

* context;
* lifecycle phases;
* callbacks;
* state recording;
* failure preservation;
* cleanup;
* logs.

Shared code should be extracted only where the contracts are genuinely
identical.

The Runtime Engine should not be implemented as a renamed Builder without
accounting for live-system risk.

---

## 43. Runtime Engine and Installer

The installer prepares and initializes the system.

The Runtime Engine manages later changes.

First-boot bootstrap may eventually use some Runtime Engine primitives, but the
initial installer lifecycle should not be replaced until runtime contracts are
stable and tested.

Migration should occur incrementally.

---

## 44. Runtime Engine and Wizard

The Wizard should interact with the Runtime Engine indirectly.

Intended flow:

```text
Wizard
   |
   v
Desired State
   |
   v
Reconciler and Planner
   |
   v
Execution Plan
   |
   v
Runtime Engine
   |
   v
Progress Events
   |
   v
Wizard
```

The Wizard gathers intent and displays progress.

It does not invoke package managers, services, or handlers directly.

---

## 45. Runtime Engine and State Manager

The Runtime Engine should not write arbitrary state files independently.

A State Manager should provide:

* schema validation;
* atomic writes;
* locking;
* version handling;
* migration;
* ownership rules;
* corruption detection;
* read consistency.

The Runtime Engine should use this contract for operation and current-state
records.

---

## 46. Runtime Engine and Verifier

Execution and verification must remain distinct.

The handler reports whether its operation completed.

The Verifier reports whether the intended system condition exists.

Conceptually:

```text
Handler
   |
   v
operation result
   |
   v
Verifier
   |
   v
observable state result
```

This distinction prevents false success caused by commands that exit cleanly
without producing the intended result.

---

## 47. Event Interface

The Runtime Engine should publish structured events.

A conceptual event may resemble:

```json
{
  "schema_version": 1,
  "event": "step_succeeded",
  "operation_id": "op-20260723-001",
  "plan_id": "plan-20260723-001",
  "step_id": "enable-ai-service",
  "timestamp": "2026-07-23T04:12:00Z"
}
```

Events may initially be written to:

* a log stream;
* a state file;
* a named pipe;
* standard output.

The transport can evolve independently of the event schema.

---

## 48. Public Interface

The first Runtime Engine interface may be a command-line entry point.

A possible conceptual interface is:

```bash
daia-runtime apply --plan /path/to/plan.json
daia-runtime status --operation OPERATION_ID
daia-runtime verify --operation OPERATION_ID
daia-runtime recover
```

The final command names are not yet defined.

The interface should return explicit exit statuses and machine-readable output.

---

## 49. Exit Statuses

The Runtime Engine should define stable exit-status categories.

Possible categories include:

```text
0   success
2   invalid arguments
3   invalid configuration
4   invalid plan
5   lock unavailable
6   precondition failure
7   handler failure
8   verification failure
9   cleanup failure
10  state failure
11  recovery required
```

The exact values should be finalized before implementation.

Detailed handler statuses should be preserved in operation state even when the
public interface maps them to broader categories.

---

## 50. Testing Strategy

Runtime Engine testing should begin with lifecycle behavior.

### 50.1 Unit Tests

Test:

* plan validation;
* state transitions;
* dependency gating;
* lock behavior;
* failure preservation;
* cleanup invocation;
* verification gating;
* current-state update rules.

### 50.2 Handler Contract Tests

Every handler should pass a common contract suite covering:

* already desired state;
* successful change;
* validation failure;
* precondition failure;
* partial failure;
* verification success;
* verification failure;
* retry behavior.

### 50.3 Test Doubles

The Runtime Engine should support fake handlers and fake verifiers.

Lifecycle testing should not require real package installation or service
changes.

### 50.4 Integration Tests

Integration testing should cover real:

* file operations;
* service operations;
* package operations;
* state persistence;
* process restart recovery;
* lock contention.

### 50.5 Virtual Machine Tests

VM testing should cover:

* complete desired-state application;
* system reboot during operation;
* handler failure;
* verification failure;
* retry;
* drift correction;
* interrupted cleanup.

---

## 51. Initial Implementation Scope

The first Runtime Engine version should remain intentionally small.

Recommended initial scope:

1. sequential execution;
2. one operation lock;
3. versioned execution-plan input;
4. structured operation state;
5. file handler;
6. service handler;
7. command handler only where unavoidable;
8. external verifier callbacks;
9. cleanup lifecycle;
10. explicit final status.

Features such as parallel execution, rollback, cancellation, and remote control
should follow later.

---

## 52. Proposed Internal Components

A possible initial structure is:

```text
runtime/
├── runtime-engine.sh
├── runtime-context.sh
├── runtime-state.sh
├── plan-loader.sh
├── step-executor.sh
├── handler-registry.sh
├── verifier-registry.sh
├── lock-manager.sh
├── event-writer.sh
└── handlers/
    ├── file-handler.sh
    ├── directory-handler.sh
    ├── service-handler.sh
    └── command-handler.sh
```

This structure is illustrative.

Final names should follow the repository's established conventions.

---

## 53. Implementation Sequence

Recommended implementation order:

1. define the execution-plan schema;
2. define runtime operation state;
3. define handler result schema;
4. implement runtime context;
5. implement operation locking;
6. implement plan loading and validation;
7. implement sequential step execution;
8. implement fake handlers for tests;
9. implement verification gating;
10. implement cleanup behavior;
11. implement real file and service handlers;
12. add recovery detection;
13. integrate with current-state storage;
14. integrate with Reconciler and Planner.

The lifecycle should be behaviourally tested before adding complex handlers.

---

## 54. Open Design Questions

The following questions require decisions before implementation:

* Where should persistent runtime state live?
* What is the canonical execution-plan format?
* What fields identify resource ownership?
* Can independent plan branches execute concurrently?
* Which failures permit automatic retry?
* Which operations support rollback?
* How is recovery triggered after reboot?
* What component owns event transport?
* How are handler plugins registered?
* How is current-state drift represented?
* Can first-boot bootstrap reuse the Runtime Engine?
* What authentication model will future remote control use?

These questions should be resolved through focused design documents or
architecture decisions.

---

## 55. Design Principles

The Runtime Engine should follow these principles:

1. desired state is immutable during one operation;
2. planning and execution remain separate;
3. plan order is authoritative;
4. handlers perform narrow resource operations;
5. execution and verification remain separate;
6. state is written atomically;
7. original failures are preserved;
8. cleanup is attempted after failure;
9. retries are based on observed state;
10. live-system mutation is serialized initially;
11. progress is exposed as structured events;
12. user interfaces remain outside the engine.

---

## 56. Success Criteria

The Runtime Engine should be considered functionally complete only when it can:

1. accept a valid execution plan;
2. reject an invalid plan;
3. acquire exclusive operation control;
4. execute steps in dependency order;
5. invoke registered handlers;
6. stop dependent work after failure;
7. preserve original error status;
8. run cleanup;
9. invoke verification;
10. update current state only after verification;
11. expose structured progress;
12. recover safely from interrupted operations;
13. pass behavioural and integration tests.

---

## 57. Summary

The DAIA Runtime Engine will be the live-system execution coordinator.

Its intended flow is:

```text
execution plan
    -> runtime validation
    -> precondition checks
    -> ordered handler execution
    -> verification
    -> current-state update
    -> cleanup
    -> final operation state
```

The Runtime Engine should reuse the architectural strengths already established
by the Builder:

* explicit lifecycle phases;
* callback-driven implementation;
* failure preservation;
* state recording;
* cleanup behavior;
* behavioural testing.

However, it must add the protections required for a live system:

* resource ownership;
* current-state awareness;
* operation locking;
* verification;
* interruption recovery;
* safe retry;
* controlled mutation.

The subsystem should be implemented only after its plan, state, handler, and
verification contracts are clearly defined.
