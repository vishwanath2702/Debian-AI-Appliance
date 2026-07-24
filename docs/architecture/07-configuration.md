# DAIA Configuration Architecture

## 1. Purpose

The DAIA Configuration subsystem defines how settings, installation choices,
profiles, defaults, hardware facts, and generated state are represented and
made available to other DAIA components.

Configuration is the structured input from which DAIA derives desired system
behavior.

It must remain distinct from:

* execution plans;
* current system state;
* transient operation state;
* logs;
* generated artifacts;
* secrets.

The Configuration subsystem provides a predictable and validated contract
between the Installation Wizard, Planner, Builder, installer, bootstrap, Module
Framework, and Runtime Engine.

---

## 2. Architectural Role

Configuration sits near the beginning of the DAIA control flow.

Conceptually:

```text
Built-In Defaults
       +
Distribution Profile
       +
Hardware Facts
       +
User Selections
       +
Site Overrides
       |
       v
Configuration Manager
       |
       v
Validated Effective Configuration
       |
       v
Desired State
       |
       v
Planner
       |
       v
Execution Plan
```

Configuration describes selected behavior and policy.

Desired State describes the system condition DAIA intends to achieve.

The execution plan describes the ordered actions required to reach that state.

---

## 3. Responsibilities

The Configuration subsystem is responsible for:

* defining configuration schemas;
* loading configuration sources;
* validating configuration syntax;
* validating configuration semantics;
* applying documented precedence rules;
* merging compatible configuration layers;
* rejecting invalid or conflicting settings;
* producing effective configuration;
* recording configuration provenance;
* protecting immutable defaults;
* separating secrets from ordinary configuration;
* exposing configuration through stable interfaces;
* supporting schema migration.

The Configuration subsystem answers:

> What settings and choices should DAIA use as input?

---

## 4. Non-Responsibilities

The Configuration subsystem does not:

* execute installation operations;
* install packages;
* start services;
* resolve the complete dependency graph;
* generate execution order;
* verify live system state;
* record chronological logs;
* replace Desired State;
* replace Current State;
* store arbitrary module output;
* infer unsupported values silently.

Configuration may influence these activities, but it does not perform them.

---

## 5. Configuration Categories

DAIA configuration should be divided into distinct categories.

Recommended categories include:

1. built-in defaults;
2. distribution profiles;
3. installation selections;
4. hardware facts;
5. site policy;
6. generated effective configuration;
7. desired-state configuration;
8. runtime overrides;
9. secrets.

These categories have different ownership and mutability rules.

They should not be stored as one undifferentiated file.

---

## 6. Built-In Defaults

Built-in defaults provide safe baseline values.

Examples include:

* default log level;
* default installation profile;
* default state locations;
* default timeout values;
* default module policy;
* default verification behavior;
* default network assumptions;
* default service settings.

Built-in defaults should:

* be version-controlled;
* be shipped with the payload;
* be immutable at runtime;
* contain no machine-specific values;
* contain no secrets;
* use conservative behavior.

A default should be valid without requiring undocumented environment
assumptions.

---

## 7. Distribution Profiles

A distribution profile represents a coherent DAIA edition or deployment class.

Examples may include:

```text
minimal
desktop
workstation
gpu-workstation
offline-complete
development
production
```

A profile may select:

* capabilities;
* modules;
* packages;
* container images;
* AI models;
* desktop components;
* hardware support;
* diagnostics;
* verification policy;
* payload contents.

Profiles should remain declarative.

They should not contain executable shell logic.

---

## 8. Installation Selections

Installation selections are choices made for one target system.

They may originate from:

* the Installation Wizard;
* a preseeded installation profile;
* a command-line tool;
* an automated deployment system;
* a manually prepared configuration file.

Examples include:

* hostname;
* locale;
* timezone;
* storage policy;
* selected AI capabilities;
* desktop selection;
* enabled services;
* preferred implementation;
* model selection;
* network behavior.

Installation selections express user or operator intent.

They should not contain direct commands.

---

## 9. Hardware Facts

Hardware facts describe the detected target environment.

Examples include:

* CPU architecture;
* CPU features;
* memory capacity;
* disk capacity;
* GPU vendor;
* GPU model;
* accelerator availability;
* firmware mode;
* virtualization support;
* network interfaces.

Hardware facts are observed input.

They are not user preferences.

A user override must not silently rewrite an observed fact.

Where an override is allowed, it should be represented as policy applied to the
fact rather than as replacement of the fact itself.

---

## 10. Site Policy

Site policy represents administrator-defined constraints or standards.

Examples include:

* prohibited modules;
* approved package sources;
* mandatory services;
* model licensing restrictions;
* storage limits;
* allowed network endpoints;
* minimum verification level;
* required audit behavior;
* security hardening rules.

Site policy may constrain user choices.

It should have higher authority than ordinary user preference where explicitly
defined.

Conflicts should produce clear validation errors.

---

## 11. Effective Configuration

Effective Configuration is the validated result of combining all applicable
configuration layers.

Conceptually:

```text
defaults
   +
profile
   +
hardware facts
   +
site policy
   +
installation selections
   +
allowed overrides
   |
   v
effective configuration
```

Effective Configuration should be:

* deterministic;
* complete enough for downstream processing;
* schema-valid;
* semantically valid;
* immutable for one planning operation;
* traceable to its source values.

It should not contain unresolved references.

---

## 12. Configuration Precedence

Precedence must be explicit.

A possible initial precedence model is:

```text
lowest authority
    |
    | built-in defaults
    | profile defaults
    | detected hardware facts
    | installation selections
    | site policy
    | explicit administrator override
    v
highest authority
```

This order is illustrative.

Not every value should be overridable by every layer.

For example:

* hardware facts should not be replaced by profile defaults;
* site policy may prohibit a user selection;
* immutable security constraints may reject all overrides;
* secrets should not be sourced from ordinary configuration layers.

Precedence should be defined per field class where necessary.

---

## 13. Merge Semantics

Configuration merging must be predictable.

Scalar values may use replacement semantics.

Example:

```yaml
log_level: info
```

overridden by:

```yaml
log_level: debug
```

Lists require explicit behavior.

Possible list semantics include:

* replace;
* append;
* unique union;
* ordered merge;
* keyed merge;
* prohibited override.

Maps may merge by key, but nested behavior must be defined by schema.

The system should not guess merge behavior from data type alone.

---

## 14. Provenance

Every effective value should be traceable to its source.

A conceptual provenance record may include:

```yaml
value: workstation
source:
  layer: installation-selection
  file: /var/lib/daia/config/installation.yaml
  field: profile
```

Provenance helps explain:

* why a value was selected;
* which source overrode another;
* whether site policy changed a preference;
* whether a default is still active;
* which input caused planning behavior.

Provenance is particularly important for debugging and support.

---

## 15. Configuration Schema

Configuration should be governed by a versioned schema.

A conceptual root structure may resemble:

```yaml
schema_version: 1

system:
  hostname: daia
  locale: en_US.UTF-8
  timezone: Asia/Kolkata

profile:
  id: workstation

capabilities:
  ai_engine: true
  container_runtime: true
  desktop: true

models:
  selected: []

runtime:
  verification_level: standard
  offline_mode: true
```

The final schema should be documented independently and validated
programmatically.

---

## 16. Schema Versioning

Every persisted configuration document should declare its schema version.

Example:

```yaml
schema_version: 1
```

The Configuration Manager should:

* accept supported versions;
* reject unsupported future versions;
* migrate supported older versions explicitly;
* preserve backups before migration;
* record migration results;
* never silently reinterpret incompatible data.

Schema version and DAIA product version are separate concepts.

---

## 17. Syntax Validation

Syntax validation confirms that configuration can be parsed.

It should detect:

* malformed YAML or JSON;
* invalid encoding;
* duplicate keys where prohibited;
* incorrect scalar types;
* invalid indentation;
* truncated files;
* unsupported document structure.

Syntax validation should occur before semantic processing.

A syntactically invalid file should not be partially applied.

---

## 18. Structural Validation

Structural validation confirms that configuration follows the schema.

It should detect:

* missing required fields;
* unknown critical fields;
* incorrect field types;
* invalid enumeration values;
* unsupported nesting;
* malformed identifiers;
* invalid list elements.

Unknown non-critical extension fields may be permitted only through a documented
extension mechanism.

---

## 19. Semantic Validation

Semantic validation checks relationships and meaning.

Examples include:

* a GPU-only module selected without supported GPU hardware;
* an offline profile requiring an unavailable remote resource;
* conflicting AI engine implementations;
* model selection exceeding disk capacity;
* desktop enabled in a headless-only profile;
* prohibited package source selected;
* invalid storage layout;
* unsupported architecture and profile combination.

Semantic validation may require registry and hardware information.

---

## 20. Configuration Manager

The Configuration Manager is the component responsible for loading, merging,
validating, and exposing DAIA configuration.

Its expected responsibilities include:

