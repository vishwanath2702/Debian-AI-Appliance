# DAIA Reconciliation Architecture

**Document:** `12-reconciliation.md`
**Architecture Version:** 1.0
**Status:** Draft
**Depends On:** `09-state-architecture.md`, `10-controller-framework.md`, `11-verification.md`

---

## 1. Purpose

This document defines the DAIA Reconciliation Architecture.

Reconciliation is the process by which DAIA compares accepted Desired State with verified Current State and coordinates work intended to reduce the difference between them.

Reconciliation answers:

> Given the accepted intent and the currently accepted representation of reality, what work should DAIA perform next?

Reconciliation does not own Desired State, Current State, verification truth, or resource-specific mutation logic.

Its responsibility is to coordinate convergence.

---

## 2. Scope

This document defines:

* reconciliation authority;
* reconciliation inputs and outputs;
* difference detection;
* planning;
* plan validity;
* State Basis validation;
* transition selection;
* dependency ordering;
* scheduling;
* execution coordination;
* verification coordination;
* retries and backoff;
* cancellation;
* compensation;
* recovery;
* unknown execution outcomes;
* drift reconciliation;
* convergence;
* quiescence;
* failure isolation;
* reconciliation conformance.

This document does not define:

* the canonical state model;
* State Acceptance persistence;
* verification evidence semantics;
* controller implementation;
* controller registration;
* Resource Type schemas;
* user configuration interfaces;
* general monitoring;
* implementation technology.

---

## 3. Normative Language

The terms **SHALL**, **SHALL NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

* **SHALL** indicates a mandatory architectural requirement.
* **SHALL NOT** indicates prohibited behavior.
* **SHOULD** indicates recommended behavior that may be departed from only for a documented reason.
* **SHOULD NOT** indicates discouraged behavior that may be used only for a documented reason.
* **MAY** indicates an optional capability.

---

## 4. Reconciliation Principle

DAIA SHALL reconcile state rather than execute fixed installation scripts.

The logical process is:

```text
Accepted Desired State
        +
Accepted Current State
        +
Applicable Policies
        +
Available Capabilities
        ↓
Difference Detection
        ↓
Reconciliation Planning
        ↓
Plan Validation
        ↓
Controller Execution
        ↓
Observation
        ↓
Verification
        ↓
State Acceptance
        ↓
Repeat Until Converged or Quiescent
```

Reconciliation SHALL operate from accepted state and verified evidence.

It SHALL NOT treat execution records as proof of external reality.

---

## 5. Architectural Authority

Reconciliation is the sole architectural authority permitted to coordinate convergence between Desired State and Current State.

Reconciliation SHALL:

* determine whether applicable resources require work;
* construct or obtain a valid Reconciliation Plan;
* validate the plan against its State Basis;
* coordinate eligible Resource Controllers;
* respect dependency and consistency boundaries;
* request verification before and after operations where required;
* respond to failures and unknown outcomes;
* determine whether another reconciliation cycle is required;
* distinguish convergence from quiescence.

Reconciliation SHALL NOT:

* modify accepted Desired State;
* directly commit Current State;
* declare evidence verified;
* implement Resource Type-specific mutation logic;
* redefine controller capabilities;
* override State Acceptance invariants;
* assume an operation succeeded because it was dispatched;
* assume an interrupted operation failed.

---

## 6. Core Terms

### 6.1 Reconciliation Cycle

A Reconciliation Cycle is one bounded evaluation of accepted state that may produce and execute a Reconciliation Plan.

### 6.2 Difference

A Difference is a semantically relevant mismatch between Desired State and Current State.

### 6.3 Reconciliation Plan

A Reconciliation Plan is an immutable description of proposed transitions intended to reduce one or more Differences.

### 6.4 Transition

A Transition is a proposed change from one resource condition toward another.

### 6.5 Action

An Action is an executable controller operation within a Transition.

### 6.6 State Basis

A State Basis is the immutable collection of state, policy, schema, dependency, and capability revisions against which the plan was produced.

### 6.7 Converged

A resource is Converged when all applicable Desired State requirements are verified as satisfied or are explicitly exempted by accepted policy.

### 6.8 Quiescent

The system is Quiescent when no immediately executable reconciliation work remains.

A quiescent system may still contain resources that are:

* blocked;
* failed;
* unknown;
* awaiting dependencies;
* awaiting a retry window;
* awaiting operator intervention.

### 6.9 Drift

Drift is a verified change in external reality that causes previously accepted Current State or satisfaction conclusions to become stale or incorrect.

### 6.10 Unknown Outcome

An Unknown Outcome exists when DAIA cannot establish whether an attempted external operation completed, partially completed, or did not occur.

### 6.11 Compensation

Compensation is a new transition intended to reduce or contain the effects of an earlier transition.

