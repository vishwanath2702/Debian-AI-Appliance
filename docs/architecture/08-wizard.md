# DAIA Installation Wizard Architecture

## 1. Purpose

The DAIA Installation Wizard provides the primary guided interface for
collecting installation intent and producing validated DAIA configuration.

It helps a user describe the system they want without requiring detailed
knowledge of:

* package names;
* service units;
* module identifiers;
* dependency graphs;
* container images;
* model file layouts;
* installer scripts;
* execution handlers.

The Wizard translates user choices into structured configuration.

It does not directly perform installation operations.

---

## 2. Architectural Role

The Wizard sits at the boundary between the user and the DAIA configuration and
planning systems.

Conceptually:

```text
User
  |
  v
Installation Wizard
  |
  v
Installation Selections
  |
  v
Configuration Manager
  |
  v
Effective Configuration
  |
  v
Desired State
  |
  v
Planner
  |
  v
Execution Plan
  |
  v
Installer, Builder, or Runtime Engine
```

The Wizard gathers intent.

The Configuration Manager validates and normalizes it.

The Planner determines how that intent can be satisfied.

Execution components perform the resulting work.

---

## 3. Status

The Wizard is a planned subsystem unless an existing implementation is
introduced separately.

This document defines its intended architecture and interfaces.

It should not be interpreted as evidence that all described screens, workflows,
validation rules, or integrations have already been implemented.

The initial Wizard should be deliberately limited and built on stable
configuration contracts.

---

## 4. Responsibilities

The Wizard is responsible for:

* presenting supported installation choices;
* displaying detected hardware information;
* explaining relevant tradeoffs;
* collecting user intent;
* validating field-level input;
* submitting selections to the Configuration Manager;
* displaying configuration errors;
* presenting planning warnings and conflicts;
* showing an installation review;
* obtaining explicit confirmation;
* writing or submitting one installation-selection document;
* presenting progress produced by execution components;
* presenting final success or failure information.

The Wizard answers:

> What DAIA system does the user want to create?

---

## 5. Non-Responsibilities

The Wizard must not:

* install packages directly;
* execute shell commands for system mutation;
* copy payload files;
* enable services;
* import container images;
* install AI models;
* resolve dependencies itself;
* choose execution order;
* bypass configuration validation;
* modify current state directly;
* invent unsupported implementations;
* treat UI state as authoritative persistent state;
* contain module-specific installation logic.

The Wizard is an interface and configuration-authoring component.

It is not an orchestration engine.

---

## 6. Design Goals

The Wizard should provide:

* a clear guided workflow;
* safe defaults;
* progressive disclosure;
* hardware-aware recommendations;
* deterministic output;
* early validation;
* explicit review before execution;
* clear explanation of conflicts;
* accessibility;
* recoverable sessions;
* support for both novice and advanced users;
* separation from execution logic.

The user should be able to understand what will be installed without needing to
understand the complete internal architecture.

---

## 7. User Types

The Wizard may serve several user types.

### 7.1 Guided User

A guided user wants a working DAIA system with minimal technical choices.

The Wizard should offer:

* use-case selection;
* recommended profile;
* simple storage choices;
* automatic implementation selection;
* concise review.

### 7.2 Advanced User

An advanced user may select:

* specific implementations;
* model variants;
* service settings;
* storage paths;
* runtime options;
* verification level.

Advanced options should remain schema-validated.

### 7.3 Administrator

An administrator may use:

* site policy;
* unattended profiles;
* signed configuration;
* preselected constraints;
* fleet-specific defaults.

The Wizard should clearly indicate settings that are locked by policy.

### 7.4 Installer Operator

An installer operator may prepare a system for another user.

The Wizard should support review and export without requiring the operator to
be the final system user.

---

## 8. Interaction Modes

DAIA may eventually support several Wizard modes.

```text
guided
profile-based
advanced
unattended review
repair or reconfigure
```

The initial implementation should focus on guided installation.

All modes should produce the same normalized installation-selection schema.

A separate internal configuration format should not be created for each UI
mode.

---

## 9. Guided Workflow

A possible guided installation flow is:

```text
Welcome
   |
   v
Environment Check
   |
   v
Use Case
   |
   v
Hardware Summary
   |
   v
Profile Recommendation
   |
   v
Capability Selection
   |
   v
Model Selection
   |
   v
Storage and Network
   |
   v
System Identity
   |
   v
Review
   |
   v
Validation and Planning
   |
   v
Confirmation
   |
   v
Execution Progress
   |
   v
Completion
```

The exact number of screens may vary.

The conceptual stages should remain distinct.

---

## 10. Welcome Stage

The Welcome stage should explain:

* what DAIA will install;
* whether the process modifies the machine;
* whether the installation can operate offline;
* what information the user will need;
* whether existing data may be affected;
* how to cancel safely before execution.

The Wizard should not hide destructive consequences behind generic wording.

Storage modifications require particularly clear disclosure.

---

## 11. Environment Check

Before gathering detailed choices, the Wizard may perform a read-only
environment check.

It may validate:

* required installer environment;
* available payload;
* target architecture;
* available disks;
* memory;
* detected accelerators;
* installation-media integrity;
* required configuration schemas;
* supported platform.

