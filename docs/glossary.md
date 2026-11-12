# Loom & DAIA — Technical Glossary

**Version:** 0.3.2  
**Purpose:** Explain technical terms used in Loom and DAIA development in everyday language.

This glossary is intended for developers, contributors, and anyone following the project without needing to know every technical term in advance.

## Guarded Edits — Highest-Priority Development Practice

**Guarded edits are our default safety procedure for changing Loom and DAIA.** The goal is to make changes predictable, easy to verify, and safe to stop. The user should not need to debug code or command syntax to follow the development process.

| Term | Plain-English meaning | DAIA example |
|---|---|---|
| **Guarded edit** | A change that proceeds only after verifying that the expected file and code are present. | Check the exact function and expected text before replacing it. |
| **Read-only inspection** | Looking at files and system state without changing anything. | Inspect a Rust module and Git status before editing. |
| **Precondition** | A requirement that must be true before an action can run. | Confirm the file contains exactly one expected section. |
| **Fail safely / guard stop** | Stop without making a change when an assumption or check fails. | If the expected code is missing or duplicated, report the mismatch instead of guessing. |
| **Scoped change** | An edit restricted to the files and behavior agreed upon. | Update only the relevant hardware manifest and its focused tests. |
| **Atomic change** | A change applied as one complete operation, avoiding a half-written result. | Write a replacement file safely, then move it into place only after validation. |
| **Idempotent edit** | An edit that does not repeat or duplicate its changes when run again. | Re-running a patch does not add a second configuration block. |
| **Targeted test** | A test focused on the behavior that changed. | Test USB hot-plug handling after editing the device integration. |
| **Verification report** | A saved record of what was checked, changed, and tested. | Save the result as a numbered DAIA step report. |
| **Approval gate** | A point where work must wait for explicit permission before proceeding. | Do not build an ISO, start services, or push a checkpoint without approval. |
| **Rollback plan** | A known way to restore the previous state if a change fails. | Preserve the original content or use the recorded Git version to recover. |

### Our guarded-edit workflow

1. **Establish the baseline.** Check the relevant Git commit, working-tree changes, and exact files involved.
2. **Inspect only what is needed.** Prefer focused read-only inspection rather than repeatedly scanning the entire repository.
3. **Check preconditions.** Confirm the expected code, file paths, and assumptions before writing.
4. **Make one meaningful, scoped change.** Use a guarded Python edit where practical; avoid manual syntax-heavy editing.
5. **Test and verify.** Run focused checks for the changed behavior and inspect the resulting diff.
6. **Save a concise report.** Record the outcome and any remaining issues.
7. **Stop on unexpected results.** Do not silently improvise a replacement edit or continue after a failed guard.
8. **Respect approval gates.** Do not commit, push, build an ISO, start services, or perform destructive operations unless specifically approved.

**Priority rule:** Correctness and preservation of the working project take precedence over speed. A failed guard is a successful safety check, not a reason to bypass it.

### Copy-and-paste example: guarded hardware diagnostics and CPU benchmark

**Runnable GitHub version:** [Guarded hardware benchmark script](examples/guarded-hardware-benchmark.sh). This preserves the command used for the Pragna laptop baseline, with the OpenSSL timeout increased to allow all block-size tests to finish and clearer timeout/failure reporting.


This is a **guarded diagnostic command**, not a source-code edit. It demonstrates the same discipline: check prerequisites, limit what commands can do, avoid configuration changes, show the results, and save a report. It works on many Linux machines with common tools installed; missing tools are reported rather than installed.

**Copy the whole block into a Bash terminal.** The CPU test measures SHA-256 throughput; the other sections identify hardware rather than measure its speed.

