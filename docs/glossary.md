# Loom & DAIA — Technical Glossary

**Version:** 0.1  
**Purpose:** Explain technical terms used in Loom and DAIA development in everyday language.

This glossary is intended for developers, contributors, and anyone following the project without needing to know every technical term in advance.

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
| **Guarded edit** | A change made only after checking specific conditions, to avoid modifying unexpected files. | Confirm the expected code exists before replacing it. |

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

## How to use this glossary

When a technical term appears in documentation, explanations should use the term consistently and link to this glossary where helpful.

The glossary should grow as the project introduces new concepts. Definitions should remain accessible without sacrificing technical accuracy.

**Document status:** Initial draft. Examples illustrate concepts; they do not independently certify that every mentioned DAIA feature is fully implemented.