This stage should not make persistent system changes.

Critical incompatibilities should be reported before the user spends time
configuring the installation.

---

## 12. Hardware Summary

The Wizard should present detected hardware facts in understandable terms.

Examples include:

* processor architecture;
* memory;
* storage devices;
* GPU or accelerator;
* available disk space;
* network availability;
* firmware mode.

The displayed values should originate from the hardware-detection subsystem.

The Wizard should not independently probe hardware using unrelated logic if a
canonical detector exists.

---

## 13. Hardware Recommendations

The Wizard may present hardware-aware recommendations.

Examples:

* recommend a CPU-only AI engine when no supported GPU exists;
* recommend a smaller model for limited memory;
* recommend additional storage for an offline-complete profile;
* warn when a selected model exceeds available disk capacity;
* hide unsupported GPU implementations.

Recommendations should come from validated policy, registry, or Planner
information.

They must not be hard-coded inconsistently across UI screens.

---

## 14. Use-Case Selection

A guided user should be able to describe their intended use at a high level.

Possible use cases include:

```text
local AI assistant
AI development workstation
offline knowledge system
general desktop with AI tools
server or headless deployment
custom installation
```

Use cases should map to profiles or initial configuration recommendations.

They should not map directly to shell scripts or package lists.

---

## 15. Profile Recommendation

The Wizard may recommend a distribution profile based on:

* selected use case;
* hardware facts;
* offline requirements;
* available storage;
* site policy;
* target architecture.

A recommendation should explain the major consequences.

For example:

```text
Recommended: GPU Workstation

Includes:
- desktop environment;
- supported GPU runtime;
- local AI engine;
- standard development tools;
- selected model bundle.

Estimated storage:
- 48 GiB installation;
- 80 GiB recommended free space.
```

The user may select another valid profile when policy allows it.

---

## 16. Capability Selection

The Wizard should present capabilities rather than internal implementation
details by default.

Examples include:

* local AI inference;
* container execution;
* desktop environment;
* development tools;
* model serving;
* monitoring;
* remote access.

The user should not normally need to select package names or module scripts.

Capability selections become configuration input for the Planner.

---

## 17. Implementation Selection

Advanced users may select a specific capability implementation.

Example:

```text
AI engine:
  Automatic
  Ollama
  llama.cpp
  vLLM
```

The `Automatic` option should allow the Planner to choose a valid
implementation.

Explicit implementations should be shown only when compatible with:

* hardware;
* selected profile;
* payload contents;
* site policy;
* other selected capabilities.

The Wizard should not display an implementation that cannot be satisfied.

---

## 18. Model Selection

Model selection may require presenting:

* model name;
* purpose;
* approximate storage size;
* memory requirement;
* hardware suitability;
* license;
* offline availability;
* quantization or variant;
* expected performance class.

The Wizard should avoid presenting an unstructured list of model filenames.

Models should be selected through registry-backed metadata.

---

## 19. Model Recommendations

Model recommendations may depend on:

* available memory;
* GPU memory;
* CPU capability;
* selected AI engine;
* available disk space;
* offline profile;
* intended use;
* licensing policy.

The recommendation system should provide reasons.

Example:

```text
Recommended because:
- fits within detected GPU memory;
- available on the installation media;
- compatible with the selected AI engine;
- licensed for local redistribution.
```

Recommendations should not guarantee performance that has not been measured.

---

## 20. Storage Configuration

Storage configuration may include:

* target disk;
* installation mode;
* partitioning policy;
* encryption choice;
* reserved free space;
* model storage location;
* container storage location;
* preservation of existing partitions.

Storage is a high-risk area.

The Wizard must clearly distinguish:

* automatic destructive installation;
* use of unallocated space;
* manual partitioning;
* preservation of existing data;
* complete disk erasure.

No destructive action should begin without explicit review and confirmation.

---

## 21. Storage Estimates

The Wizard should display estimated storage usage.

The estimate may include:

```text
base operating system
DAIA runtime
packages
container images
AI models
temporary installation space
recommended working reserve
```

The estimate should derive from payload and planning metadata where possible.

It should state that runtime data and future models may require additional
space.

---

## 22. Network Configuration

Network configuration may include:

* offline installation;
* automatic network configuration;
* static network values;
* proxy;
* approved package endpoints;
* remote resource permission;
* telemetry or update policy.

The Wizard should clearly show when network access is:

* required;
* optional;
* prohibited;
* unavailable.

An offline installation must not silently depend on remote downloads.

---

## 23. System Identity

System identity settings may include:

* hostname;
* locale;
* timezone;
* keyboard layout;
* administrator account;
* device name;
* optional organization information.

Sensitive credentials should not be stored in ordinary Wizard session data
without protection.

Password handling must use secure input controls and dedicated installation
interfaces.

---

## 24. User Accounts

Where the Wizard creates user accounts, it should collect only necessary
information.

Account configuration may include:

* username;
* display name;
* authentication method;
* administrator permission;
* home-directory policy.

The Wizard should validate:

* identifier format;
* reserved names;
* duplicate names;
* password policy;
* site-policy restrictions.

Passwords must not appear in logs or exported general configuration.