* discover configuration sources;
* load supported formats;
* validate each source;
* apply precedence;
* merge values;
* validate the effective result;
* record provenance;
* publish an immutable effective configuration;
* expose read interfaces;
* support schema migration.

The Configuration Manager should not execute arbitrary code from configuration.

---

## 21. Proposed Internal Components

A possible structure is:

```text
config/
├── defaults/
│   ├── base.yaml
│   └── runtime.yaml
│
├── profiles/
│   ├── minimal.yaml
│   ├── desktop.yaml
│   └── workstation.yaml
│
├── schemas/
│   ├── configuration-v1.json
│   └── profile-v1.json
│
└── manager/
    ├── config-loader.sh
    ├── config-validator.sh
    ├── config-merger.sh
    ├── config-provenance.sh
    └── config-migrate.sh
```

This structure is illustrative.

The final implementation should follow established repository conventions.

---

## 22. Source Discovery

Configuration source discovery should be explicit.

Possible source locations include:

```text
/opt/daia/config/defaults/
/opt/daia/config/profiles/
/etc/daia/
/var/lib/daia/config/
```

Recommended ownership:

```text
/opt/daia/config/
    immutable shipped defaults and schemas

/etc/daia/
    administrator-managed persistent configuration

/var/lib/daia/config/
    DAIA-generated installation and effective configuration

/run/daia/
    transient runtime overrides
```

The exact paths should be finalized by the state and filesystem layout design.

---

## 23. Source Loading Order

A deterministic source-loading order is required.

Example:

```text
1. built-in defaults
2. selected profile
3. detected hardware facts
4. installation selections
5. site policy
6. approved runtime override
```

Files within a layer should not be loaded according to accidental filesystem
order.

Use:

* an explicit manifest;
* sorted stable filenames;
* registry order;
* declared priorities.

Duplicate ownership within one layer should be rejected unless explicitly
supported.

---

## 24. Immutable Configuration Snapshot

Planning should use an immutable configuration snapshot.

Once planning begins:

* source files may continue to change;
* the active planning operation should retain its original values;
* the Planner should not observe a partially changed configuration;
* the snapshot should receive a stable identifier.

Conceptually:

```text
configuration sources
        |
        v
validated snapshot
        |
        v
snapshot identifier
        |
        v
Planner
```

A later configuration change should create a new snapshot and planning cycle.

---

## 25. Configuration Identifier

Each effective configuration should have a stable identifier.

The identifier may be based on:

* generated UUID;
* content hash;
* timestamp plus sequence;
* versioned state identifier.

A content hash is useful for proving equivalence.

Example metadata:

```yaml
configuration_id: config-20260723-001
content_digest: sha256:...
schema_version: 1
created_at: 2026-07-23T05:00:00Z
```

The execution plan should reference the configuration or desired-state
identifier from which it was produced.

---

## 26. Desired State Relationship

Configuration and Desired State are related but not identical.

Configuration may contain:

* preferences;
* profile choice;
* policy;
* hardware input;
* installation values.

Desired State should contain normalized target conditions.

Example:

```yaml
configuration:
  capabilities:
    container_runtime: true
```

may become:

```yaml
desired_state:
  capabilities:
    container-runtime:
      state: present
      implementation: docker
```

The Desired State Manager performs this normalization or coordinates it with
the Planner and registry.

---

## 27. Current State Relationship

Current State represents observed or verified system condition.

It must not be merged into effective configuration.

Current State may influence planning, but it remains a separate input.

Conceptually:

```text
effective configuration
        |
        v
desired state
        |
        +-------------------+
                            |
current state --------------+
                            |
                            v
                         Planner
```

Combining desired and observed state in one mutable document would create
ambiguous ownership.

---

## 28. Execution Plan Relationship

The execution plan is derived from configuration and state.

It should reference:

* configuration identifier;
* desired-state identifier;
* current-state snapshot identifier;
* registry version;
* Planner version.

The execution plan should not depend on reading mutable configuration files
during execution.

All required execution decisions should already be represented in the plan or
its immutable referenced data.

---

## 29. Wizard Relationship

The Installation Wizard is a configuration authoring interface.

Its responsibilities include:

* presenting valid choices;
* gathering user intent;
* displaying detected hardware;
* validating interactive input;
* writing installation selections;
* showing a review summary.

The Wizard should not write directly into multiple internal state files.

It should submit values through the Configuration Manager or produce one
validated input document.

---

## 30. Installer Relationship

The installer consumes a limited subset of configuration required during
installation.

Examples include:

* installation profile;
* target hostname;
* locale;
* timezone;
* storage policy;
* offline resource selection;
* first-boot behavior.

The Debian installer environment should receive only the values it needs.