Compensation is not a reversal of history.

### 6.12 Recovery

Recovery is the process of re-establishing a valid basis for reconciliation after interruption, failure, or uncertainty.

---

## 7. Reconciliation Inputs

A Reconciliation Cycle SHALL operate on an explicit input set.

The input set SHALL include:

* accepted Desired State;
* accepted Current State;
* Desired Generation;
* Current Revision;
* applicable Resource Type schemas;
* dependency relationships;
* ownership information;
* applicable policy revisions;
* available controller capabilities;
* relevant operation records;
* relevant Verification Results;
* the cycle creation time;
* the State Basis.

The cycle MAY additionally include:

* requested resource scope;
* priority information;
* maintenance windows;
* retry eligibility;
* resource-group boundaries;
* operator constraints;
* environmental constraints.

Reconciliation SHALL NOT use untracked mutable inputs that are absent from the State Basis.

---

## 8. Reconciliation Triggers

A Reconciliation Cycle MAY be triggered by:

* acceptance of a new Desired Generation;
* acceptance of a new Current Revision;
* verified drift;
* controller capability changes;
* dependency changes;
* policy changes;
* retry eligibility;
* recovery after interruption;
* explicit operator request;
* scheduled re-evaluation;
* newly available resources;
* resolution of a blocking condition.

A trigger starts evaluation.

It does not imply that external work is necessary.

Multiple triggers MAY be coalesced when they apply to a compatible State Basis.

---

## 9. Reconciliation Scope

A cycle SHALL declare its scope.

The scope MAY be:

* one Managed Resource;
* a dependency subgraph;
* a consistency group;
* a Resource Type;
* all resources affected by a Desired Generation;
* the complete managed system.

Scope expansion SHALL be explicit.

A cycle MAY discover that additional dependencies must be evaluated, but it SHALL update or regenerate its State Basis before incorporating them into executable work.

---

## 10. Difference Detection

Difference Detection compares accepted Desired State with accepted Current State using Resource Type-specific semantic rules.

Difference Detection SHALL identify:

* the resource;
* the desired requirement;
* the accepted current condition;
* the semantic mismatch;
* the State Basis;
* whether verification is required before planning;
* whether the difference is actionable;
* whether the resource is blocked;
* whether the difference is exempted;
* whether no action is required.

Difference Detection SHALL NOT rely solely on literal object equality unless literal equality is defined by the Resource Type.

---

## 11. Difference Categories

A Difference MAY be classified as:

### 11.1 Missing

The Desired State requires a resource that verified Current State shows is absent.

### 11.2 Unexpected

Verified Current State contains a managed resource or condition prohibited by Desired State.

### 11.3 Misconfigured

The resource exists but its verified properties differ from Desired State.

### 11.4 Unhealthy

The resource satisfies structural requirements but fails a required health condition.

### 11.5 Integrity Violation

The resource fails a declared integrity requirement.

### 11.6 Version Mismatch

The resource version does not satisfy the desired constraint.

### 11.7 Dependency Unsatisfied

A required dependency is absent, unknown, incompatible, or unsatisfied.

### 11.8 Unknown

The system lacks acceptable evidence to determine whether a Difference exists.

### 11.9 Exempted

Accepted policy permits the Difference without corrective work.

### 11.10 Non-actionable

The Difference is known but no eligible transition is currently available.

Unknown SHALL NOT be converted into Missing or Unsatisfied without verification.

---

## 12. Pre-Planning Verification

Reconciliation SHALL request fresh verification before planning when:

* Current State is stale;
* a relevant operation occurred after the accepted Current Revision;
* an earlier outcome is unknown;
* evidence freshness requirements have expired;
* dependency state is uncertain;
* existing evidence conflicts;
* policy requires direct precondition evidence;
* external mutation may have occurred outside DAIA.

Pre-planning verification MAY eliminate the need for corrective work.

Reconciliation SHOULD avoid planning from stale assumptions when fresh observation is reasonably available.

---

## 13. Planning

Planning transforms actionable Differences into a Reconciliation Plan.

Planning SHALL:

* operate against an explicit State Basis;
* use declared Resource Type transition semantics;
* select only eligible controllers;
* respect dependencies;
* respect ownership;
* respect policy constraints;
* identify preconditions;
* identify postconditions;
* identify verification requirements;
* identify consistency boundaries;
* preserve deterministic decision inputs;
* avoid performing external mutation.

Planning is a pure architectural decision process.

Planning SHALL NOT execute actions.

---

## 14. Plan Structure

Every Reconciliation Plan SHALL identify:

* plan identity;
* creation time;
* initiating trigger;
* reconciliation scope;
* State Basis;
* Desired Generation;
* Current Revision;
* applicable policy revisions;
* capability and controller revisions;
* included Managed Resources;
* detected Differences;
* proposed Transitions;
* dependency relationships;
* execution ordering constraints;
* concurrency constraints;
* preconditions;
* postconditions;
* verification requirements;
* compensation declarations, when applicable;
* cancellation semantics;
* completion criteria;
* plan schema version.

