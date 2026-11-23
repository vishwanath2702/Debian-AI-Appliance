# Pragna — Engineering Census

This document retains the November 18 and November 23, 2026
engineering census records. Earlier dated census snapshots have
been retired from this document; Git history preserves them.

## November 18, 2026 — Engineering census

The following Step 0707 measurements were recorded on November 18.
The original report labelled them November 17 measurements;
that historical label is retained in the table for accuracy.

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

## Update policy

Publish new dated censuses at major milestones. Preserve the
measurement timestamp, Git revision, scope and methodology.
Older snapshots remain accessible through Git history.

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
installed-laptop software census. The November 18 quantitative
measurement remains a separate historical snapshot.