```bash
(
set -u
set -o pipefail
umask 077
REPORT="$(mktemp /tmp/daia-hardware-benchmark-XXXXXXXX.txt)" || exit 1
section() { printf '\n========== %s ==========\n' "$1"; }
check() {
    local label="$1"
    shift
    printf '\n--- %s ---\n' "$label"
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'Tool unavailable: %s\n' "$1"
    elif ! command -v timeout >/dev/null 2>&1; then
        printf 'GUARD STOP: timeout utility unavailable\n'
    else
        timeout 15 "$@" 2>&1 || printf '[Check failed or timed out]\n'
    fi
}
{
    section "DAIA HARDWARE REPORT"
    printf 'Date: %s\nKernel: %s\n' "$(date -Is)" "$(uname -sr)"
    section "CPU"; check "Processor" lscpu
    section "MEMORY"; check "RAM" free -h
    section "STORAGE"; check "Disks" lsblk -o NAME,SIZE,TYPE,MODEL,TRAN,ROTA
    section "GPU"; check "Graphics devices" lspci -nn
    section "NETWORK"; check "Interfaces" ip -brief link
    check "NetworkManager" nmcli device status
    section "BLUETOOTH"; check "Controllers" bluetoothctl list
    section "USB AND SDR"; check "USB devices" lsusb
    section "CPU BENCHMARK"
    if command -v openssl >/dev/null 2>&1 &&
       command -v timeout >/dev/null 2>&1; then
        timeout 10 openssl speed -seconds 3 sha256 2>&1 ||
            printf '[Benchmark failed or timed out]\n'
    else
        printf 'Benchmark skipped: openssl or timeout unavailable\n'
    fi
    section "SAFETY"
    printf 'No package installs, driver changes, service restarts, or disk-write benchmark.\n'
    section "COMPLETE"
} 2>&1 | tee "$REPORT"
printf '\nReport saved to: %s\n' "$REPORT"
)
```

**How to read the guards:**

- `umask 077` and `mktemp` create a new report with restricted access and a unique name; an existing report is not overwritten.
- `command -v` checks whether a tool exists before attempting to run it.
- `timeout` limits external checks so an unavailable or stuck tool does not run indefinitely.
- The script **does not** use `sudo`, install packages, change configuration, or restart services. It does create a report under `/tmp`.
- `tee` displays results in the terminal while writing the report.

**What success looks like:** The terminal displays the sections, OpenSSL prints a CPU throughput table if available, and the final line gives the report path. **What failure looks like:** An unavailable or timed-out command is reported and the other checks continue. If the initial report file cannot be created, the command stops immediately.

**Important limits:** This is a lightweight CPU microbenchmark plus hardware inventory, not a comprehensive performance or device-readiness test. A listed SDR, Wi-Fi, or Bluetooth device is not necessarily functional. The report may include machine and device identifiers; review it before sharing. If the user interrupts the pipeline or the report filesystem fills, the displayed output and saved report may be incomplete.

**How this relates to a guarded edit:** The hardware command *observes* the system and writes only a report. A guarded **edit** adds stricter checks that the expected source code matches, changes only the agreed files, verifies the diff and tests, and stops or rolls back safely on unexpected results. Both use the same **check → act within limits → verify → report** principle.


## Testing and Verification

| Term | Plain-English meaning | DAIA example |
|---|---|---|
| **Regression** | A problem that returns, or a new problem caused by a change that previously worked. | Fixing Model Manager accidentally breaks model importing. |
| **Regression testing** | Testing previously working behavior after a change to make sure it still works. | After adding benchmarking, check that Model Manager can still list and import models. |
| **Unit test** | A small test checking one function or component in isolation. | Test that a YAML parser rejects an invalid capability name. |
| **Integration test** | A test checking whether multiple components work together. | Test that the installer correctly passes a storage selection to the installation engine. |
| **End-to-end (E2E) test** | A test covering a complete process from beginning to end. | Boot DAIA, run the installer, reboot, and verify the installed appliance. |
| **Smoke test** | A quick check that the most important functions work. | Confirm that the ISO boots and the installer opens. |
| **Test fixture** | Temporary sample data or a controlled environment used for testing. | A small GGUF model used to test the importer. |
| **Mock** | A substitute for a real component used during testing. | Simulate Docker responses without starting real containers. |
| **Assertion** | A statement in a test that must be true for the test to pass. | Assert that the installer never selects the live USB as its installation target. |
| **Edge case** | An unusual situation that might reveal a problem. | A USB drive is removed during content discovery. |

## Security and Trust

