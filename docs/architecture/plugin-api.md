# DAIA Plugin API

## 1. Purpose

DAIA plugins translate validated installation configuration into declarative
resources.

Plugins contain capability-specific knowledge such as:

- desktop environments,
- AI engines,
- AI interfaces,
- container platforms,
- developer tools,
- optional system services.

Plugins do not perform installation work directly.

They contribute resources to the Desired State Object. The Planner and Resource
Manager are responsible for deciding how and when those resources are applied.

---

## 2. Architectural position

```text
Installation Configuration
        ↓
Configuration Manager
        ↓
Plugin Registry
        ↓
Capability Plugins
        ↓
Desired State Builder
        ↓
Sealed Resource Graph
        ↓
Planner