A plan SHALL be immutable after publication.

Changes SHALL produce a new plan.

---

## 15. Transition Structure

Each Transition SHALL identify:

* transition identity;
* Managed Resource;
* starting accepted condition;
* target condition;
* Difference being addressed;
* selected Resource Controller;
* required capability;
* controller operation;
* operation parameters;
* dependencies;
* preconditions;
* postconditions;
* expected observations;
* required verification purpose;
* timeout semantics;
* retry classification;
* compensation information;
* State Basis.

A Transition SHALL describe intended architectural effect rather than implementation-specific shell sequencing.

---

## 16. Deterministic Planning

Planning SHALL be deterministic under semantically equivalent:

* Desired State;
* Current State;
* State Basis;
* policies;
* schemas;
* dependencies;
* controller capabilities;
* planning configuration.

Equivalent inputs SHALL produce semantically equivalent plans.

Plan identities and timestamps may differ.

External execution need not be deterministic.

The reasoning that selected a transition SHALL remain attributable to the planning inputs.

---

## 17. Minimal Planning

A Reconciliation Plan SHOULD contain only work required to reduce identified Differences.

Planning SHOULD avoid:

* reapplying already satisfied state;
* restarting healthy services without cause;
* reinstalling correct packages;
* rewriting equivalent configuration;
* replacing resources solely because a preferred command was not previously used;
* including unrelated resources.

A no-op plan is a valid outcome.

---

## 18. Dependency Model

A Transition MAY depend on:

* another resource reaching a verified condition;
* another Transition completing;
* a dependency Verification Result;
* a required capability;
* a policy condition;
* an environmental condition;
* a consistency-group decision.

Dependencies SHALL form a directed graph.

The graph SHALL be validated before execution.

Unresolved dependency cycles SHALL prevent execution of the affected graph unless the applicable Resource Type explicitly defines a valid grouped transition.

---

## 19. Dependency Ordering

Reconciliation SHALL execute a Transition only when all mandatory dependencies are satisfied or explicitly waived by accepted policy.

Dependency satisfaction SHALL be based on:

* accepted Current State;
* applicable fresh Verification Results;
* completed and verified preceding Transitions;
* declared State Basis.

Dispatch completion alone SHALL NOT satisfy a dependency.

---

## 20. Consistency Groups

Some resources require coordinated reconciliation.

A Consistency Group declares a boundary within which:

* planning must consider members together;
* partial execution may create invalid state;
* verification must consider a combined condition;
* compensation may apply to the group;
* concurrency may be restricted.

Examples may include:

* an application and its configuration;
* a service and required credentials;
* an AI engine and model storage;
* an interface and configured backend;
* a package set with version compatibility.

Consistency groups SHALL be explicit.

The existence of a database transaction SHALL NOT be assumed to make external resource changes atomic.

---

## 21. Controller Selection

A Transition SHALL reference a controller eligible under `10-controller-framework.md`.

Selection SHALL consider:

* Resource Type;
* required capability;
* resource schema version;
* controller version;
* platform compatibility;
* authority scope;
* declared operation support;
* verification support where required;
* applicable policy;
* State Basis.

Reconciliation SHALL NOT dispatch work to an ineligible controller.

Controller selection SHALL remain stable for one Transition unless the plan is invalidated and regenerated.

---

## 22. Plan Validation

Before execution, Reconciliation SHALL validate:

* plan schema;
* resource identities;
* Resource Type compatibility;
* controller eligibility;
* dependency graph validity;
* ownership constraints;
* policy constraints;
* precondition definitions;
* verification requirements;
* consistency boundaries;
* State Basis freshness;
* absence of prohibited concurrent work.

A plan that fails validation SHALL NOT execute.

---

## 23. State Basis Validation

The State Basis SHALL be validated:

* before execution begins;
* before each Transition whose validity depends on mutable state;
* before resuming suspended execution;
* before accepting post-execution verification;
* before requesting State Acceptance;
* when a relevant trigger occurs during execution.

A relevant basis change MAY include:

* a new Desired Generation;
* a newer Current Revision;
* changed resource ownership;
* changed dependency state;
* changed policy;
* changed schema;
* changed controller capability;
* changed verification policy;
* external operation affecting the same resource.

When the State Basis is stale, Reconciliation SHALL:

1. stop dispatching affected new work;
2. preserve existing Operation State;
3. determine whether in-flight work can safely finish;
4. obtain fresh observation where necessary;
5. invalidate or supersede the plan;
6. start a new planning cycle when appropriate.

---

## 24. Scheduling

