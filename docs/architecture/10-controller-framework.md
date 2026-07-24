# DAIA Controller Framework

**Document:** `10-controller-framework.md`
**Architecture Version:** 1.0
**Status:** Draft
**Depends On:** `09-state-architecture.md`
**Used By:** `11-verification.md`, `12-reconciliation.md`

---

# 1. Purpose

This document defines the DAIA Controller Framework.

The Controller Framework provides the execution layer that performs operations on Managed Resources and produces observations about those resources.

A Resource Controller encapsulates resource-specific knowledge while presenting a uniform architectural interface to the rest of DAIA.

The Controller Framework answers:

> **How does DAIA interact with a specific type of managed resource?**

Controllers execute work.

Controllers observe resources.

Controllers do **not** own Desired State, Current State, planning, reconciliation, or verification.

---

# 2. Scope

This document defines:

* Resource Controllers
* Controller Capabilities
* Controller Contracts
* Registration
* Discovery
* Eligibility
* Capability Resolution
* Controller Selection
* Operations
* Observations
* Lifecycle
* Controller Results
* Versioning
* Isolation
* Extensibility
* Conformance

This document does **not** define:

* Desired State
* Current State
* State Acceptance
* Verification semantics
* Reconciliation planning
* Scheduling
* Persistence
* Resource schemas

---

# 3. Architectural Position

```text
Desired State
        │
        ▼
Reconciliation
        │
        ▼
Controller Framework
        │
        ▼
Resource Controllers
        │
        ▼
External Resources
        │
        ▼
Observations
        │
        ▼
Verification
```

Controllers are the only architectural components that directly interact with managed resources.

---

# 4. Core Principle

Controllers are adapters between DAIA's declarative resource model and concrete resource technologies.

A controller knows **how** to perform work.

It never decides **whether** work should happen.

---

# 5. Controller Authority

A Resource Controller SHALL:

* perform supported operations;
* collect observations;
* report operation outcomes;
* expose supported capabilities;
* validate operation inputs;
* preserve operation identity.

A Resource Controller SHALL NOT:

* modify Desired State;
* commit Current State;
* declare resources verified;
* construct reconciliation plans;
* schedule execution;
* select itself;
* modify another controller's state.

---

# 6. Controller Model

Each Resource Type is managed through one or more Resource Controllers.

Examples include:

* Package Controller
* Service Controller
* File Controller
* Directory Controller
* Container Controller
* Image Controller
* Network Controller
* Model Controller
* AI Engine Controller
* Interface Controller

A Resource Type MAY have multiple compatible controller implementations.

---

# 7. Controller Identity

Every controller SHALL declare:

* Controller ID
* Name
* Version
* Vendor
* Resource Types
* Supported schema versions
* Capability set
* Platform compatibility
* Interface version

The identity is immutable for the controller instance.

---

# 8. Controller Capabilities

Capabilities describe what a controller is able to perform.

Examples include:

* Create
* Update
* Delete
* Start
* Stop
* Restart
* Enable
* Disable
* Install
* Uninstall
* Observe
* Verify-support
* Health-check
* Import
* Export

Capabilities are declarative.

They describe available behavior rather than implementation.

---

# 9. Capability Declaration

Every controller SHALL explicitly declare:

* supported operations;
* supported Resource Types;
* supported schema versions;
* observation support;
* verification support;
* destructive operations;
* concurrency limitations;
* dependency requirements;
* platform limitations.

Undeclared capabilities SHALL NOT be assumed.

---

# 10. Registration

Controllers SHALL register with the Controller Registry.

Registration SHALL include:

* identity;
* capabilities;
* supported versions;
* compatibility;
* interface revision.

Registration makes a controller discoverable.

It does not automatically make the controller eligible.

---

# 11. Discovery

The Controller Framework SHALL support discovery of registered controllers.

Discovery SHALL allow filtering by:

* Resource Type;
* capability;
* platform;
* schema version;
* controller version;
* compatibility.

Discovery produces candidates only.

---

# 12. Eligibility

A controller is eligible only when:

* it supports the Resource Type;
* required capabilities exist;
* schema versions are compatible;
* platform requirements are satisfied;
* policy permits its use.

Ineligible controllers SHALL NOT receive work.

---

# 13. Controller Selection

Controller selection is performed by Reconciliation.

The Controller Framework supplies eligible candidates.

Selection SHALL be deterministic for equivalent inputs.

