# DAIA Verification Architecture

**Document:** `11-verification.md`
**Architecture Version:** 1.0
**Status:** Draft
**Depends On:** `09-state-architecture.md`, `10-controller-framework.md`
**Used By:** `12-reconciliation.md`

---

## 1. Purpose

This document defines the DAIA Verification Architecture.

Verification determines whether evidence about a Managed Resource is sufficient to support an architectural conclusion.

Verification answers:

> What does the available evidence prove about the Managed Resource?

Verification does not express intent, plan transitions, mutate external resources, or commit Current State.

Its purpose is to evaluate evidence and produce a deterministic Verification Result that State Management may use during State Acceptance.

---

## 2. Scope

This document defines:

* the verification authority;
* Verification Requests;
* evidence requirements;
* evidence sources;
* evidence freshness;
* verification rules;
* Verification Results;
* satisfaction evaluation;
* integrity evaluation;
* health evaluation;
* disagreement between evidence sources;
* inconclusive verification;
* verification errors;
* verification extensibility;
* verification conformance requirements.

This document does not define:

* Desired State semantics;
* Current State persistence;
* State Acceptance;
* transition planning;
* reconciliation scheduling;
* controller execution;
* controller registration;
* general monitoring or alerting;
* user-interface presentation.

Those responsibilities belong to their respective architectural components.

---

## 3. Normative Language

The terms **SHALL**, **SHALL NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative.

* **SHALL** indicates a mandatory architectural requirement.
* **SHALL NOT** indicates a prohibited behavior.
* **SHOULD** indicates a recommended behavior that may be departed from only for a documented reason.
* **SHOULD NOT** indicates a discouraged behavior that may be used only for a documented reason.
* **MAY** indicates an optional capability.

---

## 4. Verification Principle

DAIA SHALL distinguish between:

1. performing an operation;
2. observing its apparent outcome;
3. verifying what the evidence proves;
4. accepting verified Current State.

Execution success does not prove resource correctness.

Command success does not prove Desired State satisfaction.

Controller reports do not become Current State merely because the controller produced them.

Only evidence accepted through Verification may authorize State Acceptance.

---

## 5. Architectural Position

```text
Desired State
      │
      │ expected condition
      ▼
Verification Request
      │
      ├── State Basis
      ├── Resource identity
      ├── Desired specification
      ├── Available evidence
      └── Verification policy
      │
      ▼
Verification
      │
      ▼
Verification Result
      │
      ├── satisfied
      ├── unsatisfied
      ├── unknown
      ├── not-applicable
      └── error
      │
      ▼
State Acceptance
```

Verification produces a decision.

State Management decides whether that decision may be committed within a valid State Acceptance boundary.

---

## 6. Verification Authority

Verification is the sole architectural authority permitted to declare whether evidence is acceptable for State Acceptance.

Verification SHALL:

* evaluate evidence against declared requirements;
* identify the State Basis used;
* distinguish absence of evidence from evidence of absence;
* distinguish an unsatisfied condition from an unknown condition;
* identify which conditions were evaluated;
* preserve the reasons for its conclusion;
* produce a structured Verification Result.

Verification SHALL NOT:

* modify Desired State;
* mutate external resources;
* select corrective transitions;
* schedule reconciliation;
* commit Current State;
* treat execution success as verification;
* infer facts not supported by acceptable evidence.

State Management is the sole authority permitted to commit accepted Current State.

---

## 7. Core Terms

### 7.1 Observation

An Observation is a structured representation of evidence collected from or about a Managed Resource.

An Observation describes what a source reports or measures.

An Observation is not automatically true, complete, current, or sufficient.

### 7.2 Evidence

Evidence is information submitted for evaluation by Verification.

Evidence may include:

* direct resource observations;
* operating-system state;
* package metadata;
* service state;
* process state;
* file metadata;
* file content;
* cryptographic digests;
* API responses;
* container runtime state;
* model metadata;
* dependency state;
* controller operation results;
* authoritative external records.

### 7.3 Verification Request

A Verification Request asks Verification to evaluate one or more declared conditions against supplied or obtainable evidence.

### 7.4 Verification Rule

A Verification Rule defines how a condition is evaluated.

