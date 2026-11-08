#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck disable=SC1091
source "$SCRIPT_DIR/variables.sh"

echo "========================================"
echo " Building DAIA ISO"
echo "========================================"

[[ -d "$EXTRACT_DIR" ]] || {
    echo "ERROR: Extracted ISO directory not found:"
    echo "  $EXTRACT_DIR"
    exit 1
}

# Validate the staged Ollama payload before modifying the output ISO.
OLLAMA_CONFIG="$EXTRACT_DIR/opt/daia/config/pragna.conf"
OLLAMA_DIRECTORY="$EXTRACT_DIR/opt/daia/payload/packages/ollama"
OLLAMA_MANIFEST="$OLLAMA_DIRECTORY/ollama-release.json"
OLLAMA_CHECKER="$SCRIPT_DIR/check-ollama-release.py"

[[ -f "$OLLAMA_CONFIG" && ! -L "$OLLAMA_CONFIG" ]] || {
    echo "ERROR: Missing or unsafe staged DAIA configuration"
    exit 1
}

ollama_settings=()
while IFS= read -r setting
do
    ollama_settings+=("$setting")
done < <(grep '^DAIA_ENABLE_OLLAMA=' "$OLLAMA_CONFIG" || true)

[[ "${#ollama_settings[@]}" -eq 1 ]] || {
    echo "ERROR: Expected exactly one Ollama enablement setting"
    exit 1
}

case "${ollama_settings[0]}" in
    'DAIA_ENABLE_OLLAMA="true"')
        ollama_args=(
            --release-manifest "$OLLAMA_MANIFEST"
            --archive-directory "$OLLAMA_DIRECTORY"
        )
        case "${DAIA_OLLAMA_OFFLINE_APPROVED:-false}" in
            true) ollama_args+=(--approved-offline) ;;
            false) ;;
            *)
                echo "ERROR: Invalid Ollama offline approval value"
                exit 1
                ;;
        esac
        python3 "$OLLAMA_CHECKER" "${ollama_args[@]}" || exit 1
        ;;
    'DAIA_ENABLE_OLLAMA="false"')
        ;;
    *)
        echo "ERROR: Invalid Ollama enablement setting"
        exit 1
        ;;
esac

mkdir -p "$OUTPUT_DIR"
rm -f "$OUTPUT_ISO"

echo "Source ISO : $SOURCE_ISO"
echo "Extract tree: $EXTRACT_DIR"
echo "Output ISO : $OUTPUT_ISO"
echo

xorriso \
    -indev "$SOURCE_ISO" \
    -outdev "$OUTPUT_ISO" \
    -boot_image any replay \
    -map "$EXTRACT_DIR" / \
    -commit

[[ -f "$OUTPUT_ISO" ]] || {
    echo "ERROR: ISO creation completed without producing:"
    echo "  $OUTPUT_ISO"
    exit 1
}

echo
echo "ISO created successfully:"
echo "  $OUTPUT_ISO"