Complex planning and module configuration should preferably occur in the
installed environment.

---

## 31. Bootstrap Relationship

Bootstrap uses validated configuration to determine which prepared module
operations should run during first boot.

Bootstrap should not:

* parse multiple conflicting source files independently;
* reimplement precedence;
* invent missing defaults;
* silently ignore invalid values.

It should consume an effective configuration or generated installation plan.

---

## 32. Module Configuration

Each module should receive a scoped configuration view.

A container-runtime module should not need unrestricted access to unrelated AI
model or desktop configuration.

Scoped configuration improves:

* security;
* testability;
* interface stability;
* ownership clarity;
* validation.

A module configuration section may resemble:

```yaml
modules:
  container-runtime:
    enabled: true
    implementation: docker
    storage_driver: overlay2
```

The exact structure should be derived from the module metadata and desired-state
schema.

---

## 33. Runtime Override

Runtime overrides may be useful for temporary behavior.

Examples include:

* temporary log-level increase;
* temporary network disablement;
* maintenance mode;
* test endpoint selection;
* one-operation timeout adjustment.

Runtime overrides should:

* be explicitly authorized;
* have narrow scope;
* have clear expiration;
* be recorded in audit state;
* not mutate immutable source configuration;
* not bypass security policy.

A transient override should not silently become permanent configuration.

---

## 34. Environment Variables

Environment variables may be used for limited process-specific overrides.

They should not become the primary configuration system.

Environment variable use should be restricted to documented values such as:

```text
DAIA_CONFIG_PATH
DAIA_LOG_LEVEL
DAIA_OPERATION_ID
DAIA_STATE_DIR
```

Configuration values passed through the environment should:

* be validated;
* be safely quoted;
* avoid secrets where possible;
* not be evaluated as shell code.

Environment precedence must be documented.

---

## 35. Command-Line Overrides

Command-line overrides may be useful for administrative tools.

Example:

```bash
daia-config validate --config /path/to/config.yaml
```

or:

```bash
daia-runtime apply --log-level debug
```

Command-line values should normally affect one invocation.

They should not modify persistent configuration unless an explicit write action
is requested.

Administrative tools should display the resulting effective value and source.

---

## 36. Secrets

Secrets must remain separate from ordinary configuration.

Examples include:

* API tokens;
* private keys;
* registry credentials;
* passwords;
* enrollment secrets.

Secrets should not be stored in:

* source-controlled defaults;
* profile files;
* payload manifests;
* execution logs;
* command-line arguments;
* ordinary effective-configuration exports.

Configuration may contain a secret reference.

Example:

```yaml
registry:
  credential_ref: secret://container-registry/main
```

A separate Secret Manager should resolve the reference when needed.

---

## 37. Secret References

Secret references should be opaque to components that do not need the secret.

The Planner may need to know that credentials are required.

It should not need the credential value.

The Runtime Engine or handler may resolve the value at execution time through a
controlled interface.

Secret resolution should be:

* auditable;
* scoped;
* temporary;
* redacted from logs;
* unavailable to unrelated modules.

---

## 38. Configuration File Permissions

Persistent configuration permissions should be explicit.

General policy:

* immutable defaults: readable, not runtime-writable;
* administrator configuration: writable only by authorized administrators;
* generated effective configuration: writable only by DAIA;
* hardware facts: writable only by detection components;
* secret references: protected according to sensitivity;
* resolved secrets: never stored with ordinary configuration.

World-writable configuration must be rejected.

---

## 39. Atomic Writes

Configuration writes should be atomic.

Recommended pattern:

```text
write temporary file
      |
      v
validate temporary file
      |
      v
flush and set permissions
      |
      v
atomic rename
```

This prevents readers from observing partially written configuration.

The previous valid version should be retained when the replacement fails
validation.

---

## 40. Configuration History

Important persistent configuration changes should be recorded.

A history entry may include:

* configuration identifier;
* parent identifier;
* actor;
* source;
* timestamp;
* changed fields;
* validation result;
* reason;
* content digest.

History supports:

* troubleshooting;
* auditing;
* rollback;
* comparison;
* support diagnostics.

Sensitive values should not appear in change summaries.

---

## 41. Configuration Rollback

Configuration rollback should create a new active configuration based on a
previous valid version.

It should not erase history or pretend the intervening changes never occurred.

Rollback must still pass current validation.

A previously valid configuration may become invalid because:

* hardware changed;
* modules were removed;
* schema support changed;
* site policy changed;
* resources are no longer available.

Rollback therefore requires revalidation and new planning.

---