Scheduling determines when executable Transitions may be dispatched.

Scheduling SHALL respect:

* dependency readiness;
* consistency boundaries;
* concurrency limits;
* controller limits;
* resource locks;
* retry windows;
* maintenance windows;
* priority;
* cancellation;
* State Basis validity.

Scheduling SHALL NOT alter the semantic content of a Transition.

A scheduler may choose among equally valid ready Transitions, but it SHALL NOT execute a Transition before its mandatory conditions are met.

---

## 25. Concurrency

Independent Transitions MAY execute concurrently.

Concurrency SHALL be prohibited when Transitions:

* mutate the same Managed Resource;
* affect the same exclusive external object;
* belong to a serial consistency group;
* depend on each other;
* use a controller that declares exclusive execution;
* violate policy limits;
* could make verification ambiguous;
* could invalidate each other’s State Basis.

Concurrency decisions SHALL be attributable.

---

## 26. Resource Coordination

Reconciliation SHALL prevent incompatible concurrent mutation of the same resource.

Coordination MAY use:

* leases;
* locks;
* operation ownership records;
* controller exclusivity;
* consistency-group scheduling;
* single-writer constraints.

The mechanism is implementation-specific.

The architectural requirement is that one resource SHALL NOT be subject to mutually incompatible active transitions.

A lost lock or lease SHALL make the execution outcome uncertain until verified.

---

## 27. Execution Dispatch

Reconciliation dispatches a Transition to the selected Resource Controller.

The dispatch SHALL include:

* transition identity;
* operation identity;
* Managed Resource identity;
* Resource Type;
* requested operation;
* validated parameters;
* State Basis;
* relevant preconditions;
* expected postconditions;
* cancellation context;
* execution deadline or timeout semantics;
* correlation information.

The controller SHALL return a structured Operation Result or progress information as defined by the Controller Framework.

Dispatch acknowledgment SHALL NOT mean that the external operation succeeded.

---

## 28. Operation State

Reconciliation SHALL record or cause the recording of Operation State for dispatched work.

Operation State MAY include:

* pending;
* dispatched;
* running;
* waiting;
* completed;
* failed;
* cancelled;
* interrupted;
* timed-out;
* outcome-unknown.

Operation State describes the execution process.

It SHALL NOT establish Current State.

A completed controller operation still requires postcondition verification when the Transition contract requires it.

---

## 29. Transition Preconditions

A Transition SHALL execute only when its mandatory preconditions are satisfied.

Preconditions MAY include:

* required Current State;
* dependency satisfaction;
* resource existence or absence;
* platform capability;
* sufficient storage;
* service readiness;
* integrity condition;
* maintenance window;
* exclusive ownership;
* fresh State Basis;
* absence of conflicting operations.

Preconditions requiring claims about external reality SHALL be established through acceptable Verification Results.

---

## 30. Postcondition Verification

After execution, Reconciliation SHALL request verification of the declared postconditions unless the Resource Type explicitly permits acceptance without further observation.

Postcondition verification SHALL:

* use the applicable verification purpose;
* identify the Transition and operation;
* use fresh evidence;
* bind the result to a valid State Basis;
* cover all mandatory postconditions;
* distinguish satisfied, unsatisfied, unknown, and error.

Controller completion SHALL NOT replace postcondition verification.

---

## 31. State Acceptance Coordination

When postconditions are verified, Reconciliation MAY request State Acceptance.

The request SHALL include:

* Managed Resource identity;
* proposed Current State representation;
* applicable Verification Result;
* State Basis;
* Desired Generation;
* previous Current Revision;
* operation correlation;
* acceptance boundary.

State Management determines whether the proposal may be committed.

Reconciliation SHALL NOT directly write accepted Current State.

After State Acceptance, a newer Current Revision may require remaining plan work to revalidate its State Basis.

---

## 32. Reconciliation Loop

The logical reconciliation loop is:

```text
Load Accepted State
        ↓
Validate State Basis
        ↓
Verify When Required
        ↓
Detect Differences
        ↓
Plan
        ↓
Validate Plan
        ↓
Execute Ready Transitions
        ↓
Verify Outcomes
        ↓
Request State Acceptance
        ↓
Re-evaluate
```

The loop continues until:

* the scoped resources are Converged;
* the scoped resources are Quiescent;
* the cycle is cancelled;
* a policy boundary stops further work;
* the State Basis is invalidated;
* unrecoverable architectural error prevents continuation.

---

## 33. Convergence

A Managed Resource is Converged when:

* every applicable mandatory Desired State condition is verified as satisfied; or
* an accepted policy explicitly exempts the condition.

A resource SHALL NOT be declared Converged solely because:

* no plan exists;
* an operation completed;
* retries were exhausted;
* the resource is blocked;
* the resource is unknown;
* the scheduler has no ready work;
* the controller reported success.