---

## 25. Review Stage

Before execution, the Wizard should show a complete review.

The review should include:

* target disk and destructive actions;
* selected profile;
* selected capabilities;
* chosen implementations;
* models;
* estimated storage;
* network requirements;
* system identity;
* policy constraints;
* warnings;
* plan summary;
* verification expectations.

The review should be generated from validated configuration and planning output.

It should not rely only on local UI state.

---

## 26. Review Categories

The review should distinguish:

```text
User choices
Automatic selections
Policy-enforced settings
Hardware-derived decisions
Warnings
Destructive actions
Unavailable optional features
```

This allows the user to understand which decisions they made and which DAIA
derived automatically.

---

## 27. Validation Before Confirmation

Before the user can confirm execution, the Wizard should ensure that:

* installation selections are syntactically valid;
* effective configuration is structurally valid;
* semantic validation passed;
* selected resources exist;
* the Planner produced a valid plan;
* no unresolved conflicts remain;
* destructive operations are identified;
* required licenses or terms are acknowledged where applicable.

Warnings may allow continuation.

Errors must block execution.

---

## 28. Planning Integration

The Wizard should submit validated configuration to the planning flow.

Conceptually:

```text
Wizard selections
       |
       v
Configuration Manager
       |
       v
Effective Configuration
       |
       v
Desired State
       |
       v
Planner
       |
       v
Plan Result
       |
       +-- valid plan
       +-- warnings
       +-- conflict
       +-- unsupported request
```

The Wizard presents the result.

It should not attempt to repair dependency graphs itself.

---

## 29. Planning Errors

Planning errors should be translated into understandable user-facing messages.

Internal error:

```text
capability ai-inference has no valid provider for architecture arm64
```

Possible user-facing explanation:

```text
The selected AI feature is not available for this device architecture.

Choose a different AI engine or continue without local AI inference.
```

The original structured error should remain available for diagnostics.

---

## 30. Conflict Resolution

Where multiple valid resolutions exist, the Wizard may present choices produced
by the Planner.

Example:

```text
The selected AI engine conflicts with the chosen GPU runtime.

Choose one:
- use the recommended GPU-compatible AI engine;
- keep the AI engine and use CPU execution;
- remove local AI inference.
```

The Wizard should not invent resolution options independently.

Each offered resolution must produce valid configuration.

---

## 31. Confirmation

Execution requires explicit user confirmation.

The confirmation should identify:

* the target system;
* destructive storage actions;
* whether network access will occur;
* whether licenses were accepted;
* the plan identifier;
* the configuration identifier.

A generic button such as `Continue` may be insufficient for destructive
actions.

A clearer action might be:

```text
Erase Disk and Install DAIA
```

The wording should match the actual operation.

---

## 32. Configuration Output

The Wizard should produce one installation-selection document or submit an
equivalent structured request.

A conceptual output may resemble:

```yaml
schema_version: 1

installation:
  mode: guided
  profile: gpu-workstation

system:
  hostname: daia-workstation
  locale: en_US.UTF-8
  timezone: Asia/Kolkata

capabilities:
  local-ai:
    enabled: true
    implementation: automatic
  desktop:
    enabled: true
  container-runtime:
    enabled: true

models:
  selected:
    - model-registry-id

storage:
  target: disk-registry-id
  policy: erase-and-install

network:
  offline_required: true
```

The exact schema belongs to the Configuration architecture.

---

## 33. No Direct Shell Configuration

The Wizard must not generate executable shell configuration from untrusted user
input.

It should produce structured data such as YAML or JSON.

Avoid output such as:

```bash
HOSTNAME="$USER_INPUT"
SELECTED_MODEL="$MODEL_INPUT"
```

that may later be sourced by privileged scripts.

Structured output must be parsed and schema-validated.

---

## 34. Wizard Session State

The Wizard needs temporary session state while the user navigates screens.

Session state may include:

* current step;
* entered values;
* detected hardware reference;
* selected profile;
* validation status;
* planning status;
* warning acknowledgements.

Session state is not authoritative installation state.

It should be converted into validated configuration before execution.

---

## 35. Session Persistence

The Wizard may support saving and resuming a session.

A saved session should include:

* Wizard schema version;
* session identifier;
* selected values;
* completed stages;
* hardware snapshot identifier;
* timestamps;
* optional validation results.

On resume, the Wizard should revalidate:

* hardware;
* payload availability;
* configuration schema;
* registry versions;
* storage targets.

A saved session must not bypass current validation.

---

## 36. Session Storage

Possible session storage locations include:

```text
/run/daia/wizard/
```

for transient installer sessions, or:

```text
/var/lib/daia/wizard/sessions/
```

for resumable installed-system workflows.

The final location should follow the state architecture.

Session files should use restrictive permissions when they contain personal or
sensitive information.

---

## 37. Session Expiration

Temporary sessions should expire.

Expiration policy may consider:

* creation time;
* last activity;
* installer restart;
* configuration version;
* hardware changes;
* completed execution;
* explicit cancellation.

Expired sessions should not be resumed automatically.

Sensitive transient data should be deleted securely where practical.

---

## 38. Wizard State Machine

The Wizard should use an explicit state machine.