## 42. Configuration Migration

When schemas evolve, the Configuration Manager may migrate persisted
configuration.

Migration should:

1. identify source schema version;
2. verify migration support;
3. create a backup;
4. transform values;
5. validate the result;
6. record provenance;
7. write atomically;
8. preserve the original on failure.

Migration should never silently discard unsupported values.

A migration report should identify changed, renamed, defaulted, or rejected
fields.

---

## 43. Unknown Fields

Unknown fields should be handled according to policy.

Recommended behavior:

* unknown top-level fields: reject;
* unknown security-critical fields: reject;
* unknown module fields: reject unless extension namespace is declared;
* unknown future optional metadata: preserve only when safe;
* deprecated fields: warn or migrate.

Silently ignoring misspelled fields can produce dangerous unintended defaults.

---

## 44. Deprecated Fields

Schema definitions should mark deprecated fields.

Deprecation handling may include:

* warning;
* automatic migration;
* supported-until version;
* replacement field;
* hard rejection after removal.

Example:

```yaml
deprecated:
  old_field:
    replacement: new_field
    remove_after: 2
```

Users should receive actionable migration information.

---

## 45. Configuration Extensions

Plugins or modules may require extension fields.

Extensions should use namespaced keys.

Example:

```yaml
extensions:
  daia.example.vendor:
    feature_enabled: true
```

An extension must declare:

* owning component;
* schema;
* version;
* validation rules;
* whether unknown fields are allowed;
* whether it affects planning or execution.

Extensions should not bypass the primary schema.

---

## 46. Human-Readable and Machine-Readable Forms

Configuration should be machine-readable and reasonably human-readable.

YAML may be suitable for maintained configuration because of readability.

JSON may be suitable for generated snapshots and API exchange.

The architecture should define a canonical internal representation regardless of
input format.

Equivalent inputs should produce equivalent normalized configuration.

---

## 47. Normalization

Normalization converts accepted input variations into one canonical form.

Examples include:

* standardizing identifiers;
* expanding shorthand values;
* normalizing booleans;
* canonicalizing paths;
* resolving aliases;
* sorting unordered sets;
* converting sizes into bytes;
* converting durations into one unit.

Normalized output improves deterministic planning and comparison.

Normalization must occur after parsing and before final semantic validation.

---

## 48. Paths

Configuration paths should be validated carefully.

Validation should consider:

* absolute versus relative path;
* allowed root;
* path traversal;
* symbolic links;
* expected file type;
* ownership;
* writability;
* whether the path exists at validation or execution time.

Configuration should not permit arbitrary target paths for privileged writes
without policy restrictions.

---

## 49. Units

Values representing size, duration, percentage, or rate should use explicit
units.

Examples:

```yaml
disk_reserve: 20GiB
operation_timeout: 15m
model_cache_limit: 80%
```

The parser should normalize these values and reject ambiguous formats.

Bare values should not be interpreted differently by different components.

---

## 50. Enumerations

Known choices should use stable machine identifiers.

Prefer:

```yaml
profile: gpu-workstation
```

rather than:

```yaml
profile: GPU Workstation Edition
```

Human-readable labels belong in UI metadata.

Machine identifiers should remain stable even if display names change.

---

## 51. Boolean Values

Boolean settings should accept only clearly defined values.

Preferred canonical values:

```yaml
enabled: true
offline_mode: false
```

Ambiguous strings such as:

```text
yes
on
enabled
1
```

should either be normalized consistently or rejected according to the parser
contract.

The same input must not be interpreted differently by different DAIA
components.

---

## 52. Null and Unset Values

The architecture should distinguish:

* field absent;
* field explicitly null;
* field set to empty string;
* field set to empty list;
* field set to default value.

These conditions may have different meanings.

Example:

* absent: inherit lower-precedence value;
* null: intentionally clear an optional value;
* empty list: explicitly select no entries;
* default: select schema-defined value.

Merge semantics must define these cases.

---

## 53. Lists

Lists should specify whether order is meaningful.

Examples:

* ordered boot operations: order matters;
* selected capabilities: order may not matter;
* search paths: order matters;
* prohibited modules: order does not matter.

Where order does not matter, normalized configuration should sort entries
deterministically.

Duplicate entries should normally be rejected or removed predictably.

---

## 54. Configuration Validation Errors

Validation errors should be structured.

A useful error should include:

* source file;
* field path;
* error category;
* invalid value where safe;
* expected type or values;
* source layer;
* remediation guidance.

Example:

```text
Configuration error:
  source: installation.yaml
  field: capabilities.ai_engine
  value: "maybe"
  expected: boolean
```