### 7.5 Verification Result

A Verification Result is the immutable conclusion produced by Verification for a particular request and State Basis.

### 7.6 Satisfaction

Satisfaction means that verified evidence proves the applicable Desired State requirements for the resource.

### 7.7 Integrity

Integrity means that verified evidence proves that the relevant resource representation has not been altered beyond permitted expectations.

### 7.8 Health

Health describes an operational condition reported by defined health criteria.

Health and satisfaction are distinct.

A resource may be:

* satisfied but unhealthy;
* healthy but unsatisfied;
* both satisfied and healthy;
* neither satisfied nor healthy.

### 7.9 Unknown

Unknown means that available evidence is insufficient, stale, contradictory, inaccessible, or otherwise incapable of proving satisfaction or non-satisfaction.

Unknown SHALL NOT be treated as success or failure.

---

## 8. Verification Request

Every Verification Request SHALL identify:

* the Managed Resource;
* the Resource Type;
* the resource schema version;
* the verification purpose;
* the State Basis;
* the applicable Desired Generation, when satisfaction is evaluated;
* the expected conditions;
* the available evidence or authorized evidence sources;
* the verification policy or policy revision;
* the request timestamp;
* the requesting architectural component.

A Verification Request MAY cover:

* one condition;
* multiple conditions of one resource;
* a declared consistency group;
* a resource relationship;
* dependency readiness;
* transition preconditions;
* transition postconditions.

A request spanning multiple resources SHALL identify the consistency boundary being evaluated.

---

## 9. Verification Purposes

Verification MAY be requested for different purposes.

Supported architectural purposes include:

### 9.1 Current-State Establishment

Determines which claims may be represented in Current State.

### 9.2 Desired-State Satisfaction

Determines whether the Managed Resource satisfies its accepted Desired State.

### 9.3 Transition Preconditions

Determines whether a proposed transition may safely begin.

### 9.4 Transition Postconditions

Determines whether an executed transition produced the required outcome.

### 9.5 Recovery Verification

Determines the state of a resource after interruption or an unknown execution outcome.

### 9.6 Drift Verification

Determines whether previously accepted Current State remains consistent with newly observed reality.

### 9.7 Integrity Verification

Determines whether a resource matches an expected identity, digest, signature, schema, ownership, or protected representation.

### 9.8 Health Verification

Determines whether defined operational health conditions are met.

The purpose SHALL be explicit because evidence sufficient for one purpose may be insufficient for another.

---

## 10. Evidence Sources

Evidence MAY originate from:

* a dedicated Observer;
* a Resource Controller;
* the operating system;
* a local service;
* a remote service;
* the DAIA Persistence component;
* a trusted package or image repository;
* a cryptographic verification mechanism;
* a hardware interface;
* an administrator-approved external authority.

Every evidence item SHALL identify its source.

A source declaration SHOULD include:

* source identity;
* source type;
* collection time;
* collection method;
* applicable resource identity;
* schema version;
* trust classification;
* freshness limit;
* completeness information;
* collection errors.

Evidence from a Resource Controller SHALL be treated as evidence, not as authoritative Current State.

---

## 11. Direct and Indirect Evidence

### 11.1 Direct Evidence

Direct Evidence is collected from the Managed Resource or from an authority that directly represents its state.

Examples include:

* querying a service manager for service state;
* reading a package database;
* calculating a file digest;
* querying a container runtime;
* inspecting a model manifest.

### 11.2 Indirect Evidence

Indirect Evidence supports a conclusion without directly observing the complete resource condition.

Examples include:

* an operation returned exit code zero;
* a controller reported completion;
* a dependency claims that it created an object;
* a cached inventory lists the resource.

Indirect Evidence MAY support verification but SHALL NOT be treated as sufficient unless the applicable Verification Rule explicitly permits it.

Where reliable Direct Evidence is available, Verification SHOULD prefer Direct Evidence.

---

## 12. Evidence Requirements

Evidence used for verification SHALL be:

* attributable;
* relevant;
* interpretable under a known schema;
* applicable to the identified Managed Resource;
* collected within an acceptable time boundary;
* sufficiently complete for the evaluated condition;
* obtained through an authorized source;
* free from unresolved integrity errors.