Possible states include:

```text
initializing
collecting-input
validating
planning
reviewing
awaiting-confirmation
executing
completed
failed
cancelled
```

Valid transitions should be defined.

For example:

```text
collecting-input
    -> validating
    -> planning
    -> reviewing
    -> awaiting-confirmation
    -> executing
```

The Wizard should not enter execution without a valid plan and confirmation.

---

## 39. Navigation

Users should be able to navigate backward before execution.

When a previous choice changes:

* dependent selections may become invalid;
* recommendations may change;
* planning results must be discarded;
* review must be regenerated.

The Wizard should track dependencies between fields so stale derived values are
not retained silently.

---

## 40. Derived Values

Some displayed values are derived rather than entered.

Examples include:

* selected implementation;
* required packages;
* storage estimate;
* supported models;
* dependency list;
* expected installation duration category.

Derived values should:

* identify their source;
* be recomputed after relevant changes;
* not be persisted as independent user choices unless explicitly locked;
* be validated by downstream components.

---

## 41. Default Values

Defaults should come from the Configuration Manager, profile metadata, or
policy.

The Wizard should not maintain separate hidden defaults.

Displaying a default should indicate whether it is:

* recommended;
* profile-defined;
* hardware-derived;
* policy-enforced;
* previously selected.

A default selection should still appear in the final review.

---

## 42. Advanced Mode

Advanced mode may expose implementation-specific configuration.

Examples include:

* service port;
* container storage driver;
* model context size;
* inference concurrency;
* GPU allocation;
* verification timeout;
* package source.

Advanced fields should be provided by structured schemas and UI metadata.

Module-specific UI logic should not be hard-coded throughout the Wizard.

---

## 43. Schema-Driven Fields

Where practical, the Wizard should generate fields from configuration schemas
and registry metadata.

Field metadata may include:

* field identifier;
* label;
* description;
* type;
* allowed values;
* default;
* validation rules;
* visibility condition;
* sensitivity;
* advanced status.

A schema-driven approach reduces duplication between UI validation and backend
validation.

---

## 44. Conditional Visibility

Some fields should appear only when relevant.

Examples:

* GPU settings only when supported GPU hardware exists;
* model selection only when local AI is enabled;
* proxy settings only when network access is enabled;
* service-port settings only in advanced mode;
* encryption options only for supported storage policies.

Hidden values should not remain active silently after their parent feature is
disabled.

---

## 45. Field Validation

The Wizard may provide immediate field-level validation.

Examples include:

* invalid hostname;
* unsupported username;
* insufficient password length;
* invalid static IP address;
* invalid storage reserve;
* duplicate service port.

Client-side or interface-level validation improves usability.

Backend schema validation remains authoritative.

---

## 46. Cross-Field Validation

Some validation depends on multiple fields.

Examples include:

* selected model and available memory;
* offline mode and remote-only resource;
* storage encryption and partitioning mode;
* static network configuration and missing gateway;
* selected service port and conflicting service;
* profile and architecture compatibility.

Cross-field validation should use shared backend rules where possible.

The Wizard should not maintain an inconsistent duplicate rule set.

---

## 47. Warnings

Warnings should explain risk without blocking a valid installation.

Examples include:

* low remaining disk space;
* model likely to run slowly;
* network unavailable for optional updates;
* selected mutable container tag;
* unsupported optional accelerator;
* disabled health verification.

Warnings should be:

* specific;
* actionable;
* associated with the relevant field or review item;
* recorded when acknowledgement is required.

Warnings must not be used for unsafe or impossible configurations.

---

## 48. Errors

Errors block progression until resolved.

Examples include:

* invalid target disk;
* unsupported architecture;
* unavailable required payload resource;
* unresolved capability conflict;
* malformed configuration;
* insufficient disk space for required content;
* invalid plan;
* installation-media integrity failure.

The Wizard should preserve a structured error identifier for diagnostics.

---

## 49. Error Presentation

A user-facing error should include:

* what failed;
* why it matters;
* how to resolve it;
* whether current selections were preserved;
* optional diagnostic details.

Example:

```text
The selected model requires more storage than is available.

Required: 42 GiB
Available: 31 GiB

Select a smaller model or choose a different storage location.
```

Raw stack traces or shell output should not be the primary error presentation.

---

## 50. Accessibility

The Wizard should be usable with:

* keyboard-only navigation;
* screen readers;
* sufficient contrast;
* scalable text;
* clear focus indicators;
* non-color status indicators;
* understandable labels;
* predictable navigation.

Errors should be associated programmatically with their fields.

Progress should not rely solely on animation or color.

---

## 51. Localization

User-facing text should be separable from logic.

Localization support should consider:

* language;
* locale;
* date and time formats;
* number formats;
* measurement units;
* right-to-left layouts;
* text expansion;
* keyboard layout.

Machine identifiers and configuration values should remain locale-independent.

Localized labels must not become persisted internal identifiers.

---

## 52. Terminology

The Wizard should use consistent terms.

Examples:

* `profile` for a distribution configuration set;
* `capability` for a user-facing feature;
* `implementation` for a concrete provider;
* `model` for an AI model resource;
* `current state` for observed system condition;
* `desired state` for intended condition.