Errors should avoid exposing secrets.

---

## 55. Multiple Validation Errors

The validator should report multiple independent errors in one run where
possible.

This improves the editing experience.

However, validation should stop when continued processing would be unsafe, such
as:

* unreadable file;
* unsupported schema;
* malformed document root;
* invalid signature;
* corrupted encoded content.

Error ordering should be deterministic.

---

## 56. Warnings

Warnings represent valid but potentially undesirable configuration.

Examples include:

* deprecated field;
* low disk reserve;
* disabled verification;
* mutable container tag;
* optional capability unavailable;
* runtime override with expiration.

Warnings should not be used for conditions that make the configuration unsafe
or ambiguous.

Those conditions should be errors.

---

## 57. Configuration Signing

Future deployments may require signed configuration.

Signed configuration may be useful for:

* enterprise policy;
* remote provisioning;
* unattended deployment;
* appliance fleet management.

A signed document should bind:

* content;
* schema version;
* issuer;
* creation time;
* optional target identity;
* expiration;
* signature algorithm.

Signature verification should occur before privileged execution relies on the
configuration.

---

## 58. Trust Model

Configuration sources should have explicit trust levels.

Examples:

```text
shipped defaults       trusted distribution content
site policy            trusted administrator content
Wizard selections      authenticated local user input
hardware facts         trusted detector output
runtime override       authorized operation input
remote profile         signed external content
```

Trust level may influence:

* allowed fields;
* override authority;
* required signatures;
* audit requirements;
* whether secrets may be referenced.

---

## 59. Offline Operation

DAIA configuration should be resolvable without network access for offline
installation profiles.

Configuration must not depend on retrieving undefined remote defaults during
installation.

Remote resources should be:

* represented explicitly;
* included in the payload when offline operation is required;
* checksum-pinned;
* validated during planning or build;
* reported as unavailable before execution.

---

## 60. Configuration and Payload Profiles

Payload profiles and runtime configuration are related but distinct.

Payload profile determines what resources are available on the installation
media.

Runtime configuration determines what should be selected or enabled on the
installed system.

A configuration must not request an offline resource absent from the selected
payload unless online retrieval is explicitly permitted.

The Planner should validate this relationship.

---

## 61. Configuration and Registry

The Configuration Manager validates syntax and general semantics.

The Registry provides authoritative information about:

* capabilities;
* implementations;
* modules;
* versions;
* conflicts;
* supported hardware;
* payload availability.

Registry-aware validation may confirm that identifiers referenced by
configuration actually exist.

The Configuration Manager should not duplicate the Registry's entire content.

---

## 62. Configuration and Planner

The Planner consumes normalized effective configuration or derived Desired
State.

The Planner should not:

* apply configuration precedence;
* read unrelated source files;
* infer user intent from comments or filenames;
* repair invalid configuration.

The Planner may produce planning errors when a valid configuration cannot be
satisfied.

Configuration validity does not guarantee plan satisfiability.

---

## 63. Configuration and Builder

The Builder may consume build-specific configuration such as:

* target architecture;
* payload profile;
* output paths;
* cache policy;
* artifact version;
* reproducibility settings.

Build configuration should be separated from appliance desired state where
their ownership differs.

The Builder should receive a validated build context rather than parse arbitrary
global files.

---

## 64. Configuration and Runtime Engine

The Runtime Engine should execute an immutable plan.

It may also consume runtime policy such as:

* operation timeout;
* retry policy;
* verification strictness;
* concurrency limit;
* maintenance window;
* cancellation policy.

Runtime policy should be captured with the operation.

It should not change unpredictably halfway through execution.

---

## 65. Configuration and Verifier

Configuration may define verification policy.

Examples include:

* required verification level;
* allowed timeout;
* health endpoint;
* acceptable version range;
* optional versus mandatory checks.

The Verifier should receive normalized expected-state data.

It should not parse broad user configuration independently.

---

## 66. Configuration and State Manager

Persistent configuration should be stored through the State Manager or an
equivalent controlled persistence interface.

Required capabilities include:

* atomic writes;
* versioning;
* locking;
* history;
* schema validation;
* corruption detection;
* migration;
* permission enforcement.

Individual modules should not invent separate untracked persistent
configuration stores.

---

## 67. Configuration and Logs

Configuration and logs have different purposes.

Configuration describes selected values.

Logs record events.

Logs may record:

* configuration identifier;
* profile;
* changed field paths;
* validation status;
* source names.

Logs should not record:

* complete secret values;
* private keys;
* passwords;
* full sensitive configuration documents.