System convergence is evaluated over the declared reconciliation scope.

---

## 34. Quiescence

A scope is Quiescent when no Transition is immediately executable.

Quiescence may result from:

* convergence;
* dependency blockage;
* retry delay;
* unknown resource state;
* exhausted policy-approved attempts;
* maintenance-window closure;
* unavailable controller;
* operator intervention requirement;
* unresolved conflict;
* cancellation.

Every non-converged quiescent resource SHALL have an attributable reason.

Quiescence SHALL NOT be reported as convergence.

---

## 35. Drift Reconciliation

When fresh verification identifies Drift, Reconciliation SHALL evaluate the resource against the currently accepted Desired State.

Drift MAY result in:

* no action, when the new state remains satisfactory;
* State Acceptance of a newly observed but satisfactory representation;
* corrective planning;
* dependency re-evaluation;
* policy escalation;
* unknown status pending stronger evidence.

Drift does not automatically imply controller failure.

External actors may alter managed resources.

---

## 36. Idempotence

Reconciliation SHOULD select idempotent operations when equivalent alternatives exist.

Repeating a Reconciliation Cycle against semantically equivalent accepted state SHOULD produce no additional mutation after convergence.

When a controller operation is not inherently idempotent, the Transition contract SHALL define:

* safe retry conditions;
* observation requirements;
* operation identity semantics;
* duplicate-execution risks;
* compensation or escalation behavior.

---

## 37. Retry Classification

A failed or incomplete Transition SHALL be classified before retry.

A retry classification MAY be:

* immediately retryable;
* retryable after delay;
* retryable after dependency change;
* retryable after fresh verification;
* retryable only with a new plan;
* non-retryable;
* operator intervention required;
* outcome unknown.

Reconciliation SHALL NOT retry an Unknown Outcome until fresh verification determines whether retry is safe.

---

## 38. Retry Policy

Retry policy MAY define:

* maximum attempts;
* delay;
* exponential or fixed backoff;
* jitter;
* retry deadline;
* eligible error classes;
* dependency conditions;
* controller-specific limits;
* escalation behavior.

Retry policy SHALL NOT redefine truth.

Exhausting retries means that reconciliation will not attempt the same work under the current policy and basis.

It does not prove that the resource is unsatisfied.

---

## 39. Unknown Execution Outcomes

An execution outcome SHALL be classified as unknown when:

* the runtime is interrupted;
* controller communication is lost;
* a timeout occurs without reliable cancellation confirmation;
* a lock or lease is lost during mutation;
* the operation result is corrupted;
* the controller cannot establish whether an external side effect occurred;
* DAIA restarts during execution without a conclusive record.

For an Unknown Outcome, Reconciliation SHALL:

1. stop assuming the prior Current State remains accurate;
2. avoid immediate duplicate execution;
3. obtain fresh observation;
4. request recovery verification;
5. determine the actual resource condition;
6. replan from accepted and newly verified state.

Recovery begins from observation, not execution assumptions.

---

## 40. Cancellation

A Reconciliation Cycle or Transition MAY be cancelled.

Cancellation SHALL distinguish:

* cancellation requested;
* dispatch prevented;
* controller cancellation accepted;
* external operation stopped;
* cancellation unconfirmed;
* operation completed before cancellation;
* outcome unknown.

A cancellation request SHALL NOT be interpreted as proof that no mutation occurred.

After uncertain cancellation, fresh verification SHALL be required.

---

## 41. Compensation

Compensation MAY be used when:

* a transition partially succeeded;
* a consistency group could not complete;
* a policy requires containment;
* a later transition invalidates an earlier result;
* an executed change must be replaced by a safer target state.

Compensation SHALL be represented as a new planned Transition.

Compensation SHALL:

* have its own State Basis;
* use eligible controllers;
* declare preconditions and postconditions;
* be verified;
* produce its own Operation State;
* preserve the history of the original operation.

Compensation does not erase the original transition.

---

## 42. Rollback Semantics

Rollback SHALL be expressed through a new accepted Desired State or an authorized recovery target.

Reconciliation SHALL NOT rewrite history to make a previous state appear never to have existed.

A rollback plan is an ordinary forward plan toward an earlier or safer desired condition.

Rollback remains subject to:

* current external reality;
* compatibility constraints;
* controller eligibility;
* dependency conditions;
* verification;
* State Acceptance.

An earlier configuration may no longer be achievable.

---

## 43. Recovery

Recovery SHALL occur when reconciliation cannot trust the relationship between recorded execution and external reality.

Recovery SHALL:

* identify incomplete or uncertain operations;
* load the latest durable accepted state;
* avoid trusting transient in-memory state;
* obtain fresh observations;
* verify affected resources;
* validate current ownership and controller eligibility;
* invalidate stale plans;
* construct a new State Basis;
* resume through a new or revalidated plan.