Internal architectural vocabulary should be simplified where necessary without
changing meaning.

---

## 53. Progress Interface

During execution, the Wizard should consume structured progress events.

Possible events include:

```text
operation-started
phase-started
module-started
resource-started
resource-completed
verification-started
warning
operation-completed
operation-failed
```

The Wizard should not parse arbitrary log lines to determine progress.

Logs may be displayed separately as diagnostic detail.

---

## 54. Progress Presentation

Execution progress should show:

* current phase;
* current component;
* completed steps;
* total known steps where available;
* current warning or failure;
* whether cancellation is safe;
* whether reboot will be required.

Percentages should be displayed only when they are meaningful.

A false precise percentage may be more misleading than a phase-based status.

---

## 55. Execution Boundary

After confirmation, the Wizard should submit the approved operation through a
defined execution interface.

It should not call module functions directly.

Conceptually:

```text
Approved plan
      |
      v
Execution API
      |
      +-- installer
      +-- Builder
      +-- Runtime Engine
      |
      v
Progress event stream
      |
      v
Wizard
```

The execution interface should provide explicit operation and plan identifiers.

---

## 56. Cancellation

The Wizard may allow cancellation only when the execution component reports it
as safe.

Before execution, cancellation should be immediate.

During execution, the UI should distinguish:

* safe to cancel now;
* cancellation requested;
* waiting for safe boundary;
* cannot cancel this operation;
* cleanup in progress.

The Wizard should not terminate privileged processes directly.

---

## 57. Failure Recovery

When execution fails, the Wizard should display:

* failed phase;
* failed component;
* user-relevant explanation;
* whether the system changed partially;
* whether cleanup completed;
* whether retry is safe;
* whether reboot is required;
* where diagnostics are stored.

Recovery actions must come from the execution system.

The Wizard should not assume that rerunning the same command is safe.

---

## 58. Retry

A retry action should trigger the supported recovery or replanning flow.

Preferred conceptual behavior:

```text
failure
   |
   v
inspect current state
   |
   v
reconcile
   |
   v
generate new plan
   |
   v
review changed actions if necessary
   |
   v
retry
```

For simple installer-stage failures, the subsystem may support direct retry.

The Wizard must follow the reported retry contract.

---

## 59. Completion

The completion screen should show:

* installation result;
* installed DAIA version;
* selected profile;
* major enabled capabilities;
* verification result;
* whether reboot is required;
* diagnostic or support location;
* next user action.

The Wizard should not show success until authoritative execution and
verification state indicates success.

---

## 60. Reboot

Where reboot is required, the Wizard may offer:

* reboot now;
* reboot later;
* shut down;
* return to diagnostics.

The execution subsystem should report whether reboot is:

* required;
* recommended;
* unnecessary;
* unsafe before cleanup.

The Wizard should not infer reboot requirements from package names.

---

## 61. Installation Summary

The Wizard should save or display a final installation summary.

The summary may include:

* configuration identifier;
* desired-state identifier;
* plan identifier;
* operation identifier;
* profile;
* selected capabilities;
* implementations;
* model inventory;
* installation time;
* verification result;
* warnings;
* final status.

Sensitive values must be excluded.

---

## 62. Export

The Wizard may support exporting:

* installation selections;
* normalized configuration;
* review summary;
* diagnostic bundle;
* unattended installation profile.

Exported configuration should be:

* schema-versioned;
* validated;
* free of resolved secrets;
* marked with provenance;
* suitable for re-import.

An exported review document should not be treated as executable
configuration.

---

## 63. Import

The Wizard may support importing a prepared configuration.

Import flow should include:

1. parse document;
2. validate schema;
3. verify signature where required;
4. check hardware compatibility;
5. apply site policy;
6. display imported choices;
7. regenerate effective configuration;
8. produce a new plan;
9. require review and confirmation.

Import must not bypass destructive-action confirmation.

---

## 64. Unattended Installation

Unattended installation may use a complete validated configuration without
interactive choices.

The Wizard's role may be limited to:

* validation;
* review when a display is available;
* progress presentation;
* failure diagnostics.

Unattended mode requires explicit policy for:

* destructive storage actions;
* credentials;
* license acceptance;
* configuration signing;
* failure behavior;
* reboot behavior.

Unattended configuration should use the same schema as guided installation.

---

## 65. Policy-Enforced Values

Site policy may lock or constrain fields.

The Wizard should show:

* the enforced value;
* why it is enforced;
* whether the field can be changed;
* the policy source where appropriate.

A disabled field should not appear broken or unexplained.

Policy values should be applied by the Configuration Manager.

The Wizard only presents the resulting constraint.

---

## 66. Licensing

Some models or third-party components may require license presentation or
acceptance.

The Wizard should display:

* component name;
* license identifier;
* relevant restrictions;
* whether redistribution or use is permitted;
* whether acceptance is required.

Acceptance should be recorded with:

* component identifier;
* license version;
* timestamp;
* actor or session;
* configuration or operation identifier.

The Wizard should not claim legal interpretation beyond the supplied metadata.

---

## 67. Privacy

The Wizard should collect only information required for installation.

It should clearly identify:

* information stored locally;
* information transmitted over the network;
* telemetry behavior;
* update checks;
* remote resource requests.

Optional data collection must not be enabled through ambiguous consent.

Privacy selections should be represented in configuration and policy.

---

## 68. Security

The Wizard may operate with or communicate with privileged components.

Security controls should include:

* separation of UI and privileged execution;
* schema validation;
* authentication of privileged requests;
* operation authorization;
* safe handling of credentials;
* protection against command injection;
* no shell evaluation of user input;
* path validation;
* secure session storage;
* event-source validation;
* secret redaction;
* audit logging.

The UI process should run with minimal privilege where possible.

---

## 69. Privilege Separation

A preferred architecture separates:

```text
Unprivileged Wizard UI
          |
          v
Validated request interface
          |
          v
Privileged DAIA service
          |
          v
Configuration, Planner, and Execution Components
```

The Wizard should not require unrestricted root access merely to render screens
or collect input.

Privileged operations should occur through narrow authenticated interfaces.

---

## 70. Request Authentication

A local privileged interface should verify that requests originate from an
authorized Wizard session or administrator.

Possible mechanisms may include:

* local Unix socket permissions;
* short-lived session token;
* process credentials;
* policy framework authorization;
* explicit installer environment trust.

The final mechanism depends on whether the Wizard runs during installation,
first boot, or normal runtime.

---

## 71. Input Security

All Wizard input must be treated as untrusted until validated.

Particularly sensitive fields include:

* hostname;
* username;
* paths;
* URLs;
* proxy values;
* model identifiers;
* service ports;
* advanced command-like parameters.

User input must never be concatenated into shell commands.

Structured APIs and validated argument arrays should be used.

---

## 72. Web-Based Wizard

If the Wizard uses a web interface, it should consider:

* cross-site request forgery;
* cross-site scripting;
* content security policy;
* local network exposure;
* session authentication;
* secure origin;
* clickjacking;
* file upload validation;
* API authorization;
* secret handling.

A local web Wizard should bind only to an appropriate interface by default.

It should not expose privileged installation APIs to the network unintentionally.

---

## 73. Desktop Wizard

A desktop Wizard may communicate with a privileged backend over a local IPC
channel.

The desktop process should remain responsible for:

* rendering;
* navigation;
* accessibility;
* local session state.

The backend remains responsible for:

* authoritative validation;
* configuration writes;
* planning;
* execution;
* state.

Toolkit selection should not change architectural boundaries.

---

## 74. Terminal Wizard

A terminal-based Wizard may be useful for:

* headless installations;
* serial-console installations;
* recovery;
* development;
* minimal profiles.

The terminal and graphical Wizards should use the same backend contracts.

They should not implement independent configuration rules.

---

## 75. Interface Independence

The Wizard backend should not depend on one presentation technology.

Possible interfaces include:

```text
graphical desktop
local web application
terminal interface
remote management interface
unattended configuration
```

All interfaces should consume shared schemas, registries, validation, planning,
and execution APIs.

---

## 76. Backend API

The Wizard backend may expose operations such as:

```text
get hardware facts
list profiles
list capabilities
list compatible implementations
list models
validate selections
generate review
generate plan
confirm operation
get operation status
subscribe to progress
cancel operation
export configuration
```

The exact API is not yet defined.

Operations should use versioned structured messages.

---

## 77. API Versioning

Wizard-facing APIs should declare a schema or protocol version.

The Wizard should reject incompatible backend versions gracefully.

Compatibility rules should cover:

* supported request versions;
* supported response versions;
* optional fields;
* deprecated fields;
* event versions;
* error formats.

UI and backend deployment versions may not always change together.

---

## 78. Registry Integration

The Wizard should read available choices from registries.

Registry metadata may provide:

* display name;
* description;
* capability identifier;
* implementation identifier;
* compatibility;
* required resources;
* license;
* estimated size;
* advanced-field schema;
* recommendation metadata.

The Wizard should not duplicate registry content in source code.

---

## 79. Dynamic Choices

Available choices may change based on:

* selected profile;
* hardware facts;
* site policy;
* payload contents;
* previous selections;
* network availability;
* architecture;
* resource conflicts.

Dynamic choices should be recomputed through backend validation or query
interfaces.

The Wizard should not hide a previously selected value without explaining why
it became invalid.

---

## 80. Deterministic Output

Identical user selections, hardware facts, schemas, profiles, registries, and
policy should produce equivalent installation-selection output.

The Wizard should not introduce nondeterminism through:

* random default choices;
* filesystem ordering;
* unstable registry ordering;
* locale-dependent identifiers;
* hidden timing behavior.

Display ordering should also be stable where possible.

---

## 81. Offline Behavior

The Wizard must function without internet access for supported offline
profiles.

Offline behavior requires:

* local schemas;
* local registry metadata;
* local help text;
* local licenses;
* local model metadata;
* local resource availability information;
* no required remote fonts or scripts;
* no silent external calls.

When online-only functionality is unavailable, the Wizard should state that
clearly.

---

## 82. Help Content

Each meaningful choice should have accessible help.

Help may explain:

* what the feature does;
* who it is for;
* hardware requirements;
* disk requirements;
* security implications;
* whether it can be changed later;
* whether it requires network access.

Help content should be versioned with the associated capability or field.

It should not become the only place where critical constraints are documented.

---

## 83. Diagnostics

The Wizard should provide access to safe diagnostics when something fails.

Diagnostics may include:

* Wizard version;
* backend version;
* configuration identifier;
* plan identifier;
* operation identifier;
* failed phase;
* structured error;
* hardware summary;
* payload version;
* log location.

Diagnostics should exclude:

* passwords;
* tokens;
* private keys;
* resolved secrets;
* unnecessary personal information.

---

## 84. Logging

Wizard logs should record:

* session identifier;
* stage transitions;
* validation requests;
* planning requests;
* operation identifiers;
* non-sensitive selections;
* warnings;
* error identifiers;
* final result.

Wizard logs should not become the authoritative record of system execution.

The execution subsystem maintains authoritative operation logs.

---

## 85. Audit Events

Security-relevant Wizard actions may require audit events.

Examples include:

* imported signed configuration;
* policy override attempt;
* destructive storage confirmation;
* license acceptance;
* privileged execution request;
* cancellation request;
* configuration export;
* authentication failure.

Audit events should identify the actor or session where possible.

---

## 86. Metrics

Optional product metrics may measure:

* stage completion;
* validation failure categories;
* abandoned sessions;
* frequently changed defaults;
* installation success;
* common unsupported selections.

Metrics must respect privacy configuration.

They should not include secret or personally identifying content unless
explicitly authorized and required.

The Wizard must work fully without telemetry.

---

## 87. Testing Strategy

Wizard testing should include multiple levels.

### 87.1 State-Machine Tests

Test:

* valid navigation;
* invalid transitions;
* backward navigation;
* confirmation gating;
* cancellation;
* completion;
* failure states.

### 87.2 Field Validation Tests

Test:

* accepted values;
* rejected values;
* boundary values;
* malicious values;
* Unicode input;
* localization differences.

### 87.3 Cross-Field Tests

Test:

* profile and hardware;
* model and memory;
* offline mode and resources;
* storage policy and disk size;
* implementation conflicts;
* policy restrictions.

### 87.4 Backend Contract Tests

Test:

* configuration submission;
* validation response;
* planning response;
* progress events;
* operation failure;
* version mismatch.

### 87.5 UI Tests

Test:

* navigation;
* visible defaults;
* error association;
* warning acknowledgement;
* review accuracy;
* progress rendering;
* completion rendering.

### 87.6 Accessibility Tests

Test:

* keyboard operation;
* focus order;
* screen-reader labels;
* contrast;
* text scaling;
* non-color indicators.

---

## 88. Test Doubles

Wizard testing should use fake backend components.

Test doubles should simulate:

* different hardware;
* valid and invalid profiles;
* planning conflicts;
* low disk space;
* missing payload resources;
* execution progress;
* handler failure;
* verification failure;
* retry availability.

UI testing should not require real package installation or disk modification.

---

## 89. Snapshot Testing

Review screens and exported configuration may be tested through deterministic
snapshots.

Snapshot tests should verify:

* selected values;
* automatic decisions;
* warnings;
* destructive actions;
* storage estimate;
* plan summary.

Snapshot tests should not replace semantic assertions.

UI snapshots must be updated only after intentional review.

---

## 90. Integration Testing

Integration tests should verify the Wizard against real:

* Configuration Manager;
* schemas;
* hardware-fact input;
* registries;
* Planner;
* execution event interface.

Tests should confirm that the Wizard does not rely on mock-only assumptions.

---

## 91. Virtual Machine Testing

End-to-end VM testing should cover:

* supported hardware profiles;
* offline installation;
* network-enabled installation;
* destructive disk confirmation;
* invalid imported configuration;
* planning conflict;
* execution failure;
* retry;
* reboot;
* successful completion.

VM tests should compare the final installed system with the approved review and
plan.

---

## 92. Security Testing

Security testing should include:

* shell-injection attempts;
* path traversal;
* malicious imported configuration;
* forged progress events;
* unauthorized privileged request;
* session fixation;
* secret exposure;
* web-origin attacks where applicable;
* unsafe file export;
* policy bypass attempts.

The Wizard should be treated as a privileged workflow even when its UI process
is unprivileged.

---

## 93. Localization Testing

Localization tests should cover:

* translated strings;
* long labels;
* right-to-left layouts;
* date and number formatting;
* non-ASCII host and user input policy;
* stable machine identifiers;
* error messages;
* review layout.

Configuration output must remain locale-independent.

---

## 94. Initial Implementation Scope

The first Wizard version should remain focused.

Recommended initial scope:

1. guided installation mode;
2. one supported interface;
3. hardware summary;
4. profile selection;
5. capability selection;
6. basic model selection;
7. hostname, locale, and timezone;
8. simple storage policy;
9. configuration validation;
10. plan generation;
11. complete review;
12. explicit confirmation;
13. structured progress display;
14. final result display.

Advanced configuration, remote management, policy administration, and complex
resume behavior can follow later.

---

## 95. Proposed Internal Structure

A possible implementation structure is:

```text
wizard/
├── app/
│   ├── wizard-controller
│   ├── wizard-state
│   ├── navigation
│   └── session-store
│
├── backend/
│   ├── configuration-client
│   ├── hardware-client
│   ├── registry-client
│   ├── planner-client
│   └── execution-client
│
├── screens/
│   ├── welcome
│   ├── hardware
│   ├── profile
│   ├── capabilities
│   ├── models
│   ├── storage
│   ├── identity
│   ├── review
│   ├── progress
│   └── completion
│
├── schemas/
├── localization/
├── assets/
└── tests/
```

The actual structure depends on the selected implementation language and UI
technology.

The architectural separation should remain.

---

## 96. Implementation Sequence

Recommended implementation order:

1. finalize installation-selection schema;
2. define Wizard state machine;
3. define backend error format;
4. define hardware-summary interface;
5. define profile and capability registry queries;
6. implement configuration submission;
7. implement validation display;
8. implement plan-summary interface;
9. implement review generation;
10. implement confirmation contract;
11. implement execution event consumption;
12. implement completion and failure screens;
13. add accessibility support;
14. add session persistence;
15. add import and export.

The Wizard should not be implemented by embedding installer commands into UI
event handlers.

---

## 97. Open Design Questions

The following questions require decisions before implementation:

* Which interface technology will be used initially?
* Will the Wizard run inside the installer, after first boot, or both?
* What component owns Desired State generation?
* How will the Wizard communicate with privileged services?
* What is the installation-selection schema?
* How are dynamic module fields described?
* How will storage partitioning be represented safely?
* Which planning errors can offer automatic resolutions?
* How are model licenses displayed and accepted?
* Which session data is resumable?
* What is the offline help-content format?
* How are progress events transported?
* Can the same Wizard support later system reconfiguration?
* What authentication is required after installation?

These decisions should be recorded through focused design documents or
architecture decisions.

---

## 98. Current Limitations

Because the Wizard is planned, current limitations include:

### 98.1 No Stable UI Contract

The user-interface technology and backend protocol are not yet finalized.

### 98.2 Configuration Dependency

The Wizard depends on a formal configuration schema and Configuration Manager.

### 98.3 Planner Dependency

Accurate review and conflict handling depend on stable Planner output.

### 98.4 Execution Event Dependency

Progress presentation depends on structured execution events.

### 98.5 Storage Design

Storage configuration requires a separate high-risk design.

### 98.6 Privilege Boundary

The privileged backend and authorization model require formal definition.

### 98.7 Registry Metadata

User-facing labels, descriptions, estimates, and licenses require registry
support.

---

## 99. Recommended Near-Term Work

Recommended next steps are:

1. define the installation-selection schema;
2. define the minimal guided workflow;
3. define the Wizard state machine;
4. define profile and capability presentation metadata;
5. define hardware-summary data;
6. define review-summary schema;
7. define structured validation errors;
8. define planning-conflict responses;
9. define progress-event schema;
10. define privileged request boundary;
11. create a non-destructive prototype;
12. test the prototype with fake backend services.

The first prototype should prove the contracts before implementing complete
installation execution.

---

## 100. Wizard Contract

The Wizard accepts:

* configuration schemas;
* profile metadata;
* capability metadata;
* hardware facts;
* site policy constraints;
* validation results;
* planning results;
* execution events;
* user input.

It produces:

* structured installation selections;
* warning acknowledgements;
* destructive-action confirmation;
* approved configuration and plan references;
* execution request;
* user-facing progress and result presentation.

A Wizard installation workflow is valid only when:

1. the environment was checked;
2. user selections were captured;
3. configuration validation passed;
4. planning completed;
5. unresolved conflicts were removed;
6. destructive operations were displayed;
7. the user explicitly confirmed;
8. execution was submitted through the defined interface;
9. progress came from authoritative events;
10. success was shown only after verified completion.

---

## 101. Design Principles

The Wizard architecture follows these principles:

1. the Wizard gathers intent;
2. configuration remains declarative;
3. backend validation is authoritative;
4. the Planner owns dependency and conflict resolution;
5. execution components own system mutation;
6. the Wizard never invokes modules directly;
7. safe defaults are visible;
8. automatic choices are explainable;
9. destructive actions require explicit confirmation;
10. session state is not authoritative system state;
11. progress uses structured events;
12. the UI runs with minimal privilege;
13. all interfaces share the same backend contracts;
14. offline installation remains fully supported;
15. accessibility is a core requirement;
16. success requires authoritative verification.

---

## 102. Summary

The DAIA Installation Wizard is the guided configuration and review interface
for DAIA installation.

Its intended flow is:

```text
user intent
    -> Wizard selections
    -> configuration validation
    -> desired state
    -> planning
    -> review and confirmation
    -> execution request
    -> progress presentation
    -> verified completion
```

The Wizard must not become a graphical shell-script launcher.

Its value comes from providing a safe, understandable interface over stable
DAIA contracts:

* configuration;
* hardware facts;
* registries;
* planning;
* execution;
* verification;
* state.

The immediate architectural priority is to define the installation-selection,
review, error, progress, and privilege-boundary contracts before choosing or
expanding the user-interface implementation.
