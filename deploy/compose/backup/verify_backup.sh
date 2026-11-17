#!/bin/sh
# P12.02 — backup verification job (WRITTEN-UNRUN; POSIX sh).
#
# Runs the stanza-level pgBackRest `check` (repository + archive
# verification) and records `info --output=json`. THE STANDING RULE THIS
# SCRIPT EXISTS UNDER: **a verify job is NOT a restore** (P9.24 §3 last
# row; the ledger's named classic failure LR-204 — a backup configuration
# that has never been restored). A passing `check` is a configuration
# outcome, never evidence that the backup restores. The evidence is the
# P12.03 restore drill (P9.24 §5 R-0..R-10), executed by the human gate
# into an isolated environment.
#
# Required environment:
#   PGBACKREST_BIN      path to the pgbackrest binary (default: pgbackrest)
#   PGBACKREST_CONFIG   stanza config path (deployment's authoritative copy)
#   VERIFY_LOG          output log path (default: ./verify_outcomes.log)
#
# Exit status is the `check` command's status.

set -eu

PGBACKREST_BIN="${PGBACKREST_BIN:-pgbackrest}"
PGBACKREST_CONFIG="${PGBACKREST_CONFIG:?PGBACKREST_CONFIG must name the stanza config}"
VERIFY_LOG="${VERIFY_LOG:-./verify_outcomes.log}"
STANZA="${STANZA:-flexcore}"

if ! command -v "$PGBACKREST_BIN" >/dev/null 2>&1; then
    printf 'verify_backup.sh: %s not found on PATH (PGBACKREST_BIN)\n' "$PGBACKREST_BIN" >&2
    exit 69
fi

if [ ! -r "$PGBACKREST_CONFIG" ]; then
    printf 'verify_backup.sh: stanza config not readable: %s\n' "$PGBACKREST_CONFIG" >&2
    exit 66
fi

set +e
"$PGBACKREST_BIN" --config="$PGBACKREST_CONFIG" --stanza="$STANZA" check
CHECK_STATUS=$?

"$PGBACKREST_BIN" --config="$PGBACKREST_CONFIG" --stanza="$STANZA" info --output=json \
    >> "$VERIFY_LOG" 2>&1
INFO_STATUS=$?
set -e

printf '%s stanza=%s check_status=%d info_status=%d\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$STANZA" "$CHECK_STATUS" "$INFO_STATUS" \
    >> "$VERIFY_LOG"

# A non-zero `info` status means the record could not even be written —
# fail loudly; the check status is the script's verdict.
if [ "$INFO_STATUS" -ne 0 ]; then
    printf 'verify_backup.sh: info command failed (%d) — verification record is incomplete\n' "$INFO_STATUS" >&2
    exit "$INFO_STATUS"
fi

exit "$CHECK_STATUS"