| Term | Plain-English meaning | DAIA example |
|---|---|---|
| **Provenance** | Where something came from and how we know its origin. | Record the source, version, and checksum of a downloaded AI model. |
| **Integrity** | Confidence that something has not been unexpectedly changed. | Verify an ISO's checksum after downloading it. |
| **Checksum** | A calculated fingerprint of a file, used to detect changes or corruption. | Compare SHA-256 values before importing a model. |
| **SHA-256** | A commonly used algorithm that produces a 256-bit fingerprint of data. | Identify the exact ISO or container image being tested. |
| **Authentication** | Checking who someone or something claims to be. | Verify an administrator's credentials. |
| **Authorization** | Deciding what an authenticated person or process is allowed to do. | Permit an authorized administrator to start a managed service. |
| **Least privilege** | Give a program only the permissions it needs. | A model-import process should not have unrestricted access to the system. |
| **Attack surface** | All the places where a system might be attacked. | Unnecessary open network ports increase the attack surface. |
| **Fail-closed** | If a security check cannot complete, deny the operation instead of allowing it. | Refuse to start pgvector when container identity verification fails. |
| **Isolation** | Keeping components separated so one cannot freely interfere with another. | Separate a model-export worker from privileged system operations. |
| **Trust boundary** | A point where information or control passes between components with different trust levels. | A graphical application requests a privileged export operation. |

## Git and Development Workflow

| Term | Plain-English meaning | DAIA example |
|---|---|---|
| **Repository** | A collection of project files and their recorded history. | The Debian-AI-Appliance GitHub repository. |
| **Commit** | A recorded snapshot of changes. | Save the completed Model Manager benchmark integration. |
| **Branch** | A separate line of development. | Develop a feature without immediately changing `master`. |
| **HEAD** | The commit currently checked out in a Git working copy. | Check which commit the local DAIA source is based on. |
| **Working tree** | The project files currently present on your computer. | Files edited locally but not yet committed. |
| **Diff** | A comparison showing what changed between versions. | Review added and removed lines before committing. |
| **Push** | Send local commits to a remote repository. | Upload verified commits to GitHub. |
| **Pull** | Bring remote changes into a local working copy. | Update the laptop's repository with GitHub changes. |
| **Checkpoint** | A known point where progress has been recorded and can be reviewed. | Commit a verified installer improvement before beginning another feature. |
| **Baseline** | A known starting state used for comparison. | Record the current Git commit and test results before making changes. |


## Architecture and Software Design

| Term | Plain-English meaning | DAIA example |
|---|---|---|
| **Architecture** | The overall organization of a system and how its parts work together. | Loom plans capabilities; the installer applies installation decisions. |
| **Control plane** | The part of a system that decides what should happen. | Loom resolves requested capabilities and produces an execution plan. |
| **Runtime** | Software and services operating while the installed system is running. | DAIA's service and module orchestration. |
| **Declarative configuration** | Describing the result you want rather than writing every step to achieve it. | A YAML appliance profile requests `desktop` capability. |
| **Manifest** | A structured file describing a component, its contents, or requirements. | A package manifest lists packages needed for an appliance. |
| **Dependency** | Something another component needs to function. | Open WebUI may depend on an inference service. |
| **Resolver** | Software that works out which components satisfy requested requirements. | Loom selects a provider for a capability. |
| **Planner** | Software that organizes the required operations before they run. | Loom creates a plan for installing an appliance profile. |
| **Executor** | Software that carries out planned operations. | Install packages and configure the target filesystem. |
| **Orchestration** | Coordinating multiple components or operations in the correct sequence. | First-boot provisioning starts and verifies required services. |
| **Idempotent** | Safe to repeat without creating additional unintended changes. | Re-running a configuration step does not duplicate entries. |
| **State** | Information describing the current condition of a system. | Whether first-boot provisioning has completed. |
| **Contract** | A clearly defined agreement about inputs, outputs, and behavior between components. | The pgvector startup helper must verify identity before starting a container. |

## Hardware and Offline Support

