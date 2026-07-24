# ADR-001: Debian 13 as the Base Operating System

## Status

Accepted

## Decision

DAIA 1.0 will use Debian 13 amd64 as its base operating system.

## Rationale

Debian provides stability, broad hardware support, a large package ecosystem,
long-term maintainability and a strong open-source foundation.

## Consequences

DAIA inherits Debian packaging, systemd, security maintenance and installer
infrastructure.
EOF

cat > docs/adr/ADR-002-OFFLINE-FIRST.md <<'EOF'
# ADR-002: Offline-First Distribution

## Status

Accepted

## Decision

DAIA must install and provide its primary local AI assistant without requiring
internet access.

## Rationale

The target includes rural and low-connectivity environments. A network-dependent
installation would conflict with the project mission.

## Consequences

Docker packages, Ollama, Open WebUI and the default model must be bundled and
validated during the ISO build.
EOF

cat > docs/adr/ADR-003-XFCE-DESKTOP.md <<'EOF'
# ADR-003: XFCE as the Default Desktop

## Status

Accepted

## Decision

DAIA 1.0 will use XFCE as its default desktop environment.

## Rationale

XFCE is lightweight, stable and familiar to users accustomed to conventional
Windows-style desktop layouts. It performs well on modest hardware.

## Consequences

Desktop integration, launchers, branding and documentation will target XFCE
first.
