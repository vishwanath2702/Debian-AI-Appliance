#!/bin/bash

# DAIA KDE model importer — read-only discovery and authorized GGUF import.

DAIA_COMMAND="${DAIA_COMMAND:-/usr/bin/daia}"
TITLE="DAIA Model Importer"

progress_service=""
progress_object=""
import_total_bytes=""
progress_output_file=""

open_progress_dialog() {
    local reference

    reference="$(
        kdialog --title "$TITLE" \
            --progressbar "Preparing model import..." 100
    )" || return 1

    [[ "$reference" == *" /ProgressDialog" ]] || return 1

    progress_service="${reference%% *}"
    progress_object="${reference#* }"

    if ! qdbus6 "$progress_service" "$progress_object" \
        org.kde.kdialog.ProgressDialog.showCancelButton false; then
        close_progress_dialog
        return 1
    fi

    if ! qdbus6 "$progress_service" "$progress_object" \
        org.freedesktop.DBus.Properties.Set \
        org.kde.kdialog.ProgressDialog autoClose false; then
        close_progress_dialog
        return 1
    fi
}

update_progress_dialog() {
    [[ -n "$progress_service" && -n "$progress_object" ]] || return 1

    qdbus6 "$progress_service" "$progress_object" \
        org.kde.kdialog.ProgressDialog.setLabelText "$1"
}

close_progress_dialog() {
    if [[ -n "$progress_service" && -n "$progress_object" ]]; then
        qdbus6 "$progress_service" "$progress_object" \
            org.kde.kdialog.ProgressDialog.close >/dev/null 2>&1 || true
    fi

    progress_service=""
    progress_object=""
}

show_import_progress_event() {
    local event="$1"
    local bytes
    local percentage
    local label

    case "$event" in
        DAIA_PROGRESS_TOTAL_BYTES=*)
            bytes="${event#DAIA_PROGRESS_TOTAL_BYTES=}"
            [[ "$bytes" =~ ^[0-9]+$ ]] || return 0
            import_total_bytes="$bytes"
            return 0
            ;;
        DAIA_PROGRESS_BYTES=*)
            bytes="${event#DAIA_PROGRESS_BYTES=}"
            [[ "$bytes" =~ ^[0-9]+$ ]] || return 0

            if [[ "$import_total_bytes" =~ ^[0-9]+$ ]] &&
               [[ "$import_total_bytes" != "0" ]]; then
                percentage="$(
                    awk -v copied="$bytes" -v total="$import_total_bytes" '
                        BEGIN {
                            if (total <= 0) { print 0; exit }
                            value = int((copied / total) * 100)
                            if (value > 100) value = 100
                            if (value < 0) value = 0
                            print value
                        }'
                )"
                label="Copying GGUF model: $selected_model

Progress: $percentage%
Transferred: $bytes / $import_total_bytes bytes"

                if [[ -n "$progress_service" && -n "$progress_object" ]]; then
                    qdbus6 "$progress_service" "$progress_object" \
                        org.freedesktop.DBus.Properties.Set \
                        org.kde.kdialog.ProgressDialog value "$percentage" \
                        >/dev/null 2>&1 || true
                fi
            else
                label="Copying GGUF model: $selected_model

Transferred: $bytes bytes"
            fi
            ;;
        DAIA_PROGRESS_PHASE=registering)
            label="Registering model with Ollama: $model_name

GGUF file: $selected_model"
            ;;
        *)
            return 0
            ;;
    esac

    update_progress_dialog "$label" >/dev/null 2>&1 || true
}

run_import_with_progress() {
    local status
    local event
    local progress_output

    progress_output="$(mktemp)" || return 1
    progress_output_file="$progress_output"

    if command -v qdbus6 >/dev/null 2>&1; then
        open_progress_dialog >/dev/null 2>&1 || true
    fi

    update_progress_dialog "Importing GGUF model: $selected_model

Waiting for administrator authorization..." >/dev/null 2>&1 || true

    : > "$error_file"

    pkexec /usr/bin/daia-model-import \
        "$selected_device" "$selected_model" "$model_name" \
        >"$progress_output" 2>"$error_file" &
    local import_pid=$!

    local progress_fd
    local pending_event=""
    local fragment=""
    exec {progress_fd}<"$progress_output"

    while kill -0 "$import_pid" 2>/dev/null; do
        while true; do
            fragment=""
            if IFS= read -r -t 0.05 -u "$progress_fd" fragment; then
                event="${pending_event}${fragment}"
                pending_event=""
                show_import_progress_event "$event"
            else
                pending_event+="$fragment"
                break
            fi
        done
        sleep 0.2
    done

    wait "$import_pid"
    status=$?

    while true; do
        fragment=""
        if IFS= read -r -u "$progress_fd" fragment; then
            event="${pending_event}${fragment}"
            pending_event=""
            show_import_progress_event "$event"
        else
            pending_event+="$fragment"
            break
        fi
    done

    exec {progress_fd}<&-

    rm -f "$progress_output"
    progress_output_file=""
    close_progress_dialog

    return "$status"
}

error_file="$(mktemp)" || {
    printf 'Error: cannot create temporary error file\n' >&2
    exit 1
}
cleanup_importer() {
    close_progress_dialog

    if [[ -n "$progress_output_file" ]]; then
        rm -f "$progress_output_file"
        progress_output_file=""
    fi

    rm -f "$error_file"
}

trap cleanup_importer EXIT

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

                            if run_import_with_progress; then
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
