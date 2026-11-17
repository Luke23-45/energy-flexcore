// P20.02 — Load leg 4: SOAK (written-unrun; decision record:
// docs/phases-11-20/phase-20-deployment-architecture-scale/records/
// 02_p20_02_load_test_plan.md).
//
// Shape: constant-arrival-rate at a steady level for a long duration —
// surfaces leaks, queue growth, and slow degradation invisible in short
// runs (context.md §4). Default duration 4h (operator-tunable).
//
// Targets:
//   POST /api/v1/optimization-runs + POST /api/v1/schedules (the accept
//     writes, one iteration per FLEXCORE_SOAK_PACE seconds)
//   GET /api/v1/live — the tenant-scoped SSE stream (P5.25), held open by
//     a second scenario of long-lived streaming connections consuming
//     chunks; the stream's at-least-once/UNORDERED contract and
//     RESYNC_REQUIRED mechanism are consumed as documented behavior and
//     asserted nowhere.
//   GET /ready — observed over time (the P1.22 readiness contract).
//
// SSE implementation note (A-P20.02-1): the stream scenario uses k6's
// HTTP streaming form (http.get with a per-chunk callback returning true
// to keep the stream open). The exact callback semantics are verified
// against the operator's pinned k6 binary AT EXECUTION — this script has
// never been parsed by k6 (written-unrun).
//
// NO THRESHOLDS: nothing gates on a duration (P3.27 discipline).
//
// Env contract: as legs 1/2, plus
//   FLEXCORE_SOAK_RATE    arrival rate for the accept-write scenario
//                         (default 3 iterations/s)
//   FLEXCORE_SOAK_DURATION leg duration (default 4h)
//   FLEXCORE_SOAK_SSE_CONNECTIONS concurrent open SSE streams (default 20)
//   FLEXCORE_SOAK_SSE_HOLD  seconds each stream stays open (default 3600)

import http from 'k6/http';
import { check, sleep } from 'k6';
import { uuidv4 } from 'https://jslib.k6.io/k6-utils/1.4.0/index.js';

const BASE_URL = __ENV.FLEXCORE_LOAD_BASE_URL || 'http://localhost:8080';
const TOKEN = __ENV.FLEXCORE_LOAD_TOKEN || '';
const TENANT = __ENV.FLEXCORE_LOAD_TENANT_UUID || '';
const PORTFOLIO_ID = __ENV.FLEXCORE_LOAD_PORTFOLIO_ID || '';
const SITE_ID = __ENV.FLEXCORE_LOAD_SITE_ID || '';
const ASSET_ID = __ENV.FLEXCORE_LOAD_ASSET_ID || '';
const RATE = Number(__ENV.FLEXCORE_SOAK_RATE || 3);
const DURATION = __ENV.FLEXCORE_SOAK_DURATION || '4h';
const SSE_CONNECTIONS = Number(__ENV.FLEXCORE_SOAK_SSE_CONNECTIONS || 20);
const SSE_HOLD_SECONDS = Number(__ENV.FLEXCORE_SOAK_SSE_HOLD || 3600);

export const options = {
  // Deliberately NO `thresholds` key anywhere.
  scenarios: {
    accept_writes: {
      executor: 'constant-arrival-rate',
      rate: RATE,
      timeUnit: '1s',
      duration: DURATION,
      preAllocatedVUs: 50,
      maxVUs: 200,
      exec: 'acceptWrite',
    },
    sse_streams: {
      executor: 'per-vu-iterations',
      vus: SSE_CONNECTIONS,
      iterations: 1,
      maxDuration: DURATION,
      exec: 'holdSseStream',
      startTime: '30s',
    },
  },
};

function requireEnv(name, value) {
  if (!value) {
    throw new Error(`${name} is required (see deploy/load-test/README.md)`);
  }
  return value;
}

export function setup() {
  requireEnv('FLEXCORE_LOAD_TOKEN', TOKEN);
  requireEnv('FLEXCORE_LOAD_TENANT_UUID', TENANT);
  requireEnv('FLEXCORE_LOAD_PORTFOLIO_ID', PORTFOLIO_ID);
  requireEnv('FLEXCORE_LOAD_SITE_ID', SITE_ID);
  requireEnv('FLEXCORE_LOAD_ASSET_ID', ASSET_ID);
}

export function acceptWrite() {
  const params = {
    headers: { Authorization: TOKEN, 'Content-Type': 'application/json' },
    tags: { leg: '4-soak', tenant: TENANT },
  };
  const candidateId = `SYN-cand-${uuidv4()}`;
  const startMillis = Date.now();

  http.post(
    `${BASE_URL}/api/v1/optimization-runs`,
    JSON.stringify({
      portfolio_id: PORTFOLIO_ID,
      site_id: SITE_ID,
      candidate_id: candidateId,
      status: 'feasible',
      problem_class: 'flexibility_scheduling',
      solver_name: 'synthetic-load-solver',
      solver_version: 'synthetic-1',
      provider_id: 'synthetic',
      provider_version: 'synthetic-1',
      objective_value: 1.0,
      objective_units: 'MW',
      input_snapshot_id: `SYN-snapshot-${uuidv4()}`,
      grid_model_version: 'synthetic-grid-v1',
      record_payload_json: JSON.stringify({
        synthetic: true,
        data_policy: 'deploy/load-test/data/synthetic-data-policy.md',
      }),
    }),
    params
  );
  http.post(
    `${BASE_URL}/api/v1/schedules`,
    JSON.stringify({
      portfolio_id: PORTFOLIO_ID,
      site_id: SITE_ID,
      revision: 1,
      horizon_start_millis: startMillis,
      horizon_end_millis: startMillis + 3600 * 1000,
      roll_trigger: 'initial_solve',
      candidate_id: candidateId,
      asset_time_coupled_validation_passed: true,
      schedule_payload_json: JSON.stringify([
        {
          asset_id: ASSET_ID,
          intervals: [{ start_millis: startMillis, end_millis: startMillis + 3600 * 1000, setpoint_mw: 0.5 }],
        },
      ]),
    }),
    params
  );

  // Readiness observed over time (the P1.22 contract: /ready reflects the
  // dependencies; a not_ready window under load is a recorded
  // observation, not a script-level failure).
  const r = http.get(`${BASE_URL}/ready`, { tags: { name: 'GET /ready', leg: '4-soak' } });
  check(r, {
    'readiness endpoint responded (any status is recorded; a 503 is an observation, not a gate)':
      (res) => res.status > 0,
  });
}

export function holdSseStream() {
  // One long-lived SSE connection per VU. The streaming callback consumes
  // chunks until the hold window elapses, then returns false to close.
  // The heartbeat (KeepAlive comment frames) and event cadence land in
  // the recorded output for the human to inspect — nothing here judges a
  // duration.
  const deadline = Date.now() + SSE_HOLD_SECONDS * 1000;
  http.get(
    `${BASE_URL}/api/v1/live`,
    {
      headers: { Authorization: TOKEN, Accept: 'text/event-stream' },
      tags: { name: 'GET /api/v1/live (stream)', leg: '4-soak' },
    },
    function (err, data) {
      if (err) {
        // Connection-level errors are recorded observations under soak;
        // the client resyncs honestly per the P5.25 contract — never
        // silently gap-fills.
        return false;
      }
      if (Date.now() >= deadline) {
        return false; // hold window elapsed; close this stream
      }
      return true; // keep streaming
    }
  );
  // After the stream closes the VU exits (iterations: 1 per VU).
  sleep(1);
}