Verification SHALL reject or classify as unknown any conclusion that depends on evidence whose identity, origin, interpretation, or applicability cannot be established.

---

## 13. Evidence Freshness

Every Verification Rule that depends on time-sensitive evidence SHALL define a freshness requirement.

Evidence freshness SHALL be evaluated relative to:

* the collection timestamp;
* the verification timestamp;
* the resource’s expected rate of change;
* the verification purpose;
* intervening operations;
* intervening Desired Generation changes;
* relevant dependency changes.

Evidence SHALL be considered stale when it no longer provides a reliable basis for the requested conclusion.

Stale evidence SHALL NOT establish Current State unless the applicable architecture explicitly permits a bounded stale conclusion.

When stale evidence remains useful for diagnostic purposes, it MAY be preserved but SHALL be identified as stale.

---

## 14. Evidence Completeness

Verification SHALL determine whether the available evidence covers all conditions required by the request.

Partial evidence MAY produce partial condition results.

Partial evidence SHALL NOT produce an overall `satisfied` result when unevaluated mandatory conditions remain.

When required evidence cannot be obtained, the applicable condition SHALL normally be `unknown`.

An absence of a resource may be verified only when the observation method is capable of reliably proving absence within the declared scope.

---

## 15. Evidence Integrity

Evidence integrity SHALL be evaluated when corruption, tampering, truncation, schema mismatch, or source substitution could materially affect the conclusion.

Integrity mechanisms MAY include:

* cryptographic hashes;
* digital signatures;
* authenticated transport;
* trusted local interfaces;
* schema validation;
* source identity validation;
* immutable audit records;
* sequence or revision checks.

Evidence with unresolved integrity failure SHALL NOT authorize State Acceptance.

---

## 16. Evidence Normalization

Evidence from different sources MAY use different representations.

Verification MAY normalize evidence into a canonical verification representation before evaluation.

Normalization SHALL:

* preserve the semantic meaning of the evidence;
* record the source representation;
* record the normalization rule or version;
* avoid inventing unavailable values;
* distinguish missing, empty, false, and unknown values;
* preserve precision relevant to the Verification Rule.

Normalization SHALL NOT convert uncertainty into certainty.

---

## 17. Verification Rules

A Verification Rule SHALL define:

* the condition being evaluated;
* the applicable Resource Type or schema;
* required evidence;
* acceptable evidence sources;
* freshness requirements;
* normalization requirements;
* comparison semantics;
* result mapping;
* error handling;
* rule version.

Verification Rules SHALL be deterministic under semantically equivalent inputs.

A rule SHALL NOT perform external resource mutation.

A rule MAY request additional evidence through an authorized observation interface, but observation and evaluation SHALL remain conceptually distinct.

---

## 18. Comparison Semantics

Verification SHALL use semantic comparison appropriate to the Resource Type.

Literal equality SHALL NOT be assumed unless literal equality represents the intended condition.

Comparison MAY include:

* exact equality;
* normalized equality;
* set equality;
* subset or superset requirements;
* ordered sequence comparison;
* range comparison;
* version constraints;
* structural equivalence;
* cryptographic identity;
* endpoint reachability;
* policy evaluation;
* dependency satisfaction;
* tolerance-based comparison.

The comparison semantics SHALL be declared by the applicable resource schema or Verification Rule.

---

## 19. Condition Results

Each evaluated condition SHALL produce one of the following results:

### 19.1 Satisfied

Acceptable evidence proves that the condition is met.

### 19.2 Unsatisfied

Acceptable evidence proves that the condition is not met.

### 19.3 Unknown

The condition cannot be proven satisfied or unsatisfied.

### 19.4 Not Applicable

The condition does not apply under the evaluated resource specification or environment.

### 19.5 Error

Verification could not correctly evaluate the condition because the verification mechanism itself failed.

`error` describes failure of verification processing.

`unknown` describes insufficient knowledge about the resource.

These states SHALL remain distinct.

---

## 20. Overall Verification Result

A Verification Result MAY include multiple Condition Results.

Unless a Resource Type defines stricter aggregation semantics:

