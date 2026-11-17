#!/bin/sh
# P20.02 — Load-ladder orchestrator (generalizes deploy/compose/smoke.sh).
#
# WRITTEN-UNRUN: this script has NEVER been executed. It requires an
# explicit human confirmation flag (FLEXCORE_LOAD_CONFIRM=1) so no
# non-interactive path can start a load run.
#
# Brings the Compose stack up (dev-auth overlay required for the
# authenticated legs, per deploy/compose/smoke.sh), waits for /ready, runs
# the four k6 legs IN ORDER, and tears the stack down. The Locust leg is
# run separately by the operator (see README.md step 4).
#
# NO THRESHOLD LOGIC EXISTS HERE: the script runs legs and reports exit
# codes of the tools; it judges nothing against a duration (the P3.27
# no-runtime-threshold discipline).
set -euo pipefail

COMPOSE_DIR="$(cd "$(dirname "$0")/../compose" && pwd)"
LOAD_DIR="$(cd "$(dirname "$0")" && pwd)"
RESULTS_DIR="${FLEXCORE_LOAD_RESULTS_DIR:-$LOAD_DIR/results}"
API_BASE="${FLEXCORE_LOAD_BASE_URL:-http://localhost:8080}"

log() { printf '[load] %s\n' "$1"; }

if [ "${FLEXCORE_LOAD_CONFIRM:-0}" != "1" ]; then
  log "refusing to run: set FLEXCORE_LOAD_CONFIRM=1 to confirm a HUMAN-driven load run"
  exit 1
fi

if [ ! -d "$COMPOSE_DIR" ]; then
  log "compose dir not found at $COMPOSE_DIR"
  exit 1
fi

mkdir -p "$RESULTS_DIR"

cleanup() {
  log "tearing down"
  (cd "$COMPOSE_DIR" && docker compose down -v >/dev/null 2>&1 || true)
}
trap cleanup EXIT

log "bringing the stack up"
(cd "$COMPOSE_DIR" && docker compose up -d --wait)

log "waiting for /ready"
i=0
until curl -fsS "$API_BASE/ready" >/dev/null 2>&1; do
  i=$((i + 1))
  if [ "$i" -gt 60 ]; then
    log "control plane never became ready"
    exit 1
  fi
  sleep 2
done

log "leg 1/4: baseline"
k6 run -o "json=$RESULTS_DIR/leg1_baseline.json" "$LOAD_DIR/k6/01_baseline.js"

log "leg 2/4: ramp"
k6 run -o "json=$RESULTS_DIR/leg2_ramp.json" "$LOAD_DIR/k6/02_ramp.js"

log "leg 3/4: stress (operator may interrupt once the knee is observed)"
k6 run -o "json=$RESULTS_DIR/leg3_stress.json" "$LOAD_DIR/k6/03_stress.js"

log "leg 4/4: soak (default 4h — override FLEXCORE_LOAD_SOAK_DURATION first)"
k6 run -o "json=$RESULTS_DIR/leg4_soak.json" "$LOAD_DIR/k6/04_soak.js"

log "ladder complete; summaries in $RESULTS_DIR"
log "feed the records into the P20.01 capacity-statement template FROM THE RUN ONLY"