Recovery SHALL NOT blindly replay the last operation.

---

## 44. Startup Recovery

After DAIA restarts, Reconciliation SHALL inspect Operation State for work that was:

* dispatched;
* running;
* waiting;
* cancelling;
* interrupted;
* timed out;
* outcome unknown.

For every affected resource, Reconciliation SHALL determine whether:

* the operation conclusively did not begin;
* the operation completed and requires verification;
* the operation failed conclusively;
* the outcome remains unknown.

Unknown cases require fresh observation before new mutation.

---

## 45. Plan Resumption

A suspended plan MAY resume only when:

* the plan remains valid;
* its State Basis remains valid;
* controller eligibility remains valid;
* ownership remains valid;
* dependencies remain satisfied;
* no conflicting operation occurred;
* applicable policy permits resumption.

Otherwise, the plan SHALL be invalidated and regenerated.

Completed and verified transitions need not be repeated when their accepted results remain valid.

---

## 46. Plan Invalidation

A plan SHALL be invalidated when a relevant assumption becomes false.

Invalidating events include:

* new Desired State affecting the scope;
* changed Current State;
* changed dependency state;
* changed policy;
* changed schema;
* changed controller capability;
* loss of ownership;
* external mutation;
* stale verification;
* consistency-group membership change.

Invalidation SHALL stop new affected dispatches.

In-flight transitions SHALL be handled according to their cancellation and unknown-outcome semantics.

---

## 47. Failure Handling

A Transition failure SHALL produce an attributable failure classification.

Failure classification SHOULD distinguish:

* invalid request;
* failed precondition;
* controller unavailable;
* controller rejected operation;
* external operation failed;
* verification unsatisfied;
* verification unknown;
* verification error;
* timeout;
* cancellation;
* policy denial;
* dependency failure;
* State Basis invalidation;
* internal reconciliation error.

Failure handling SHALL preserve the distinction between process failure and resource condition.

---

## 48. Failure Isolation

Failure of one resource SHOULD NOT prevent independent resources from reconciling.

Failure propagation SHALL follow declared dependency and consistency relationships.

A failed dependency MAY block its dependents.

It SHALL NOT automatically fail unrelated resources.

A consistency-group failure MAY affect all group members when partial completion is unsafe.

---

## 49. Blocked Resources

A resource is Blocked when required reconciliation work cannot currently proceed because a prerequisite is unmet.

A block record SHALL identify:

* blocked resource;
* blocking condition;
* blocking resource or policy;
* State Basis;
* whether automatic re-evaluation is possible;
* whether operator intervention is required.

Blocked SHALL remain distinct from failed, unknown, and converged.

---

## 50. Unavailable Controllers

When no eligible controller is available for an actionable Difference:

* the resource SHALL be classified as non-actionable or blocked;
* Reconciliation SHALL NOT fabricate an operation;
* the missing capability SHALL be reported;
* the scope MAY become Quiescent;
* other independent work MAY continue.

Controller unavailability does not alter Desired State.

---

## 51. Policy Interaction

Policy MAY constrain:

* controller selection;
* allowed transitions;
* destructive operations;
* retry limits;
* maintenance windows;
* concurrency;
* compensation;
* rollback;
* operator approval;
* automatic recovery;
* degraded satisfaction;
* exemptions.

Every policy-dependent decision SHALL identify the applicable policy revision in the State Basis.

Policy SHALL NOT directly claim external truth.

---

## 52. Destructive Transitions

A destructive Transition is one that may remove data, disable capability, destroy a resource, or create difficult-to-reverse effects.

Destructive Transitions SHALL:

* be explicitly identified;
* satisfy applicable authorization policy;
* declare expected impact;
* validate a fresh State Basis;
* define postconditions;
* define compensation limitations;
* use strengthened verification where required.

Absence from Desired State SHALL NOT automatically authorize deletion unless the Resource Type ownership and policy semantics permit it.

---

## 53. Ownership

Reconciliation SHALL mutate only resources within DAIA’s accepted authority.

Ownership information SHALL determine whether DAIA may:

* create;
* modify;
* replace;
* disable;
* remove;
* adopt;
* ignore;

a resource.

Externally owned resources MAY be observed and used as dependencies without being mutable by DAIA.

Ambiguous ownership SHALL block destructive work.

---

## 54. Adoption

Adoption is the process by which an existing external resource becomes managed by DAIA.

Adoption SHALL require:

* verified resource identity;
* compatibility with the Resource Type;
* explicit ownership authorization;
* accepted Desired State;
* initial State Acceptance;
* applicable policy approval.

Reconciliation SHALL NOT silently adopt an existing resource merely because it resembles Desired State.

---

## 55. Orphan Handling

