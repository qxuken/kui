#!/bin/sh
# The application writes a result after checking both widgets' model and caret.
set -eu
for attempt in $(seq 1 60); do
    result=$("$@" 2>/dev/null || true)
    case "$result" in
        PASS*) echo "$result"; exit 0 ;;
        FAIL*) echo "$result" >&2; exit 1 ;;
    esac
    sleep 1
done
echo "Timed out waiting for the two-field editing regression" >&2
exit 1
