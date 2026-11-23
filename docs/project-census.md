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


## November 16, 2026 — follow-up census

**Measured local Git HEAD:** `9193a7df7808f8ad161e4bd4eaf297584abbf24c`  
**Scope:** Local development checkout, including tracked and untracked source/configuration files.  
**Method:** Read-only Step 0580 static approximation; no tests executed.

| Metric | November 16 measurement |
| --- | ---: |
| Git commits (locally available `--all` history) | 825 |
| Cumulative lines added | 177,050 |
| Cumulative lines deleted | 32,734 |
| Cumulative line changes | 209,784 |
| Source/configuration files | 383 |
| Total lines | 142,462 |
| Nonblank lines | 115,455 |
| Approximate functions/methods | 4,950 |
| Python `test_*` definitions | 1,147 |
| Rust test attributes | 1,674 |
| Test-related files | 140 |

**Methodology and comparability:** The Step 0580 script counts a selected set of source/configuration extensions, tracked and untracked non-symlink files, and uses regular-expression approximations for functions and tests. Its function/method pattern differs from the November 15 Step 0366 census; the 4,950 and 5,299 estimates must **not** be interpreted as a measured decline. The test-file count and other static metrics may likewise vary with detection methodology. Git history reflects the local checkout at the measured HEAD, not necessarily the latest GitHub branch; documentation-only GitHub commits made afterward are not included. These figures are not live counters, executed-test results, or proof of build success.

The original November 15 census and its complete measurement report remain preserved above.

## November 17, 2026 — RAG recovery integration checkpoint

**Verified Git HEAD:** `4586d9a73ad39794294f69e9fea9ebd25f4e482c`
**Branch:** `master`
**Scope:** PostgreSQL 17/pgvector, Apache Tika, Open WebUI Slim,
and their guarded recovery dependencies.

### Engineering changes

- Corrected warm RAG recovery authorization sequencing so that
  already-running, registered PostgreSQL, Tika and Open WebUI
  containers are verified together rather than incorrectly rejected
  by a PostgreSQL-only network-membership check.
- Preserved staged authorization for cold recovery.
- Retained fail-closed behavior for unauthorized membership,
  authorization failures and unsupported mixed recovery states.
- Updated isolated Open WebUI test fixtures to account for the
  trusted Ollama gateway prerequisite without weakening production
  gateway validation.
- Confirmed that the recovery helper and systemd service are
  staged into the ISO payload, with dependent service ordering.

### Verification evidence

| Check | Result |
| --- | --- |
| Targeted regressions, Step 0700 | 45 passed |
| Expanded RAG/pgvector suite, Step 0701 | 417 passed, 23 skipped |
| Expanded suite failures/errors | 0 |
| Shell syntax and Git diff checks | Passed |
| Guarded commit and GitHub verification, Step 0702 | Passed |
| Verified Git commit | `4586d9a7` |

The 23 skipped tests require privileged or isolated root fixtures.
These executed test results are distinct from the static test
definition counts in the November 15 and November 16 censuses.

### Physical laptop validation outstanding

The installed laptop previously exhibited a failed RAG recovery
service and inaccessible Open WebUI at `http://localhost:3000`.
The source correction has passed automated tests but has **not**
yet been validated through a newly built and installed ISO.

The next approved ISO validation must check:

1. UEFI and Legacy BIOS boot and the Rust installer.
2. KDE desktop, networking and hardware integration.
3. PostgreSQL 17/pgvector initialization and persistence.
4. Apache Tika availability and document extraction.
5. Warm and cold RAG recovery and trusted network membership.
6. Open WebUI availability at `http://localhost:3000`.
7. Ollama connectivity and local model inference.
8. End-to-end document ingestion, retrieval and recovery.

**Release gate:** Source-level checkpoint verified; physical
integration pending. ISO construction requires separate approval.

### November 17 quantitative measurement — Step 0707

**Measurement recorded at:** `2026-11-18T02:55:01+05:30`

**Measured Git HEAD:** `4586d9a73ad39794294f69e9fea9ebd25f4e482c`

| Metric | November 17 measurement |
| --- | ---: |
| Git commits (locally available `--all` history) | 835 |
| Cumulative lines added | 179,468 |
| Cumulative lines deleted | 32,856 |
| Cumulative line changes | 212,324 |
| Source/configuration files | 384 |
| Total lines | 143,023 |
| Nonblank lines | 115,944 |
| Approximate Python functions | 1,736 |
| Approximate Rust functions | 3,394 |
| Approximate Shell functions | 235 |
| Python `test_*` definitions | 1,170 |
| Rust test attributes | 1,674 |
| Test-related files | 144 |

**Language breakdown:**

| Language/configuration | Files | Lines |
| --- | ---: | ---: |
| Rust | 102 | 73,744 |
| Python | 168 | 29,735 |
| Markdown | 39 | 26,604 |
| Shell | 40 | 12,112 |
| Systemd | 8 | 179 |
| TOML | 14 | 264 |
| YAML | 10 | 157 |
| JSON | 2 | 56 |
| JavaScript | 1 | 172 |

**Methodology:** Read-only Step 0707. Counts include Git-tracked
and nonignored untracked files with selected source/configuration
extensions, excluding symlinks. Physical line counts include blank
lines and comments. Function and test counts use regular-expression
approximations. Historical changes cover locally available Git
`--all` history. No tests were executed during measurement.

**Timestamp interpretation:** The timestamp above records when
these Step 0707 results were entered into the Engineering Census.
It is not the original execution timestamp of Step 0707.

**Comparability:** Direct equivalence with the November 16 Step 0580
measurement has not been established. Differences between dated
measurements must not be interpreted as exact development deltas.

### Census measurement boundary

This milestone records engineering changes and executed tests.
It does not replace or recompute the November 15 or November 16
static codebase measurements. A new quantitative census requires
a separately executed, documented measurement.

## Update policy

Publish a new dated census at major milestones. Preserve older
snapshots rather than silently rewriting historical measurements.

## November 23, 2026 — Pragna documentation checkpoint

This checkpoint records the SearXNG integration and the
subsequent read-only repository inventory.

- Git commit: `cb27b8df`, verified on `origin/master`.
- SearXNG commit: 83 files changed.
- Selected verification: 599 tests passed across 51 suites.
- Documentation inventory: 39 files in the inspection scope.
- Five declared offline artifacts were present locally.
- Six unrelated untracked development files were preserved.

### Offline artifact inventory

| Artifact | Observed size (bytes) |
| --- | ---: |
| `open-webui-v0.13.0-slim.tar` | 624,397,824 |
| `pgvector-pg17.tar` | 450,816,000 |
| `tika-3.3.3.0.tar` | 401,518,080 |
| `searxng-amd64.tar` | 284,349,440 |
| `nomic-embed-text-ollama.tar` | 274,309,120 |

The archive manifests declare checksums. The inventory recorded
presence and size but did not independently recompute every hash.

These are repository and build-workspace observations, not an
installed-laptop software census. Historical census measurements
above remain unchanged.