---

## 68. Configuration Inspection

DAIA should eventually provide a command for inspecting effective
configuration.

A conceptual interface may include:

```bash
daia-config show
daia-config show --source
daia-config validate /path/to/config.yaml
daia-config diff CONFIG_A CONFIG_B
daia-config history
```

Useful output should show:

* effective value;
* source layer;
* source file;
* default status;
* overridden values;
* validation status.

The exact command interface is not yet defined.

---

## 69. Configuration Editing

A future configuration tool may support controlled edits.

It should:

1. load current configuration;
2. apply requested change;
3. validate the full result;
4. display the effective difference;
5. write atomically;
6. record history;
7. trigger reconciliation where appropriate.

Editing one field should not rewrite unrelated values unnecessarily.

Generated files should identify that they are managed by DAIA.

---

## 70. Declarative Configuration

Configuration should describe desired choices rather than procedural commands.

Prefer:

```yaml
services:
  ai-api:
    enabled: true
```

over:

```yaml
commands:
  - systemctl enable ai-api
  - systemctl start ai-api
```

Declarative configuration allows the Planner and Runtime Engine to reason about
state, dependencies, idempotency, verification, and reconciliation.

---

## 71. No Shell Evaluation

Configuration must never be treated as trusted shell source.

Avoid patterns such as:

```bash
source "$CONFIG_FILE"
eval "$CONFIG_VALUE"
```

for user-controlled structured configuration.

Configuration should be parsed using a data parser and validated against a
schema.

Where legacy shell configuration exists, it should be tightly controlled and
migrated toward a non-executable format.

---

## 72. Legacy Shell Configuration

If DAIA currently uses shell-based configuration, it should be classified as
transitional.

Risks include:

* arbitrary command execution;
* global variable mutation;
* quoting errors;
* hidden dependencies;
* difficulty validating types;
* weak schema support.

Migration strategy:

1. inventory current variables;
2. classify ownership and type;
3. define the structured schema;
4. create a compatibility loader;
5. generate normalized configuration;
6. update consumers;
7. deprecate executable configuration;
8. remove the compatibility layer.

---

## 73. Environment Detection

Environment detection should produce facts, not configuration decisions.

A detector may output:

```yaml
hardware:
  architecture: amd64
  memory_bytes: 34359738368
  gpu:
    vendor: nvidia
    model: RTX-4090
```

The Planner or policy layer determines what those facts imply.

This keeps detection separate from selection.

---

## 74. User Intent

User intent should remain high-level where possible.

Examples:

```yaml
intent:
  use_case: local-ai-workstation
  offline_required: true
  desktop_required: true
  model_size_preference: medium
```

The Planner and registries translate intent into implementations.

The user should not need to understand internal package or service names unless
using an advanced configuration mode.

---

## 75. Advanced Configuration

Advanced configuration may expose implementation-specific settings.

It should be clearly separated from ordinary user intent.

Examples include:

* container storage driver;
* model runtime flags;
* service port;
* GPU memory policy;
* package pinning.

Advanced values should be validated by the owning module or registry schema.

They should not bypass compatibility and security rules.

---

## 76. Configuration Modes

DAIA may support several configuration modes:

```text
guided
profile-based
advanced
unattended
managed
```

Guided mode may be produced by the Wizard.

Profile-based mode selects a predefined configuration.

Advanced mode allows scoped implementation settings.

Unattended mode uses a complete validated document.

Managed mode may use signed site policy.

All modes should produce the same normalized internal configuration model.

---

## 77. Review Representation

Before installation or reconciliation, DAIA should be able to present a review
summary.

The summary should show:

* selected profile;
* major capabilities;
* selected implementations;
* hardware-specific choices;
* required resources;
* expected disk use;
* network requirements;
* policy constraints;
* warnings.

The review should derive from validated configuration and planning output.

It should not be independently reconstructed by the Wizard.

---

## 78. Testing Strategy

Configuration testing should cover:

### 78.1 Parser Tests

* valid documents;
* malformed syntax;
* duplicate keys;
* unsupported encoding;
* empty documents.

### 78.2 Schema Tests

* required fields;
* field types;
* enumerations;
* unknown fields;
* nested structures;
* schema versions.

### 78.3 Merge Tests

* scalar override;
* list replacement;
* list union;
* map merge;
* null behavior;
* missing-field inheritance;
* prohibited override.

### 78.4 Precedence Tests

* defaults versus profile;
* profile versus user selection;
* user selection versus site policy;
* runtime override scope;
* hardware fact protection.

### 78.5 Semantic Tests

