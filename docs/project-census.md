# Pragna — Engineering Census

**Snapshot date:** November 15, 2026
**Measured Git HEAD:** `5efaaf713474ac5506baec56c80a25418becc676`
**Scope:** DAIA/Pragna development repository, within Loom.

This is a frozen engineering snapshot, not a live project
dashboard. It includes the original Shell-based architecture,
historical deletions, and subsequent Rust development.

## Key measurements

| Metric | Snapshot |
| --- | ---: |
| Git commits | 818 |
| Cumulative lines added | 174,618 |
| Cumulative lines deleted | 32,663 |
| Cumulative line changes | 207,281 |
| Current source/configuration files | 382 |
| Current total lines | 141,826 |
| Current nonblank lines | 114,862 |
| Approximate functions/methods | 5,299 |
| Python test definitions | 1,124 |
| Rust test attributes | 1,668 |
| Test-related files | 136 |
| Historically deleted paths | 98 |
| Historically deleted Shell paths | 78 |

## Interpretation and limitations

- Git activity counts repeated edits, not unique lines of code.
- Current file counts include untracked source at measurement time.
- Test counts are static code measurements, not executed test results.
- Function/method counts are approximate.
- Available Git history cannot measure work never committed.
- The snapshot does not represent every future Loom appliance.
- The historical baseline is preserved rather than recomputed
  automatically on each README update.

## Complete original census

The following is the original Step 0366 measurement report,
preserved verbatim for reproducibility and auditing.

```text
=== STEP 0366: COMPLETE PRAGNA ENGINEERING CENSUS ===

=== GIT HISTORY ===
Branch: master
HEAD: 5efaaf713474ac5506baec56c80a25418becc676
Local origin/master: 5efaaf713474ac5506baec56c80a25418becc676
Total commits: 818
Root commits: 1
First recorded commit: 2026-07-24T05:56:42+05:30
Latest recorded commit: 2026-11-14T21:56:55+05:30
Calendar span: 113 days

=== HISTORICAL DEVELOPMENT ===
Lines added across all Git history: 174,618
Lines deleted across all Git history: 32,663
Total line-change activity: 207,281
Binary change records: 2
Historical line changes include repeated edits, not unique LOC.
Git --all includes locally available branches and tags.

=== CURRENT CODEBASE (INCLUDING UNTRACKED SOURCE) ===
Files: 382
Lines: 141,826
Nonblank lines: 114,862
Functions/methods (approx.): 5,299
Python classes: 220
Python test_* definitions: 1,124
Rust test attributes: 1,668
  JSON: 2 files, 56 lines
  JavaScript: 1 files, 172 lines
  Markdown: 36 files, 26,128 lines
  Python: 163 files, 28,876 lines
  Rust: 102 files, 73,437 lines
  Shell: 46 files, 12,561 lines
  Systemd: 8 files, 175 lines
  TOML: 14 files, 264 lines
  YAML: 10 files, 157 lines
Test-related files: 136
Systemd units: 8
YAML files: 10

=== EARLIEST COMMIT SNAPSHOT ===
Commit: 3505045c612bb2d9c4fda5ce94d0529e7805e3a7
Files: 154
Lines: 53,059
Nonblank lines: 39,468
Functions/methods (approx.): 1,214
Python classes: 0
Python test_* definitions: 0
Rust test attributes: 0
  Markdown: 32 files, 23,423 lines
  Rust: 9 files, 10 lines
  Shell: 102 files, 29,475 lines
  Systemd: 1 files, 15 lines
  TOML: 10 files, 136 lines

=== DELETED / REPLACED IMPLEMENTATIONS ===
Distinct paths recorded as deleted: 98
Examples of deleted Shell paths:
  build/patch.sh
  installer/files/opt/daia/bootstrap.sh
  installer/files/opt/daia/builder/build-context.sh
  installer/files/opt/daia/builder/build-state.sh
  installer/files/opt/daia/builder/builder-test.sh
  installer/files/opt/daia/builder/builder.sh
  installer/files/opt/daia/builder/logger.sh
  installer/files/opt/daia/builder/plugin-executor.sh
  installer/files/opt/daia/builder/workspace-builder-test.sh
  installer/files/opt/daia/builder/workspace-builder.sh
  installer/files/opt/daia/core/configuration.sh
  installer/files/opt/daia/core/desired-state.sh
  installer/files/opt/daia/core/plugin-registry.sh
  installer/files/opt/daia/firstboot-test.sh
  installer/files/opt/daia/install.sh
  installer/files/opt/daia/lib/common.sh
  installer/files/opt/daia/lib/hardware.sh
  installer/files/opt/daia/lib/hardware/cpu.sh
  installer/files/opt/daia/lib/hardware/memory.sh
  installer/files/opt/daia/lib/logging.sh
  installer/files/opt/daia/lib/module.sh
  installer/files/opt/daia/lib/packages.sh
  installer/files/opt/daia/lib/services.sh
  installer/files/opt/daia/lib/ui.sh
  installer/files/opt/daia/modules/desktop.sh
  installer/files/opt/daia/planner/capability-registry.sh
  installer/files/opt/daia/planner/capability-resolver.sh
  installer/files/opt/daia/planner/capability-synchronizer.sh
  installer/files/opt/daia/planner/conflict-detector.sh
  installer/files/opt/daia/planner/dependency-resolver.sh
  installer/files/opt/daia/planner/execution-plan-builder.sh
  installer/files/opt/daia/planner/planner.sh
  installer/files/opt/daia/planner/profile-reader.sh
  installer/files/opt/daia/planner/registry-adapter.sh
  installer/hooks/late-install.sh
Total historically deleted .sh/.bash paths: 78

=== INTERPRETATION ===
Current LOC represents surviving source and configuration.
Historical additions/deletions capture refactoring and replacement.
Function and test counts are static estimates.
Git history cannot count work never committed.
No tests were executed.

RESULT: READ-ONLY CENSUS COMPLETE
NO SOURCE EDITS
NO ISO BUILD
NO COMMIT OR PUSH
```

## Update policy

Publish a new dated census at major milestones. Preserve older
snapshots rather than silently rewriting historical measurements.
