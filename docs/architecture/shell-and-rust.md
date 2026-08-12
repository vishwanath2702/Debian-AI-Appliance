# DAIA Shell and Rust Architecture

## Purpose

DAIA uses both Rust and shell, but they are not intended to represent two independent implementations of the same system.

The architecture should have one authoritative owner for each responsibility.

Rust and shell are complementary tools within a single DAIA architecture.

## Historical Context

DAIA's shell implementation came first.

That work established working behavior on a real Debian system and exposed the practical operations required for:

* system bootstrap
* package installation
* filesystem preparation
* service configuration
* ISO construction
* runtime setup
* recovery and system integration

The shell implementation also served as an architectural discovery phase.

By implementing real behavior first, DAIA was able to discover which responsibilities were policy and domain logic, and which responsibilities were low-level Linux execution.

The later Rust architecture grew from those lessons.

The shell work should therefore not be treated as discarded or unnecessary work.

## Architectural Principle

DAIA is one architecture implemented with the appropriate tools.

Rust is the authoritative control plane.

Shell may provide system-level execution where it is the appropriate implementation mechanism.

The important rule is:

> One responsibility should have one authoritative owner.

DAIA should not maintain independent Rust and shell implementations that both make the same architectural decisions.

## Rust Responsibilities

Rust should own structured DAIA policy and domain behavior, including:

* domain models
* appliance profiles
* capabilities
* provider registries
* package manifests
* content repositories
* content sources
* storage models
* dependency and capability resolution
* execution planning
* validation
* state transitions
* Wizard logic
* appliance configuration
* orchestration
* error handling
* testable system behavior

Examples include:

```text
ApplianceProfile
        ↓
Registry
        ↓
Resolver
        ↓
Planner
        ↓
Engine
```

Rust should be the source of truth for decisions such as:

* what an appliance profile means
* what capabilities an appliance requires
* which provider satisfies a capability
* what execution plan should be produced
* what configuration is valid

## Shell Responsibilities

Shell may remain appropriate for tightly scoped Linux and Debian operations, including:

* bootstrap helpers
* filesystem operations
* invoking Debian utilities
* invoking package-management tools
* recovery tasks
* service-management helpers
* short system-integration sequences
* operational tasks where shell is clearer than an equivalent Rust implementation

A shell helper should normally receive a well-defined operation rather than independently deciding appliance policy.

For example:

```text
Rust decision:
    Enable service "example.service"

Execution helper:
    systemctl enable example.service
```

The helper does not need to know why the service was selected or which appliance profile requested it.

## What DAIA Must Avoid

DAIA should not maintain duplicated authority.

For example, this is undesirable:

```text
Rust:
    wanderer requires:
      - offline-maps
      - offline-knowledge
      - ai-runtime

Shell:
    wanderer requires:
      - offline-maps
      - offline-knowledge
      - ai-runtime
```

This creates two sources of truth that can drift over time.

Likewise, DAIA should not maintain both a Rust planner and a shell planner that independently determine execution behavior.

## Existing Shell Code

Existing shell code is not automatically deprecated.

Each shell component should be evaluated according to its actual responsibility.

A component may:

1. remain as a shell implementation if shell is still the appropriate tool;
2. become a helper invoked from the Rust execution layer;
3. be retired if its responsibility has been completely and demonstrably superseded by Rust.

No shell component should be removed solely because a Rust implementation exists elsewhere.

Before retiring a shell component, DAIA should verify:

* what behavior the component currently owns;
* whether that behavior is represented elsewhere;
* whether tests cover the replacement;
* whether any operational edge cases would be lost.

## Direction for New Development

New DAIA domain and policy functionality should be implemented in the Rust/YAML architecture.

New architectural concepts should not be independently implemented in shell first and then duplicated in Rust.

Shell should grow only when there is a clearly scoped system-level operation for which shell is an appropriate implementation.

This gives DAIA the following direction:

```text
Declarative definitions
        ↓
Rust domain model
        ↓
Registry / Resolver / Planner
        ↓
Rust Engine
        ↓
Execution boundary
        ↓
Rust executor and/or shell helper
        ↓
Debian / Linux tools
```

## Relationship to the Appliance Build

DAIA appliance profiles describe capabilities, not bulk payloads.

A build such as:

```text
./build.sh --build wanderer
```

should select a declarative appliance profile.

The profile determines the capabilities that must be present in the resulting appliance.

The build system should produce a minimal ISO containing the operating system, DAIA runtime, required capability software, and provisioning logic.

Large resources such as:

* AI models
* Wikipedia data
* map datasets
* user content
* large container images

should normally be acquired after installation.

## Post-Install Architecture

The installed DAIA system should support post-install configuration of:

* resource selection
* AI model selection
* Wikipedia and other knowledge repositories
* offline maps
* user content
* language and locale
* storage targets
* native storage
* secondary disks
* removable USB storage

The expected lifecycle is:

```text
Minimal DAIA ISO
        ↓
Installation
        ↓
Post-install Wizard
        ↓
Select resources
        ↓
Select storage
        ↓
Acquire and verify resources
        ↓
Configure local services
        ↓
Fully offline appliance
```

Network access may be required during provisioning, but configured appliance operation should not depend on continuous Internet access.

## Summary

DAIA does not have a goal of becoming either "all Rust" or "all shell."

The goal is:

* one coherent architecture;
* one authoritative owner for each responsibility;
* Rust for structured control-plane logic;
* shell where it remains useful for system-level execution;
* no duplicated policy;
* gradual, test-backed evolution of existing components.

The central rule is:

> Rust and shell may coexist permanently, but duplicated authority should not.
