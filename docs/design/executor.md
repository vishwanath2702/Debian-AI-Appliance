# Executor Design

**Status:** Draft

**Version:** 1.0

---

# Overview

The Executor is responsible for executing a validated execution plan produced by the Planner.

The Executor does **not** perform planning, dependency resolution, capability resolution, conflict detection, or profile parsing. Those responsibilities belong to the Planner subsystem.

The Executor receives a complete execution plan and executes the listed plugins in the specified order.

The Executor is the only subsystem responsible for invoking plugin lifecycle hooks.

---

# Goals

The Executor shall:

- Execute plugins in dependency order.
- Provide a consistent lifecycle for every plugin.
- Prepare a shared execution context.
- Report progress.
- Capture execution results.
- Stop execution on fatal errors.
- Produce deterministic behaviour.

---

# Non-Goals

The Executor shall **not**:

- Parse installation profiles.
- Discover plugins.
- Resolve capabilities.
- Resolve dependencies.
- Detect conflicts.
- Build installation media.
- Roll back transactions.

These responsibilities belong to other subsystems.

---

# Architecture

```
Installer
    │
    ▼
Planner
    │
    ▼
Execution Plan
    │
    ▼
Executor
    │
    ▼
Transaction Engine
```

---

# Public API

The Executor exposes a single public function.

```bash
daia_executor_execute_plan <execution-plan>
```

No other Executor functions should be called directly by external subsystems.

---

# Internal Modules

```
executor/
│
├── executor.sh
│
├── execution-context.sh
├── plugin-loader.sh
├── lifecycle.sh
├── hook-dispatcher.sh
├── progress-reporter.sh
└── error-handler.sh
```

Each module has a single responsibility.

---

# Execution Pipeline

```
Execution Plan
      │
      ▼
Load Plugin
      │
      ▼
Validate
      │
      ▼
Initialize
      │
      ▼
Execute
      │
      ▼
Verify
      │
      ▼
Finalize
```

---

# Plugin Lifecycle

Every plugin may implement zero or more lifecycle hooks.

Hooks that are not implemented are skipped.

Supported hooks:

```bash
daia_plugin_validate

daia_plugin_initialize

daia_plugin_execute

daia_plugin_verify

daia_plugin_finalize
```

Future versions may add:

```bash
daia_plugin_rollback
```

---

# Lifecycle Semantics

## Validate

Checks prerequisites.

Must not modify the target system.

---

## Initialize

Prepares resources required during execution.

---

## Execute

Performs the plugin's primary work.

This is the only hook expected to modify the target system.

---

## Verify

Confirms that execution completed successfully.

---

## Finalize

Performs cleanup.

This hook should execute whenever practical, even if Execute fails after partial work, unless continuing would be unsafe.

---

# Execution Context

The Executor prepares a shared execution context before executing plugins.

The initial context includes:

- Target installation root
- Installation profile
- Configuration
- Logger
- Environment
- Transaction handle

Plugins should access execution state only through the defined context rather than relying on arbitrary global variables.

---

# Error Handling

Errors are classified as fatal or non-fatal.

Fatal errors terminate execution immediately.

Non-fatal errors are reported to the caller.

The Executor itself does not perform rollback.

---

# Progress Reporting

The Executor reports:

- Current plugin
- Completed plugins
- Failed plugins
- Overall progress

The reporting mechanism is implementation-specific.

---

# Determinism

Given:

- the same execution plan
- the same plugin versions
- the same execution context

the Executor should produce identical behaviour.

---

# Future Extensions

The following features are intentionally excluded from the initial implementation:

- Parallel execution
- Dry-run mode
- Resume support
- Checkpoints
- Rollback
- Interactive progress UI
- Remote execution

These features may be added without changing the Executor's public API.

---

# Design Principles

The Executor follows these principles:

- Single Responsibility Principle
- Explicit lifecycle
- Deterministic execution
- Minimal public API
- Modular implementation
- Testability