* unsupported hardware;
* conflicting capabilities;
* invalid profile combinations;
* unavailable offline resources;
* insufficient disk space.

### 78.6 Provenance Tests

* winning source recorded;
* overridden values traceable;
* generated defaults traceable;
* deterministic output.

### 78.7 Migration Tests

* supported old versions;
* unsupported versions;
* preserved values;
* migration failure;
* backup behavior.

---

## 79. Reproducibility Tests

Given identical:

* source configuration;
* hardware facts;
* registry;
* profile;
* schema version;

the Configuration Manager should produce identical normalized effective
configuration.

Ordering should be deterministic.

Comments and formatting differences should not alter normalized meaning.

The content digest should remain stable for equivalent configuration.

---

## 80. Security Tests

Security testing should include:

* attempted shell injection;
* unsafe path values;
* symbolic-link replacement;
* world-writable configuration;
* secret leakage into logs;
* prohibited override;
* malicious extension field;
* invalid signature;
* configuration file race;
* unexpected environment variable.

Configuration should be treated as untrusted input until validated.

---

## 81. Current Limitations

Known configuration architecture limitations may include:

### 81.1 Informal Schema

Current configuration values may not yet be governed by one formal schema.

### 81.2 Shell-Based Values

Some configuration may still use executable shell syntax.

### 81.3 Precedence Rules

The exact precedence between all current sources may not yet be documented.

### 81.4 State Separation

Configuration, desired state, and runtime state may still overlap in some
locations.

### 81.5 Provenance

Current effective values may not retain complete source provenance.

### 81.6 Migration Framework

Schema migration requires formal implementation.

### 81.7 Secret Handling

Secret references and resolution require a separate contract.

---

## 82. Recommended Near-Term Work

Recommended next steps are:

1. inventory all current configuration files and variables;
2. classify each value by owner and category;
3. define the root configuration schema;
4. define profile schema;
5. define hardware-fact schema;
6. document precedence rules;
7. implement a read-only configuration loader;
8. implement schema validation;
9. implement deterministic normalization;
10. implement provenance;
11. create effective-configuration snapshots;
12. migrate consumers away from direct shell sourcing;
13. separate configuration from desired and current state;
14. add behavioural tests.

---

## 83. Proposed Initial Scope

The first Configuration Manager implementation should remain limited.

Recommended initial features:

* YAML or JSON input;
* one schema version;
* built-in defaults;
* one selected profile;
* one installation-selection document;
* one hardware-fact document;
* deterministic merge;
* structural validation;
* basic semantic validation;
* normalized output;
* content digest;
* provenance report.

Secrets, remote policy, signed configuration, and complex extension schemas can
follow later.

---

## 84. Configuration Contract

The Configuration Manager accepts:

* built-in defaults;
* selected profile;
* installation selections;
* hardware facts;
* site policy where supported;
* approved runtime overrides;
* supported schema definitions.

It produces:

* validated normalized effective configuration;
* configuration identifier;
* content digest;
* provenance record;
* warnings;
* explicit validation status.

Successful configuration processing means:

1. all source documents were parsed;
2. all supported schemas were validated;
3. precedence was applied deterministically;
4. merge behavior was valid;
5. semantic checks passed;
6. provenance was recorded;
7. normalized output was published atomically;
8. downstream components can consume one immutable snapshot.

---

## 85. Design Principles

The Configuration architecture follows these principles:

1. configuration is declarative;
2. configuration is data, not executable code;
3. defaults, policy, facts, and user intent remain distinct;
4. precedence is explicit;
5. merge semantics are schema-defined;
6. effective configuration is immutable per operation;
7. every effective value has provenance;
8. schemas are versioned;
9. invalid configuration fails before execution;
10. secrets remain separate;
11. writes are atomic;
12. history is preserved;
13. configuration and state remain separate;
14. all input modes normalize to one internal model;
15. downstream components consume validated snapshots.

---

## 86. Summary

The DAIA Configuration subsystem transforms distributed settings and input into
one validated, normalized, traceable configuration snapshot.

Its intended flow is:

```text
defaults
   + profiles
   + hardware facts
   + user selections
   + site policy
   + approved overrides
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
```

The subsystem must prevent configuration logic from being duplicated across the
Wizard, installer, bootstrap, modules, Planner, and Runtime Engine.

Its immediate architectural priority is to formalize the current configuration
surface through:

* schemas;
* deterministic precedence;
* normalization;
* provenance;
* atomic snapshots;
* separation from desired and current state.

Once this contract exists, the Installation Wizard can become a clean
configuration-authoring interface rather than an execution component.
