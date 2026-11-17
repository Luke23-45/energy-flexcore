#!/bin/sh
# P12.02 — JetStream per-stream snapshot procedure (WRITTEN-UNRUN; POSIX sh).
# Implements the P12-E-2 DEFAULT posture: snapshot-based stream backup into
# the same object-storage target as the PostgreSQL backups. The decision
# itself is REGISTERED OPEN (`records/02_p12_02_backup_procedures.md` §4) —
# the human owner closes it with P12.14 drill evidence.
#
# THE CLI SURFACE IS UNVERIFIED: the `nats stream backup` command shape is
# cited from the JetStream documentation [S-65] and has never been run in
# this program; verify against the pinned nats CLI version at deployment
# and record defects.
#
# This script PERFORMS NO RESTORE. Restore (and the scoped restore
# authorization discipline — an unauthorized stream-name restore must be
# REFUSED, inherited unchanged from P10.11 §4.4) belongs to the P12.14
# drill design.
#
# Required environment:
#   NATS_URL          JetStream endpoint, e.g. nats://localhost:4222
#   NATS_BIN          nats CLI path (default: nats)
#   STREAMS           whitespace-separated stream names; default is the four
#                     Phase 1 provisioned streams per
#                     crates/adapters/nats/src/topology.rs (P1.14 §4.2):
#                     FLEXCORE_REGISTRY FLEXCORE_STATE FLEXCORE_AUDIT
#                     FLEXCORE_DLQ
#   SNAPSHOT_ROOT     staging root for snapshots + manifests; on a real
#                     deployment this is a mounted object-storage location or
#                     a staging dir synced to the P12.01 §4 target by the
#                     deployment's own sync mechanism (never improvised here)
#   MANIFEST_PY       path to snapshot_manifest.py (default: beside this
#                     script)

set -eu

NATS_URL="${NATS_URL:?NATS_URL must name the JetStream endpoint}"
NATS_BIN="${NATS_BIN:-nats}"
STREAMS="${STREAMS:-FLEXCORE_REGISTRY FLEXCORE_STATE FLEXCORE_AUDIT FLEXCORE_DLQ}"
SNAPSHOT_ROOT="${SNAPSHOT_ROOT:?SNAPSHOT_ROOT must name the staging root}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
MANIFEST_PY="${MANIFEST_PY:-$SCRIPT_DIR/snapshot_manifest.py}"

if ! command -v "$NATS_BIN" >/dev/null 2>&1; then
    printf 'jetstream_snapshot.sh: %s not found on PATH (NATS_BIN)\n' "$NATS_BIN" >&2
    exit 69
fi

if [ ! -r "$MANIFEST_PY" ]; then
    printf 'jetstream_snapshot.sh: manifest tool not readable: %s\n' "$MANIFEST_PY" >&2
    exit 66
fi

STAMP_UTC="$(date -u +%Y-%m-%dT%H%M%SZ)"
OVERALL_STATUS=0

for STREAM in $STREAMS; do
    STREAM_DIR="$SNAPSHOT_ROOT/$STREAM/$STAMP_UTC"
    mkdir -p "$STREAM_DIR"

    set +e
    "$NATS_BIN" --server="$NATS_URL" stream backup "$STREAM" "$STREAM_DIR"
    STATUS=$?
    set -e

    if [ "$STATUS" -ne 0 ]; then
        printf 'jetstream_snapshot.sh: snapshot FAILED for stream %s (status %d)\n' \
            "$STREAM" "$STATUS" >&2
        OVERALL_STATUS=1
        continue
    fi

    # Integrity manifest (sha256 per file) for every stream snapshot —
    # uploaded alongside the artifacts to the P12.01 §4 target.
    python -I "$MANIFEST_PY" --root "$STREAM_DIR"

    printf '%s stream=%s snapshot_dir=%s manifest=%s/manifest.json status=0\n' \
        "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$STREAM" "$STREAM_DIR" "$STREAM_DIR"
done

exit "$OVERALL_STATUS"
