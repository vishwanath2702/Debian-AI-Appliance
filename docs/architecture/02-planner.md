# DAIA Planner Architecture

## 1. Purpose

The Planner is responsible for transforming the desired system state into a
validated, ordered execution plan.

It is the decision-making component of DAIA.

The Planner determines *what* should be done and *in what order*. It does not
perform installation or runtime operations itself.

---

## 2. Architectural Role

The Planner sits between user intent (or desired state) and execution.

Conceptually:

```text
Desired State
      |
      v
Capability Resolution
      |
      v
Dependency Resolution
      |
      v
Conflict Detection
      |
      v
Execution Plan
      |
      v
Builder / Runtime Engine
```

The Planner is responsible for producing a complete and internally consistent
plan that downstream components can execute.

---

## 3. Responsibilities

The Planner is responsible for:

* reading desired state;
* loading registry information;
* resolving requested capabilities;
* resolving dependencies;
* detecting conflicts;
* validating hardware compatibility;
* determining execution order;
* generating a deterministic execution plan.

The Planner answers one question:

> **What needs to happen to reach the desired state?**

---

## 4. Non-Responsibilities

The Planner does **not**:

* install packages;
* download resources;
* configure services;
* modify files;
* execute modules;
* verify completed work;
* interact directly with users.

Those responsibilities belong to downstream components.

---

## 5. Inputs

Typical Planner inputs include:

* desired state;
* installation profile;
* hardware information;
* plugin registry;
* capability registry;
* dependency metadata;
* current state (runtime planning).

These inputs should be treated as read-only.

---

## 6. Outputs

The Planner produces an execution plan describing:

* ordered operations;
* dependency graph;
* required capabilities;
* selected implementations;
* execution constraints;
* verification requirements.

The execution plan becomes the contract between planning and execution.

---

## 7. Planning Stages

The planning lifecycle consists of:

```text
Load Desired State
        |
        v
Load Registries
        |
        v
Resolve Capabilities
        |
        v
Resolve Dependencies
        |
        v
Detect Conflicts
        |
        v
Validate Hardware
        |
        v
Generate Execution Plan
        |
        v
Validate Plan
```

Each stage should produce explicit errors if planning cannot continue.

---

## 8. Capability Resolution

Desired state is expressed in terms of capabilities rather than implementation
details.

For example:

```text
Capability:
    AI Engine

Possible implementations:
    Ollama
    vLLM
    llama.cpp
```

The Planner selects valid implementations based on the current system and the
available registry data.

---

## 9. Dependency Resolution

Dependencies are resolved recursively.

Example:

```text
Desktop
   |
   +-- Display Manager
   |
   +-- GPU Driver
   |
   +-- Container Runtime
```

The Planner ensures that prerequisites appear before dependent operations.

---

## 10. Conflict Detection

Conflicting selections are detected before execution begins.

Examples include:

* incompatible AI engines;
* mutually exclusive services;
* unsupported hardware combinations;
* conflicting package providers.

Execution should not begin until conflicts are resolved.

---

## 11. Hardware Awareness

Hardware information influences planning.

Examples:

* CPU architecture;
* GPU vendor;
* available RAM;
* storage capacity;
* virtualization support;
* accelerator availability.

The Planner adapts the execution plan to the detected hardware.

---

## 12. Deterministic Planning

Given identical inputs, the Planner should always generate the same execution
plan.

Planning should not depend on random ordering or filesystem traversal order.

Deterministic plans improve reproducibility, testing, and debugging.

---

## 13. Planner and Builder

The Planner and Builder have distinct responsibilities.

The Planner decides:

> **What should happen?**

The Builder performs:

> **How the planned operations are coordinated and executed.**

The Builder must not reinterpret the plan.

---

## 14. Planner and Runtime Engine

The Runtime Engine may invoke the Planner whenever desired state changes.

Conceptually:

```text
Desired State
      +
Current State
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

The Planner remains responsible for producing the plan regardless of whether
execution occurs during installation or long after deployment.

---

## 15. Error Handling

Planning failures should be explicit.

Typical failure categories include:

* missing capabilities;
* unresolved dependencies;
* circular dependencies;
* conflicting selections;
* unsupported hardware;
* invalid configuration.

Planning should fail before execution begins.

---

## 16. Testing

Planner testing should include:

* dependency resolution;
* conflict detection;
* capability selection;
* deterministic output;
* invalid registry data;
* circular dependency detection;
* hardware-specific planning;
* malformed desired state.

---

## 17. Future Work

Future enhancements may include:

* incremental planning;
* plan caching;
* plan visualization;
* cost-based optimization;
* runtime re-planning;
* execution rollback planning.

---

## 18. Summary

The Planner is the architectural bridge between desired state and execution.

Its role is to transform *intent* into an ordered, validated execution plan.

By keeping planning independent of execution, DAIA maintains clear subsystem
boundaries, simplifies testing, and enables both installation-time and
runtime management to share the same planning logic.