* the overall result SHALL be `error` when a mandatory evaluation mechanism fails;
* otherwise, it SHALL be `unknown` when any applicable mandatory condition is unknown;
* otherwise, it SHALL be `unsatisfied` when any applicable mandatory condition is unsatisfied;
* otherwise, it SHALL be `satisfied` when all applicable mandatory conditions are satisfied;
* `not-applicable` conditions SHALL not prevent satisfaction.

A Resource Type MAY define additional aggregation behavior, but it SHALL NOT convert unknown mandatory conditions into satisfied conditions.

---

## 21. Verification Result Structure

Every Verification Result SHALL identify:

* result identity;
* Managed Resource identity;
* Resource Type;
* verification purpose;
* State Basis;
* Desired Generation, when applicable;
* verification policy revision;
* Verification Rule versions;
* evidence identities;
* evidence collection times;
* verification time;
* individual Condition Results;
* overall result;
* explanatory reasons;
* warnings;
* verifier identity or implementation identity;
* result schema version.

A Verification Result SHOULD contain enough information to reproduce or audit the architectural decision.

A Verification Result SHALL be immutable after publication.

Corrections SHALL produce a new result rather than modifying an existing result.

---

## 22. Verification and State Basis

Every Verification Result SHALL be bound to the State Basis under which it was produced.

The result SHALL become stale when a relevant part of that basis changes.

Relevant changes MAY include:

* a new Desired Generation;
* a changed resource schema;
* a changed dependency revision;
* a changed verification policy;
* a changed controller capability;
* a new operation affecting the resource;
* a newer accepted Current Revision;
* evidence collected after the original result that contradicts it.

State Management SHALL validate the State Basis before accepting a Verification Result.

Verification SHALL NOT claim that a result remains valid beyond its declared basis.

---

## 23. Satisfaction Verification

Satisfaction Verification determines whether a Managed Resource satisfies its accepted Desired State.

It SHALL evaluate only declared Desired State requirements.

It SHALL NOT require undeclared implementation details unless those details are architectural invariants of the Resource Type.

For example, a service resource may require:

* the service to exist;
* the service to be enabled;
* the service to be running.

It should not require a particular internal command sequence unless the Desired State or Resource Type contract specifies that requirement.

Satisfaction SHALL be based on the resulting condition, not on whether a preferred operation was executed.

---

## 24. Integrity Verification

Integrity Verification determines whether the resource matches protected expectations.

Integrity requirements MAY include:

* expected digest;
* expected signature;
* expected owner;
* expected permissions;
* expected source;
* expected immutable fields;
* expected package identity;
* expected image identity;
* expected configuration structure.

Integrity failure SHALL be represented independently from operational health where both are evaluated.

A healthy resource with failed integrity SHALL NOT be considered satisfied when integrity is part of Desired State.

---

## 25. Health Verification

Health Verification evaluates declared operational health conditions.

Health criteria SHALL be Resource Type-specific.

Health MAY include:

* process responsiveness;
* service readiness;
* endpoint reachability;
* dependency connectivity;
* resource capacity;
* recent error state;
* self-test result.

Health Verification SHALL NOT silently become a general monitoring system.

Continuous health monitoring MAY exist elsewhere, but when its evidence is used for State Acceptance it SHALL comply with this Verification Architecture.

---

## 26. Dependency Verification

A resource MAY depend on verified claims about other Managed Resources.

Dependency Verification SHALL identify:

* the dependency resource;
* the required dependency condition;
* the dependency Current Revision or Verification Result;
* the freshness requirement;
* the consistency boundary.

A dependent resource SHALL NOT be declared satisfied when a mandatory dependency condition is unknown or unsatisfied, unless the resource contract explicitly permits degraded satisfaction.

---

## 27. Conflicting Evidence

Evidence conflicts when acceptable sources provide mutually incompatible claims about the same relevant condition.

Verification SHALL NOT resolve conflicts by arbitrary source order.

The applicable Verification Rule SHALL define one of the following:

* authoritative-source precedence;
* quorum or agreement requirements;
* recency precedence;
* direct-evidence precedence;
* conflict as unknown;
* conflict as verification error.

Unresolved conflict SHALL produce `unknown` or `error`, depending on whether the uncertainty concerns resource state or verification processing.