An Orphan is a resource previously associated with DAIA that no longer has a clear applicable Desired State or ownership relationship.

Orphans SHALL be classified before action.

Possible outcomes include:

* retain;
* ignore;
* detach from management;
* adopt into a new specification;
* remove through an authorized destructive Transition.

Automatic removal SHALL require explicit Resource Type and policy support.

---

## 56. Priority

Reconciliation MAY assign priority based on:

* dependency criticality;
* security impact;
* service availability;
* operator request;
* recovery urgency;
* resource class;
* maintenance window;
* policy.

Priority affects scheduling.

Priority SHALL NOT bypass mandatory preconditions, verification, ownership, or State Basis validation.

---

## 57. Fairness

A scheduler SHOULD prevent one repeatedly failing or high-volume resource from indefinitely starving unrelated ready work.

Fairness mechanisms are implementation-specific.

They SHALL preserve dependency and consistency requirements.

---

## 58. Rate Limiting

Reconciliation MAY limit:

* controller dispatch rate;
* external API usage;
* package operations;
* network transfers;
* storage-intensive operations;
* verification requests;
* retries.

Rate limiting SHALL affect scheduling, not architectural correctness.

A rate-limited resource may be Quiescent temporarily without being Converged.

---

## 59. Offline Operation

DAIA is offline-first.

Reconciliation SHALL distinguish:

* a required resource unavailable because the system is offline;
* a resource available from approved local sources;
* a resource whose Desired State explicitly requires remote access;
* a resource blocked pending approved media or connectivity.

Offline conditions SHALL be represented through evidence, policy, and blocking conditions.

They SHALL NOT be treated as generic controller failure.

---

## 60. Reconciliation Events

Reconciliation SHOULD emit structured events for:

* cycle started;
* cycle completed;
* plan created;
* plan invalidated;
* transition ready;
* transition dispatched;
* transition completed;
* verification requested;
* verification received;
* State Acceptance requested;
* resource converged;
* resource blocked;
* resource failed;
* resource unknown;
* scope quiescent;
* recovery started;
* recovery completed.

Events are operational records.

They do not establish Current State.

---

## 61. Auditability

Reconciliation decisions SHALL be auditable.

The audit trail SHALL make it possible to determine:

* which trigger started the cycle;
* which State Basis was used;
* which Differences were detected;
* why a Transition was selected;
* why a controller was selected;
* which dependencies applied;
* which actions were dispatched;
* what operation results were returned;
* which Verification Results were used;
* whether State Acceptance occurred;
* why the cycle stopped.

Sensitive operation parameters SHOULD be redacted or referenced securely.

---

## 62. Observability

A Reconciliation implementation SHOULD expose information sufficient to diagnose:

* cycle duration;
* planning duration;
* ready and blocked work;
* plan invalidations;
* dispatch latency;
* controller failures;
* verification delays;
* retry activity;
* unknown outcomes;
* convergence rate;
* quiescent non-converged resources;
* recovery activity.

Operational metrics SHALL NOT replace Verification Results or accepted state.

---

## 63. Extensibility

New Resource Types MAY introduce:

* Difference semantics;
* Transition types;
* dependency requirements;
* consistency groups;
* preconditions;
* postconditions;
* retry classifications;
* compensation capabilities.

Extensions SHALL preserve:

* immutable plans;
* explicit State Basis;
* controller eligibility;
* verification before State Acceptance;
* separation of execution and truth;
* distinction between convergence and quiescence;
* recovery from observation.

Resource-specific extensions SHALL NOT directly write Current State.

---

## 64. Reconciliation Lifecycle

The complete logical lifecycle is:

```text
Trigger
   ↓
Load Accepted State
   ↓
Construct State Basis
   ↓
Validate Scope
   ↓
Refresh Verification Where Required
   ↓
Detect Differences
   ↓
Classify Actionability
   ↓
Construct Reconciliation Plan
   ↓
Validate Plan
   ↓
Schedule Ready Transitions
   ↓
Dispatch Controller Operations
   ↓
Record Operation State
   ↓
Observe External Results
   ↓
Verify Postconditions
   ↓
Request State Acceptance
   ↓
Re-evaluate Scope
   ↓
Converged, Quiescent, Invalidated, or Cancelled
```

This lifecycle is conceptual.

It does not prescribe threads, processes, queues, databases, or programming languages.

---

## 65. Conformance Requirements

A conforming Reconciliation implementation SHALL:

1. operate from accepted Desired State and Current State;
2. use an explicit State Basis;
3. detect semantic Differences;
4. plan before execution;
5. keep plans immutable;
6. validate plans before dispatch;
7. use only eligible controllers;
8. respect dependencies and consistency groups;
9. validate mandatory preconditions;
10. record Operation State;
11. require postcondition verification where declared;
12. avoid direct Current State mutation;
13. distinguish unknown outcomes from failures;
14. avoid blind retries after unknown outcomes;
15. recover through fresh observation;
16. distinguish convergence from quiescence;
17. preserve auditability;
18. invalidate stale plans;
19. isolate unrelated failures;
20. produce deterministic plans under equivalent inputs.