| Term | Plain-English meaning | Practical Loom example |
|---|---|---|
| **Debian Stable** | Debian's main supported release, prioritizing reliability and security updates. | Loom uses Stable packages by default when preparing Pragna's Wi-Fi and Bluetooth support. |
| **Debian Backports** | Selected newer software rebuilt for a supported Debian Stable release. | If a newer supported kernel or firmware package is needed for an SDR or Wi-Fi adapter, Loom evaluates the relevant Backports package rather than upgrading everything indiscriminately. |
| **Firmware** | Device-specific software that helps hardware operate. | A wireless adapter is visible over USB but cannot connect until its compatible firmware is installed. |
| **Driver** | Software that lets the operating system communicate with hardware. | Linux needs an appropriate driver to operate an RTL-SDR receiver. |
| **Hot-plug** | Connecting or disconnecting hardware while the computer is running. | Wanderer detects a GPS receiver connected after startup and makes it available without requiring a reboot when supported. |
| **Plug-and-play** | Hardware being detected and made usable with little or no manual setup. | Pragna identifies a supported Bluetooth dongle, loads the needed support, and confirms it can scan for devices. |
| **Device readiness** | Confirming a device can actually perform its job, not merely that it appears in a device list. | Loom checks that an SDR application can open the receiver before reporting it as ready. |
| **SDR** | Software-Defined Radio: radio equipment whose signal processing is largely handled by software. | Wanderer uses a compatible USB receiver to examine permitted radio signals offline; a receive-only SDR cannot transmit rescue calls. |
| **Offline repository** | A locally stored collection of packages or content usable without internet access. | Wanderer's installation media includes verified firmware packages for supported field devices. |

## Building and Installing DAIA

| Term | Plain-English meaning | DAIA example |
|---|---|---|
| **ISO image** | A file containing the filesystem and boot information needed to create installation media. | The DAIA installer ISO. |
| **Root filesystem (rootfs)** | The main filesystem containing an operating system's directories and files. | The Debian filesystem prepared during ISO construction. |
| **Payload** | Files, packages, services, or other content delivered as part of a build or installation. | Bundled Ollama packages and DAIA runtime files. |
| **Bootstrap** | Establishing the basic environment needed for further operations. | Preparing the Debian base filesystem. |
| **Provisioning** | Configuring a system and supplying the resources it needs for its intended use. | Configure services and import offline models after installation. |
| **Live USB** | A bootable USB that lets someone run an operating system without installing it. | Select *Try DAIA Live*. |
| **UEFI** | Modern firmware used to start computers. | Boot the DAIA installer on a UEFI laptop. |
| **Legacy BIOS** | An older computer boot method. | Boot the installer on an older workstation. |
| **Reproducible build** | A build process designed so the same inputs produce the same output. | Build the same appliance from fixed versions and recorded inputs. |
| **Rollback** | Returning to an earlier known state after a change causes problems. | Restore a previous configuration after a failed update. |
| **Disaster recovery** | Restoring a usable system after major failure. | Reinstall DAIA and recover user data and configuration from backups. |

## AI and Data

| Term | Plain-English meaning | DAIA example |
|---|---|---|
| **LLM** | Large Language Model: an AI model trained to work with language. | A model running through Ollama. |
| **Inference** | Using a trained AI model to produce an answer or prediction. | Ask a local model to summarize a document. |
| **GGUF** | A file format commonly used to store models for local inference. | Import a GGUF file into Model Manager. |
| **Quantization** | Reducing the numerical precision used to store a model, often making it smaller and faster. | Run a Q4 model on a CPU-only laptop. |
| **Context window** | The amount of input and conversation history a model can consider at once, measured in tokens. | Configure Ollama with an 8192-token context window. |
| **Benchmark** | A controlled measurement used to compare performance. | Measure model generation speed on the laptop. |
| **RAG** | Retrieval-Augmented Generation: finding relevant information and providing it to an AI model to help answer a question. | Search local documents before asking the model to answer. |
| **Embedding** | A numerical representation of text or other data used for similarity comparisons. | Represent document passages for semantic search. |
| **Vector database** | A database that can store and search numerical representations of data. | PostgreSQL with pgvector. |
| **Metadata** | Information describing other information. | Model name, version, tags, size, and source. |

---

## Reading the examples

Every term above includes a project-specific example. Read each example as a **scenario**, not a claim that the feature has already shipped. To apply a term in practice, ask: *What triggers this situation? What does Loom or DAIA do? How would we verify the result?*

For example, **regression testing** begins after a change, reruns checks for previously working behavior, and succeeds only if those checks still pass. **Device readiness** begins when hardware is detected, verifies that an application can use it, and reports a clear failure if detection alone is insufficient. **Guarded editing** begins with expected source content, changes only what matches, and stops without writing when the expectation is false.

## How to use this glossary

When a technical term appears in documentation, explanations should use the term consistently and link to this glossary where helpful.

The glossary should grow as the project introduces new concepts. Definitions should remain accessible without sacrificing technical accuracy.

**Document status:** Living reference, version 0.3. Examples illustrate concepts; they do not independently certify that every mentioned DAIA feature is fully implemented.