The conflict SHALL be recorded in the Verification Result.

---

## 28. Missing Evidence

Missing evidence SHALL NOT automatically imply that a resource is absent, failed, or unsatisfied.

The result depends on what the observation mechanism is capable of proving.

Examples:

* failure to query a service manager does not prove the service is stopped;
* absence from a complete package inventory may prove a package is absent;
* absence from an incomplete cached list proves nothing;
* a missing file may be verified when the filesystem scope was successfully inspected.

Verification Rules SHALL explicitly define absence semantics.

---

## 29. Unknown Execution Outcomes

When execution is interrupted or its completion cannot be established, the outcome SHALL be treated as unknown.

Verification SHALL obtain fresh evidence before the system:

* retries the operation;
* compensates for the operation;
* accepts Current State;
* declares transition failure;
* declares transition success.

Operation State MAY report that execution was attempted.

Operation State SHALL NOT substitute for verification of external reality.

---

## 30. Verification Errors

Verification errors include:

* invalid requests;
* unsupported Resource Types;
* unavailable Verification Rules;
* schema incompatibility;
* evidence parsing failure;
* internal verifier failure;
* unauthorized evidence access;
* normalization failure;
* rule execution failure.

A verification error SHALL NOT be converted into an unsatisfied resource result unless the Resource Type contract explicitly defines that interpretation.

Verification errors SHALL be attributable and auditable.

Retry policy for verification processing belongs to Reconciliation, not to Verification.

---

## 31. Verification Providers

A Verification Provider is an implementation that supplies one or more Verification Rules or evidence-evaluation capabilities.

A provider SHALL declare:

* provider identity;
* provider version;
* supported Resource Types;
* supported schema versions;
* supported verification purposes;
* required evidence;
* output schema;
* deterministic comparison behavior;
* error semantics.

Provider discovery and registration belong to the Controller Framework or Registry architecture.

This document defines provider behavior after selection.

---

## 32. Provider Selection

The architectural component responsible for provider selection SHALL select only providers compatible with:

* the Resource Type;
* the resource schema version;
* the verification purpose;
* available evidence;
* required policy;
* the State Basis.

Provider selection SHALL be completed before verification evaluation begins.

Verification SHALL record the selected provider in the Verification Result.

Verification SHALL NOT silently change providers during one evaluation.

---

## 33. Controller-Supplied Verification

A Resource Controller MAY provide:

* observations;
* operation receipts;
* Resource Type-specific evidence;
* verification helper capabilities.

A controller SHALL NOT be considered authoritative merely because it performed the transition.

Where practical, postcondition verification SHOULD use an observation path independent of the mutation path.

Where independent verification is not possible, the Verification Result SHALL identify that the evidence originated from the executing controller.

---

## 34. Verification Caching

Verification Results MAY be cached.

A cached result MAY be reused only when:

* its State Basis remains valid;
* its evidence remains fresh;
* no relevant operation has occurred;
* no relevant dependency has changed;
* the verification policy permits reuse;
* the verification purpose is compatible.

Cache reuse SHALL be visible in the resulting audit information.

Caching SHALL NOT weaken freshness or State Basis requirements.

---

## 35. Verification Groups

Some conditions cannot be meaningfully verified in isolation.

A Verification Group MAY define a consistency boundary across multiple conditions or resources.

Examples include:

* a service and its configuration;
* a container and its persistent storage;
* an AI engine and its selected model;
* an interface and its configured engine endpoint.

A group result SHALL identify every included resource and revision.

Partial group verification SHALL NOT be represented as complete group satisfaction.

---

## 36. Verification Lifecycle

The logical Verification lifecycle is:

```text
Receive Request
      ↓
Validate Request
      ↓
Resolve Applicable Rules
      ↓
Validate State Basis
      ↓
Collect or Receive Evidence
      ↓
Validate Evidence
      ↓
Normalize Evidence
      ↓
Evaluate Conditions
      ↓
Aggregate Results
      ↓
Publish Immutable Verification Result
```

This lifecycle defines responsibility boundaries.

It does not prescribe implementation technology or process structure.

---

## 37. Relationship to Reconciliation

Reconciliation uses Verification to determine:

* whether work is required;
* whether transition preconditions hold;
* whether execution succeeded in reality;
* whether drift exists;
* whether recovery may proceed;
* whether convergence has been reached.

Reconciliation SHALL NOT override a Verification Result.

Reconciliation MAY request new verification when:

* evidence is stale;
* the State Basis changed;
* a result is unknown;
* an operation occurred;
* recovery requires re-observation;
* policy requires stronger evidence.

Retry, scheduling, backoff, and escalation belong to `12-reconciliation.md`.

---

## 38. Relationship to State Management

Verification does not commit state.

The boundary is:

```text
Verification
    produces an immutable Verification Result

State Management
    validates the State Basis and acceptance conditions

State Acceptance
    commits accepted Current State and Historical State
```

State Management SHALL reject a Verification Result when:

* its State Basis is stale;
* required evidence is missing;
* evidence integrity is unresolved;
* the result schema is unsupported;
* the provider is unauthorized;
* the result does not cover the acceptance boundary;
* acceptance invariants are not satisfied.

---

## 39. Relationship to Controllers

Controllers mutate or observe Managed Resources.

Verification judges evidence about Managed Resources.

Controllers SHALL NOT:

* commit Current State;
* mark their own operations as verified without producing a Verification Result;
* redefine verification policy;
* suppress contradictory evidence;
* convert unknown outcomes into successful outcomes.

Controllers MAY implement Resource Type-specific observation interfaces defined by the Controller Framework.

---

## 40. Security Boundaries

Verification SHALL evaluate only evidence that the requesting context is authorized to access.

Sensitive evidence SHALL be minimized.

Verification Results SHOULD avoid storing secrets or unnecessary confidential values.

Where sensitive values must be compared, Verification SHOULD store:

* a digest;
* a redacted representation;
* a boolean comparison result;
* a protected reference;

rather than the secret itself.

Evidence authorization and secret storage mechanisms belong to the applicable security architecture.

---

## 41. Auditability

Every Verification Result SHALL support architectural audit.

The audit record SHALL make it possible to determine:

* what was verified;
* why it was verified;
* which Desired Generation applied;
* which State Basis applied;
* which evidence was used;
* when the evidence was collected;
* which rules were applied;
* which provider evaluated the evidence;
* what conclusion was reached;
* why the conclusion was reached.

Auditability SHALL NOT require preserving unrestricted sensitive evidence.

---

## 42. Observability

Verification implementations SHOULD expose operational information sufficient to diagnose:

* request volume;
* verification duration;
* evidence collection failures;
* unknown results;
* conflicting evidence;
* stale evidence;
* provider errors;
* rule incompatibilities.

Operational observability SHALL NOT alter verification semantics.

Metrics and logs SHALL NOT become evidence unless they are explicitly submitted and evaluated under a Verification Rule.

---

## 43. Extensibility

New Resource Types MAY introduce new Verification Rules and providers.

Extensions SHALL preserve:

* the standard Verification Request;
* the standard Condition Result meanings;
* the standard Verification Result envelope;
* State Basis binding;
* evidence attribution;
* auditability;
* the separation between verification and State Acceptance.

An extension SHALL NOT redefine `satisfied`, `unsatisfied`, `unknown`, `not-applicable`, or `error` incompatibly.

Resource-specific details belong within the extension.

Core verification semantics remain stable.

---

## 44. Determinism

Verification SHALL be deterministic under semantically equivalent:

* requests;
* State Bases;
* evidence sets;
* rule versions;
* policy revisions.

Differences caused by time-dependent freshness SHALL be attributable to different verification times or evidence validity windows.

External observation may be nondeterministic.

The evaluation of a fixed accepted evidence set SHALL be deterministic.

---

## 45. Idempotence

Repeating verification with the same semantically equivalent inputs SHALL produce an equivalent conclusion.

Equivalent results need not have identical:

* result identities;
* creation timestamps;
* implementation metadata.

They SHALL have equivalent condition conclusions and reasoning under the same rule and policy versions.

---

## 46. Failure Isolation

Failure to verify one Managed Resource SHOULD NOT prevent unrelated resources from being verified.

A group verification failure SHALL be contained within its declared consistency boundary.

