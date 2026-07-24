# DAIA Provisioning Engine Architecture

## Overview

The provisioning engine converts a user's desired distribution into a machine that satisfies that specification.

The engine is composed of independent components, each with a single responsibility. Each component accepts a well-defined input and produces a well-defined output.

```
             User Configuration
                    │
                    ▼
        Configuration Manager
                    │
                    ▼
         Capability Resolver
                    │
                    ▼
      Required Capabilities
                    │
                    ▼
           Plugin Registry
                    │
                    ▼
          Candidate Plugins
                    │
                    ▼
           Plugin Selector
                    │
                    ▼
          Selected Plugins
                    │
                    ▼
        Desired State Builder
                    │
                    ▼
             Desired State
                    │
                    ▼
                Planner
                    │
                    ▼
            Execution Plan
                    │
                    ▼
           Resource Manager
                    │
                    ▼
              Verification
                    │
                    ▼
             State Database
```

---

# Design Principles

The provisioning engine follows these principles:

- Every component has a single responsibility.
- Components communicate only through defined data contracts.
- Components are stateless wherever possible.
- Plugins describe capabilities, not execution order.
- Planning and execution are separate concerns.
- Desired state is independent of implementation.

---

# Component Responsibilities

## Configuration Manager

### Input

Configuration files

### Output

Configuration object

### Responsibilities

- Load configuration
- Validate configuration syntax
- Provide configuration values

### Non-responsibilities

- Plugin discovery
- Capability resolution
- Planning
- System modification

---

## Capability Resolver

### Input

Configuration object

### Output

Required capability list

Example:

```
desktop
ai-engine
ssh-server
```

### Responsibilities

- Translate user intent into required capabilities

### Non-responsibilities

- Plugin selection
- Desired state construction

---

## Plugin Registry

### Input

Plugin metadata

### Output

Plugin catalog

### Responsibilities

- Discover plugins
- Validate plugin metadata
- Index plugins
- Answer lookup requests

### Non-responsibilities

- Read configuration
- Select plugins
- Execute plugins

---

## Plugin Selector

### Input

Required capabilities
Plugin catalog

### Output

Selected plugin list

Example:

```
desktop/xfce
ai-engine/ollama
service/openssh
```

### Responsibilities

- Select plugins that satisfy required capabilities

### Non-responsibilities

- Build desired state
- Execute plugins

---

## Desired State Builder

### Input

Selected plugins

### Output

Desired state

### Responsibilities

- Invoke plugin contribution hooks
- Merge contributions
- Produce desired state

### Non-responsibilities

- Package installation
- Service management

---

## Planner

### Input

Desired state

### Output

Execution plan

### Responsibilities

- Determine required operations
- Order operations
- Resolve dependencies

### Non-responsibilities

- Execute operations

---

## Resource Manager

### Input

Execution plan

### Output

Modified system

### Responsibilities

- Install packages
- Write files
- Configure services
- Execute commands

### Non-responsibilities

- Planning
- Verification

---

## Verification

### Input

System state

### Output

Verification results

### Responsibilities

- Verify desired state has been achieved
- Report deviations

### Non-responsibilities

- Correct failures

---

## State Database

### Input

Verified state

### Output

Persistent state record

### Responsibilities

- Record applied state
- Support future reconciliation

### Non-responsibilities

- System modification

---

# Data Flow

Configuration

↓

Required Capabilities

↓

Selected Plugins

↓

Desired State

↓

Execution Plan

↓

Verified System

↓

Persistent State

Each stage transforms one data structure into another without introducing hidden state or side effects.
