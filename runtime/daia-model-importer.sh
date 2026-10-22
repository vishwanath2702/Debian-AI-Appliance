#!/bin/bash

# DAIA KDE model importer — read-only discovery and authorized GGUF import.

DAIA_COMMAND="${DAIA_COMMAND:-/usr/bin/daia}"
TITLE="DAIA Model Importer"

error_file="$(mktemp)" || {
    printf 'Error: cannot create temporary error file\n' >&2
    exit 1
}
trap 'rm -f "$error_file"' EXIT

if ! command -v kdialog >/dev/null 2>&1; then
    printf 'Error: kdialog is required\n' >&2
elif [[ ! -x "$DAIA_COMMAND" ]]; then
    kdialog --title "$TITLE" --error "DAIA CLI is not installed at $DAIA_COMMAND."
else
    volumes_output="$("$DAIA_COMMAND" model list-volumes 2>"$error_file")"
    status=$?

    if (( status != 0 )); then
        kdialog --title "$TITLE" --error "$(cat "$error_file")"
        exit 1
    elif [[ -z "$volumes_output" ]]; then
        kdialog --title "$TITLE" --sorry "No model source volumes were found."
    else
        volumes=()

        while IFS= read -r device; do
            [[ -z "$device" ]] && continue
            volumes+=("$device" "$device")
        done <<< "$volumes_output"

        selected_device="$(
            kdialog --title "$TITLE" \
                --menu "Select a model source volume:" \
                "${volumes[@]}"
        )"
        status=$?

        if (( status == 0 )) && [[ -n "$selected_device" ]]; then
            if ! command -v pkexec >/dev/null 2>&1; then
                kdialog --title "$TITLE" \
                    --error "Administrator authorization (pkexec) is unavailable."
                exit 1
            fi

            : > "$error_file"
            models_output="$(
                pkexec /usr/bin/daia-model-import list-models "$selected_device" 2>"$error_file"
            )"
            status=$?

            if (( status != 0 )); then
                kdialog --title "$TITLE" --error "$(cat "$error_file")"
                exit 1
            elif [[ -z "$models_output" ]]; then
                kdialog --title "$TITLE" --sorry "No GGUF models were found."
            else
                models=()

                while IFS= read -r filename; do
                    [[ -z "$filename" ]] && continue
                    models+=("$filename" "$filename")
                done <<< "$models_output"

                selected_model="$(
                    kdialog --title "$TITLE" \
                        --menu "Select a GGUF model:" \
                        "${models[@]}"
                )"
                status=$?

                if (( status == 0 )) && [[ -n "$selected_model" ]]; then
                    model_name="$(
                        kdialog --title "$TITLE" \
                            --inputbox "Enter the Ollama model name:" \
                            "qwen2.5-local"
                    )"
                    status=$?

                    if (( status == 0 )) && [[ -n "$model_name" ]]; then
                        if kdialog --title "$TITLE" \
                            --yesno "Review model import:

GGUF file: $selected_model

Source: $selected_device

Ollama name: $model_name

Continue with model import?"; then
                            if ! command -v pkexec >/dev/null 2>&1; then
                                kdialog --title "$TITLE" \
                                    --error "Administrator authorization (pkexec) is unavailable."
                                exit 1
                            fi

                            : > "$error_file"
                            if pkexec /usr/bin/daia-model-import \
                                "$selected_device" "$selected_model" "$model_name" \
                                2>"$error_file"; then
                                kdialog --title "$TITLE" \
                                    --msgbox "Model imported successfully.

Ollama name: $model_name"
                            else
                                details="$(cat "$error_file")"

                                if kdialog --title "$TITLE" \
                                    --yesno "Model import failed.

${details:-Administrator authorization was cancelled or the import failed.}

If the GGUF was already imported, DAIA can retry Ollama registration without copying it again.

Attempt recovery for: $model_name?"; then
                                    : > "$error_file"

                                    if pkexec /usr/bin/daia-model-import \
                                        resume-model "$model_name" 2>"$error_file"; then
                                        kdialog --title "$TITLE" \
                                            --msgbox "Model recovery completed successfully.

Ollama name: $model_name"
                                    else
                                        details="$(cat "$error_file")"
                                        kdialog --title "$TITLE" \
                                            --error "Model recovery failed.

${details:-Administrator authorization was cancelled or recovery failed.}"
                                        exit 1
                                    fi
                                else
                                    exit 1
                                fi
                            fi
                        fi
                    fi
                fi
            fi
        fi
    fi
fi