Shared provider failure MAY affect multiple resources, but each affected request SHALL receive an attributable result or error.

---

## 47. Conformance Requirements

A conforming Verification implementation SHALL:

1. accept structured Verification Requests;
2. identify the State Basis;
3. validate resource and schema compatibility;
4. evaluate only declared conditions;
5. preserve evidence attribution;
6. enforce evidence freshness requirements;
7. distinguish direct and indirect evidence;
8. distinguish unknown from unsatisfied;
9. distinguish verification error from resource state;
10. produce immutable structured Verification Results;
11. record applicable rule and provider versions;
12. avoid external mutation;
13. avoid committing Current State;
14. support auditability;
15. behave deterministically under equivalent inputs.

---

## 48. Conformance Tests

A Verification implementation SHALL be testable for at least:

* satisfied conditions;
* unsatisfied conditions;
* missing evidence;
* stale evidence;
* contradictory evidence;
* invalid evidence schema;
* evidence integrity failure;
* unsupported Resource Type;
* unsupported schema version;
* unknown execution outcome;
* provider failure;
* partial condition coverage;
* dependency uncertainty;
* stale State Basis;
* equivalent-input determinism;
* immutable result behavior;
* protection against direct Current State mutation.

Resource-specific providers SHALL additionally test their declared comparison semantics.

---

## 49. Architectural Invariants

The following invariants are mandatory:

1. Execution success is not verification.
2. Observation is not Current State.
3. A controller report is evidence, not authority.
4. Verification does not mutate external resources.
5. Verification does not commit Current State.
6. Only acceptable evidence may authorize State Acceptance.
7. Every Verification Result is bound to a State Basis.
8. Stale evidence cannot establish fresh truth.
9. Missing evidence is not automatically evidence of absence.
10. Unknown is not success.
11. Unknown is not failure.
12. Health and satisfaction are distinct.
13. Integrity and health are distinct.
14. Verification errors and resource conditions are distinct.
15. Verification decisions are auditable.
16. Verification decisions are deterministic under equivalent inputs.
17. Corrections create new Verification Results.
18. Resource-specific extensions cannot redefine core result semantics.

---

## 50. Minimal Verification Example

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

Observation:

```yaml
resource_id: service/ollama
source: system-service-observer
collected_at: 2026-07-23T09:00:00Z
observed:
  present: true
  enabled: true
  active_state: active
```

Verification Result:

```yaml
resource_id: service/ollama
purpose: desired-state-satisfaction
state_basis:
  desired_generation: 12
  current_revision: 41
overall_result: satisfied
conditions:
  - name: present
    result: satisfied
  - name: enabled
    result: satisfied
  - name: running
    result: satisfied
```

This result may be submitted to State Management.

It does not update Current State by itself.

---

## 51. Unknown Outcome Example

An operation requests installation of a package.

The process is interrupted before its completion status is recorded.

Operation State reports:

```yaml
status: interrupted
outcome: unknown
```

DAIA SHALL NOT assume that the package is absent.

DAIA SHALL NOT immediately repeat the operation solely because completion was not recorded.

Verification SHALL obtain fresh package-manager evidence.

The result may then be:

* `satisfied` if the package is installed as required;
* `unsatisfied` if reliable evidence proves it is absent or incorrect;
* `unknown` if package state cannot be established;
* `error` if verification processing fails.

---

## 52. Architectural Consequences

This architecture produces several consequences:

* DAIA can recover after interrupted operations without trusting incomplete operation records.
* Controllers remain replaceable because they do not own truth.
* State Acceptance remains auditable because every accepted claim has a Verification Result.
* Resource-specific verification may evolve without changing core result semantics.
* Reconciliation can distinguish corrective work from missing knowledge.
* Drift can be identified through renewed observation rather than controller assumptions.
* External nondeterminism does not weaken deterministic architectural decisions.

---

## 53. Final Statement

DAIA Verification converts attributable evidence into an explicit architectural conclusion.

It does not perform work.

It does not express intent.

It does not own Current State.

Its authority is narrower and more important:

> Verification determines what the available evidence is sufficient to prove.

Only after that determination may State Management accept the corresponding representation of reality.
