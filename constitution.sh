#!/usr/bin/env bash

set -e

PROJECT="daia-constitution"

mkdir -p "$PROJECT"/{
docs/architecture,
docs/constitution,
docs/diagrams,
assets
}

# Root files
touch "$PROJECT"/README.md

# Architecture documents
touch \
"$PROJECT"/docs/architecture/architecture-baseline.md \
"$PROJECT"/docs/architecture/decision-log.md \
"$PROJECT"/docs/architecture/glossary.md

# Constitution documents
touch \
"$PROJECT"/docs/constitution/part-01-core-architecture.md \
"$PROJECT"/docs/constitution/part-02-canonical-state-model.md \
"$PROJECT"/docs/constitution/part-03-reconciliation.md \
"$PROJECT"/docs/constitution/part-04-planning.md \
"$PROJECT"/docs/constitution/part-05-planning.md \
"$PROJECT"/docs/constitution/part-06-execution.md \
"$PROJECT"/docs/constitution/part-07-observation.md \
"$PROJECT"/docs/constitution/part-08-verification.md \
"$PROJECT"/docs/constitution/part-09-state-acceptance.md \
"$PROJECT"/docs/constitution/part-10-resource-model.md \
"$PROJECT"/docs/constitution/part-11-ownership.md \
"$PROJECT"/docs/constitution/part-12-persistence.md \
"$PROJECT"/docs/constitution/part-13-events.md \
"$PROJECT"/docs/constitution/part-14-security.md \
"$PROJECT"/docs/constitution/part-15-interfaces.md \
"$PROJECT"/docs/constitution/part-16-governance.md \
"$PROJECT"/docs/constitution/part-17-versioning.md \
"$PROJECT"/docs/constitution/part-18-conformance.md

echo
echo "DAIA Constitution workspace created."
echo
tree "$PROJECT" 2>/dev/null || find "$PROJECT"
