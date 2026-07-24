# ShellCheck Policy

DAIA uses ShellCheck for static analysis.

Rules:

- Fix all ShellCheck warnings.
- SC1091 may be disabled when sourcing runtime-generated paths.
- All variables must be quoted.
- Use local variables inside functions.
- Use $(...) instead of backticks.
- Use set -euo pipefail for executable scripts.
