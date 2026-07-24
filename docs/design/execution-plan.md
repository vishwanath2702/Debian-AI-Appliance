# Execution Plan Specification

**Status:** Draft

**Version:** 1.0

---

# Purpose

The Execution Plan is the output of the Planner and the input to the Executor.

It represents a fully validated sequence of plugins that shall be executed to perform an installation.

The Execution Plan is deterministic and contains no unresolved dependencies or capabilities.

---

# Design Goals

The Execution Plan shall be:

- deterministic
- complete
- ordered
- immutable after creation
- easy to inspect
- easy to test

---

# Planner Guarantees

When an Execution Plan is produced, the Planner guarantees that:

- every plugin exists
- every dependency has been resolved
- every capability has been resolved
- duplicate plugins have been removed
- plugin ordering is valid
- no conflicts exist

The Executor shall not repeat these checks.

---

# Executor Assumptions

The Executor assumes:

- the plan is valid
- plugins appear in execution order
- dependencies always precede dependents
- no plugin appears more than once

If these assumptions are violated, the Execution Plan is considered invalid.

---

# Initial Representation

Version 1.0 represents an Execution Plan as an ordered list of plugin identifiers.

Example:

```text
filesystem/base
package-manager/apt
desktop/xfce
browser/firefox
ai/ollama
```

Each line represents one plugin.

Execution proceeds from top to bottom.

---

# Ordering Rules

The following rules apply:

- dependencies appear before dependents
- ordering is deterministic
- duplicate entries are prohibited

Example:

```
filesystem/base
package-manager/apt
desktop/xfce
```

NOT

```
desktop/xfce
filesystem/base
package-manager/apt
```

---

# Validation

An Execution Plan is considered valid if:

- every plugin exists
- no duplicates exist
- dependency ordering is correct
- conflict checking has completed successfully

The Executor does not perform validation.

---

# Immutability

After creation, an Execution Plan shall not be modified.

Any change requires rebuilding the plan through the Planner.

---

# Future Extensions

Future versions may associate metadata with each plugin.

Examples:

- execution timeout
- retry policy
- plugin version
- execution flags
- architecture constraints

The public Planner and Executor APIs should not change when these extensions are added.

---

# Example

```text
filesystem/base
package-manager/apt
network/networkmanager
display-server/xorg
desktop/xfce
display-manager/lightdm
applications/firefox
```

The Executor executes each plugin in the listed order.

---

# Design Principles

The Execution Plan is:

- simple
- deterministic
- human-readable
- machine-readable
- independent of implementation details
