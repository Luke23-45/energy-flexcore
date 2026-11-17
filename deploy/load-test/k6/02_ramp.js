// P20.02 — Load leg 2: RAMP (written-unrun; decision record:
// docs/phases-11-20/phase-20-deployment-architecture-scale/records/
// 02_p20_02_load_test_plan.md).
//
// Shape: ramping-arrival-rate stepping up through pre-registered levels
// (declared load INPUTS, operator-tunable — not pass criteria).
//
// Targets:
//   POST /api/v1/optimization-runs   (the persist-before-use candidate
//                                     registration — the P3.21 accept write)
//   POST /api/v1/schedules           (the schedule-proposal accept write;
//                                     `schedule.propose` authorization)
//   GET  /api/v1/dispatch-commands   (the READ-ONLY dispatch evidence
//                                     surface — read as such; there is NO
//                                     execute-command endpoint anywhere on
//                                     the API, `dto_phase4.rs`
//                                     NO_EXECUTE_ENDPOINT_STATEMENT)
//
// Honesty: no scenario fabricates a lifecycle state; every iteration
// proposes a NEW candidate chain (unique candidate_id, revision 1) — the
// duplicate-candidate 409 is avoided by construction (A-P20.02-5,
// verified at execution against the pinned build).
//
// NO THRESHOLDS: nothing gates on a duration (P3.27 discipline).
//
// Env contract: FLEXCORE_LOAD_BASE_URL, FLEXCORE_LOAD_TOKEN,
//   FLEXCORE_LOAD_TENANT_UUID, FLEXCORE_LOAD_PORTFOLIO_ID,
//   FLEXCORE_LOAD_SITE_ID, FLEXCORE_LOAD_ASSET_ID required;
//   FLEXCORE_RAMP_START_RATE (default 2), FLEXCORE_RAMP_END_RATE
//   (default 40), FLEXCORE_RAMP_STEPS (default 8), FLEXCORE_RAMP_STEP_DUR
//   (default 2m).

import http from 'k6/http';
import { check } from 'k6';
import { uuidv4 } from 'https://jslib.k6.io/k6-utils/1.4.0/index.js';

const BASE_URL = __ENV.FLEXCORE_LOAD_BASE_URL || 'http://localhost:8080';
const TOKEN = __ENV.FLEXCORE_LOAD_TOKEN || '';
const TENANT = __ENV.FLEXCORE_LOAD_TENANT_UUID || '';
const PORTFOLIO_ID = __ENV.FLEXCORE_LOAD_PORTFOLIO_ID || '';
const SITE_ID = __ENV.FLEXCORE_LOAD_SITE_ID || '';
const ASSET_ID = __ENV.FLEXCORE_LOAD_ASSET_ID || '';
const START_RATE = Number(__ENV.FLEXCORE_RAMP_START_RATE || 2);
const END_RATE = Number(__ENV.FLEXCORE_RAMP_END_RATE || 40);
const STEPS = Math.max(2, Number(__ENV.FLEXCORE_RAMP_STEPS || 8));
const STEP_DUR = __ENV.FLEXCORE_RAMP_STEP_DUR || '2m';

export const options = {
  // Deliberately NO `thresholds` key anywhere.
  scenarios: {
    ramp: {
      executor: 'ramping-arrival-rate',
      startRate: START_RATE,
      timeUnit: '1s',
      preAllocatedVUs: 50,
      maxVUs: 300,
      stages: buildStages(START_RATE, END_RATE, STEPS, STEP_DUR),
    },
  },
};

function buildStages(startRate, endRate, steps, stepDur) {
  // Linear climb from startRate to endRate over `steps` equal stages.
  const stages = [];
  for (let i = 1; i <= steps; i++) {
    const rate = Math.round(startRate + ((endRate - startRate) * i) / steps);
    stages.push({ duration: stepDur, target: rate });
  }
  return stages;
}

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
  const r = http.get(`${BASE_URL}/ready`);
  check(r, {
    'stack is ready before the ramp begins': (res) => res.status === 200,
  });
}

export default function () {
  const params = {
    headers: { Authorization: TOKEN, 'Content-Type': 'application/json' },
    tags: { leg: '2-ramp', tenant: TENANT },
  };

  // 1. Register one §12.6 optimization-run candidate (persisted before
  //    use; duplicates 409 by identity — the unique id avoids it).
  const candidateId = `SYN-cand-${uuidv4()}`;
  const optPayload = JSON.stringify({
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
  });
  const optRes = http.post(`${BASE_URL}/api/v1/optimization-runs`, optPayload, params);

  // 2. Propose the schedule revision realizing that candidate (the
  //    schedule-proposal accept write). The horizon is a fixed synthetic
  //    window: [now, now + 1h), UTC millis.
  const startMillis = Date.now();
  const schedulePayload = JSON.stringify({
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
  });
  const schedRes = http.post(`${BASE_URL}/api/v1/schedules`, schedulePayload, params);

  check(optRes, {
    'optimization-run registration is not an auth rejection (401 means the env contract is wrong)':
      (res) => res.status !== 401,
    'optimization-run registration is not a duplicate-candidate conflict':
      (res) => res.status !== 409,
  });
  check(schedRes, {
    'schedule proposal is not an auth rejection (401 means the env contract is wrong)':
      (res) => res.status !== 401,
  });

  // 3. Read the dispatch-command evidence surface (READ-ONLY — the
  //    recorded contract; there is no POST on this resource).
  http.get(`${BASE_URL}/api/v1/dispatch-commands?limit=50`, {
    headers: { Authorization: TOKEN },
    tags: { name: 'GET /api/v1/dispatch-commands', leg: '2-ramp' },
  });
}