---

## 66. Conformance Tests

A Reconciliation implementation SHALL be testable for at least:

* no-op convergence;
* missing resource planning;
* misconfiguration planning;
* dependency ordering;
* dependency cycle rejection;
* consistency-group behavior;
* concurrent independent transitions;
* conflicting transition exclusion;
* stale State Basis rejection;
* plan invalidation;
* controller unavailability;
* precondition failure;
* postcondition unsatisfied;
* verification unknown;
* verification error;
* immediate retry;
* delayed retry;
* exhausted retries;
* unknown execution outcome;
* cancellation uncertainty;
* startup recovery;
* compensation;
* rollback through new Desired State;
* blocked resources;
* quiescent non-converged scope;
* deterministic planning;
* failure isolation;
* prevention of direct Current State mutation.

---

## 67. Architectural Invariants

The following invariants are mandatory:

1. Desired State is authoritative for intent.
2. Current State is accepted only through State Management.
3. Planning does not execute.
4. Execution does not establish truth.
5. Verification precedes State Acceptance.
6. Every plan is bound to a State Basis.
7. A stale plan cannot authorize new work.
8. Controller success is not convergence.
9. Unknown outcome is not failure.
10. Unknown outcome is not success.
11. Recovery begins from observation.
12. Retry follows classification.
13. Unknown outcomes are not blindly retried.
14. Rollback is a new forward reconciliation.
15. Compensation does not erase history.
16. Blocked is not failed.
17. Quiescent is not necessarily converged.
18. Destructive work requires explicit authority.
19. Unrelated failures are isolated.
20. Reconciliation does not commit Current State.

---

## 68. Minimal Reconciliation Example

Desired State:

```yaml
resource:
  id: service/ollama
  type: service
  desired:
    present: true
    enabled: true
    running: true
```

Accepted Current State:

```yaml
resource:
  id: service/ollama
  current:
    present: true
    enabled: false
    running: false
```

Detected Differences:

```yaml
differences:
  - condition: enabled
    current: false
    desired: true
  - condition: running
    current: false
    desired: true
```

Plan:

```yaml
plan:
  state_basis:
    desired_generation: 12
    current_revision: 41
  transitions:
    - id: enable-ollama
      operation: service.enable
    - id: start-ollama
      operation: service.start
      depends_on:
        - enable-ollama
```

Execution:

```text
Enable service
      ↓
Verify enabled
      ↓
Accept new Current State
      ↓
Start service
      ↓
Verify running
      ↓
Accept new Current State
```

The resource is Converged only after the required conditions are verified and accepted.

---

## 69. Unknown Outcome Example

A controller begins installing a package.

DAIA restarts before the controller reports completion.

The Operation State is:

```yaml
operation:
  status: interrupted
  outcome: unknown
```

Correct recovery:

```text
Load durable accepted state
        ↓
Identify uncertain package operation
        ↓
Observe package database
        ↓
Verify installed package condition
        ↓
Accept verified Current State
        ↓
Replan remaining Difference
```

Incorrect recovery:

```text
Restart DAIA
        ↓
Assume installation failed
        ↓
Immediately reinstall package
```

The incorrect sequence risks duplicate or conflicting external work.

---

## 70. Quiescence Example

A model resource requires an offline model archive.

The archive is not present on any approved local resource drive.

The resource is:

```yaml
status:
  converged: false
  quiescent: true
  reason: required-offline-artifact-unavailable
  intervention: attach-approved-resource-media
```

The system has no immediately executable work.

It is Quiescent, not Converged.

---

## 71. Architectural Consequences

This architecture produces several consequences:

* DAIA can continually manage resources rather than merely install them once.
* Restart recovery does not depend on trusting incomplete execution records.
* Resource Controllers remain replaceable because orchestration owns planning.
* Verification remains independent from mutation.
* State remains authoritative because reconciliation cannot commit it directly.
* External nondeterminism is contained through observation and replanning.
* Partial failures do not require restarting the entire system.
* New Resource Types can extend transition semantics without changing the reconciliation core.
* The system can report blocked and unknown conditions honestly instead of presenting false completion.

---

## 72. Final Statement

DAIA Reconciliation is the coordination mechanism that moves managed reality toward accepted intent.

It does not own intent.

It does not own truth.

It does not implement resource-specific mutation.

Its responsibility is to repeatedly and safely answer:

> What valid, verified, and currently authorized work should happen next?

Reconciliation continues until the applicable resources are Converged or no immediately executable work remains.
