#!/bin/bash
#
# DAIA User Interface Helpers
#

ui_line() {
    printf '%*s\n' 40 '' | tr ' ' '='
}

ui_header() {
    echo
    ui_line
    printf " %s\n" "$1"
    ui_line
    echo
}

ui_section() {
    echo
    echo "$1"
    printf '%*s\n' "${#1}" '' | tr ' ' '-'
}

ui_pass() {
    echo
    ui_line
    echo "PASS - $1"
    ui_line
}

ui_fail() {
    echo
    ui_line
    echo "FAIL - $1"
    ui_line
}

ui_warning() {
    echo
    echo "WARNING - $1"
}