---

# 14. Controller Contract

Every controller SHALL expose a uniform contract consisting of:

* Execute Operation
* Observe Resource
* Describe Capabilities
* Describe Supported Schemas
* Report Version

Additional interfaces MAY exist but SHALL NOT replace the standard contract.

---

# 15. Operations

An operation represents one executable mutation.

Examples:

* install package
* remove package
* create file
* update configuration
* start service
* stop container

Operations SHALL:

* be explicit;
* have validated inputs;
* return structured results.

---

# 16. Operation Results

Operation Results describe execution.

They MAY contain:

* accepted;
* rejected;
* started;
* completed;
* failed;
* cancelled;
* interrupted;
* timeout.

Operation Results SHALL NOT become Current State.

---

# 17. Observation

Controllers MAY observe resources.

Observations are evidence.

Observations SHALL identify:

* resource;
* source;
* timestamp;
* schema version;
* collected values.

Observations SHALL NOT be treated as verified truth.

---

# 18. Verification Support

Controllers MAY implement helper functionality for verification.

Examples include:

* service inspection;
* package queries;
* container inspection;
* checksum calculation.

Verification remains responsible for evaluating evidence.

---

# 19. Error Reporting

Controllers SHALL return structured errors.

Errors SHALL distinguish:

* invalid request;
* unsupported capability;
* unsupported schema;
* permission denied;
* external failure;
* timeout;
* interruption;
* internal error.

Errors SHALL NOT redefine resource state.

---

# 20. Versioning

Controllers SHALL version:

* interface;
* implementation;
* supported schemas;
* capabilities.

Compatibility SHALL be explicit.

---

# 21. Isolation

Controllers SHALL be isolated.

One controller SHALL NOT directly manipulate another controller's resources.

Shared infrastructure SHALL be accessed only through defined interfaces.

---

# 22. Concurrency

Controllers MAY support concurrent execution.

Concurrency limitations SHALL be declared.

The framework SHALL honor those limitations.

---

# 23. Extensibility

New Resource Types are introduced by adding new controllers.

Existing architectural interfaces remain unchanged.

This allows DAIA to evolve without redesigning the framework.

---

# 24. Determinism

Given equivalent:

* operation requests;
* parameters;
* environment;
* controller version;

a controller SHOULD produce equivalent behavior.

External systems remain inherently nondeterministic.

---

# 25. Auditability

Every operation SHALL be attributable.

The framework SHALL preserve:

* controller identity;
* operation identity;
* timestamps;
* resource identity;
* returned results.

---

# 26. Security

Controllers SHALL operate only within their declared authority.

They SHALL NOT:

* access unrelated resources;
* bypass authorization;
* expose secrets unnecessarily.

---

# 27. Conformance

A conforming controller SHALL:

1. expose a standard contract;
2. declare capabilities;
3. support explicit versioning;
4. validate inputs;
5. return structured results;
6. produce observations when applicable;
7. avoid modifying Current State;
8. avoid verification decisions;
9. avoid reconciliation decisions;
10. remain replaceable.

---

# 28. Architectural Invariants

The following invariants are mandatory:

1. Controllers execute work.
2. Controllers observe resources.
3. Controllers do not own Desired State.
4. Controllers do not own Current State.
5. Controllers do not verify truth.
6. Controllers do not reconcile state.
7. Controllers are replaceable.
8. Capabilities are explicitly declared.
9. Eligibility precedes execution.
10. Operation Results are not Current State.
11. Observations are evidence.
12. Verification evaluates evidence.
13. State Management accepts Current State.
14. Reconciliation decides what work should happen.
15. Controllers decide only how to perform requested work.

---

# 29. Example

```text
Desired State
      │
      ▼
Reconciliation
      │
Select Package Controller
      │
Execute Install
      │
Package Manager
      │
Observe Installed Package
      │
Verification
      │
State Acceptance
```

The Package Controller installs the package.

Verification determines whether the package is actually installed.

State Management decides whether the verified condition becomes Current State.

---

# 30. Final Statement

The Controller Framework provides the execution abstraction between DAIA's declarative architecture and concrete resource technologies.

Controllers perform work and collect observations.

They never determine architectural truth, never decide reconciliation strategy, and never own state.

By isolating resource-specific behavior behind a stable contract, the Controller Framework enables DAIA to support new resource types and execution technologies without changing the platform's core architecture.
