#!/bin/sh
# P12.02 — PostgreSQL backup procedure (WRITTEN-UNRUN; POSIX sh; no tool is
# installed in the authoring loop). The first human run may surface defects
# (paths, option names against the pinned pgBackRest version, env wiring) —
# record them; do not assume correctness.
#
# Runs ONE pgBackRest backup of the flexcore stanza and appends an outcome
# line to a log file. It schedules nothing: the cadence comes from the
# P12.04 register (RECOMMENDATIONS; operator sign-off cell EMPTY) and is
# the deployment scheduler's configuration.
#
# Required environment:
#   PGBACKREST_BIN      path to the pgbackrest binary (default: pgbackrest)
#   PGBACKREST_CONFIG   path to the deployment's authoritative stanza config
#                       (the shape is deploy/compose/backup/pgbackrest.conf)
#   BACKUP_TYPE         one of: full | diff | incr (default: diff)
#   BACKUP_LOG          outcome log file (default: ./backup_outcomes.log)
#
# Exit status is the backup command's status. The outcome line records the
# UTC timestamp, the requested type, the exit code, and the stanza — never
# a claim that the backup is restorable. Restorability is established ONLY
# by the P12.03 restore drill (P9.24 §5), never by this script.

set -eu

PGBACKREST_BIN="${PGBACKREST_BIN:-pgbackrest}"
PGBACKREST_CONFIG="${PGBACKREST_CONFIG:?PGBACKREST_CONFIG must name the stanza config}"
BACKUP_TYPE="${BACKUP_TYPE:-diff}"
BACKUP_LOG="${BACKUP_LOG:-./backup_outcomes.log}"
STANZA="${STANZA:-flexcore}"

case "$BACKUP_TYPE" in
    full|diff|incr) ;;
    *)
        printf 'backup_postgres.sh: BACKUP_TYPE must be full|diff|incr, got "%s"\n' "$BACKUP_TYPE" >&2
        exit 64
        ;;
esac

if ! command -v "$PGBACKREST_BIN" >/dev/null 2>&1; then
    printf 'backup_postgres.sh: %s not found on PATH (PGBACKREST_BIN)\n' "$PGBACKREST_BIN" >&2
    exit 69
fi

if [ ! -r "$PGBACKREST_CONFIG" ]; then
    printf 'backup_postgres.sh: stanza config not readable: %s\n' "$PGBACKREST_CONFIG" >&2
    exit 66
fi

STARTED_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

set +e
"$PGBACKREST_BIN" --config="$PGBACKREST_CONFIG" --stanza="$STANZA" --type="$BACKUP_TYPE" backup
STATUS=$?
set -e

FINISHED_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

printf '%s start=%s end=%s stanza=%s type=%s status=%d\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    "$STARTED_UTC" "$FINISHED_UTC" "$STANZA" "$BACKUP_TYPE" "$STATUS" \
    >> "$BACKUP_LOG"

exit "$STATUS"
