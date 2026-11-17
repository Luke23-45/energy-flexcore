// P20.02 — Load leg 3: STRESS (written-unrun; decision record:
// docs/phases-11-20/phase-20-deployment-architecture-scale/records/
// 02_p20_02_load_test_plan.md).
//
// Shape: stepped ramping-arrival-rate climbing until the operator stops
// the leg. The leg LOOKS FOR the latency knee — the load level where
// latency stops growing linearly and turns sharply (context.md §4). It
// NEVER ASSUMES one: the knee is read by the human from the recorded
// per-step latency series, via the P20.01 framework.
//
// Targets: the same schedule-proposal accept path as leg 2 (POST
// /api/v1/optimization-runs + POST /api/v1/schedules) plus the READ-ONLY
// dispatch-command read surface. 429 responses from the P11.14 rate
// limiter are EXPECTED stress observations, recorded, never judged as
// failures.
//
// NO THRESHOLDS: nothing gates on a duration (P3.27 discipline). The
// operator may interrupt (Ctrl-C) once the knee region is observed; the
// partial run's output is still a valid record of the steps completed.
//
// Env contract: as leg 2, plus
//   FLEXCORE_STRESS_START_RATE (default 10),
//   FLEXCORE_STRESS_STEP_RATE  (default 10, added per step),
//   FLEXCORE_STRESS_MAX_RATE   (default 200, the climb ceiling),
//   FLEXCORE_STRESS_STEP_DUR   (default 3m).

import http from 'k6/http';
import { check } from 'k6';
import { uuidv4 } from 'https://jslib.k6.io/k6-utils/1.4.0/index.js';

const BASE_URL = __ENV.FLEXCORE_LOAD_BASE_URL || 'http://localhost:8080';
const TOKEN = __ENV.FLEXCORE_LOAD_TOKEN || '';
const TENANT = __ENV.FLEXCORE_LOAD_TENANT_UUID || '';
const PORTFOLIO_ID = __ENV.FLEXCORE_LOAD_PORTFOLIO_ID || '';
const SITE_ID = __ENV.FLEXCORE_LOAD_SITE_ID || '';
const ASSET_ID = __ENV.FLEXCORE_LOAD_ASSET_ID || '';
const START_RATE = Number(__ENV.FLEXCORE_STRESS_START_RATE || 10);
const STEP_RATE = Number(__ENV.FLEXCORE_STRESS_STEP_RATE || 10);
const MAX_RATE = Number(__ENV.FLEXCORE_STRESS_MAX_RATE || 200);
const STEP_DUR = __ENV.FLEXCORE_STRESS_STEP_DUR || '3m';

export const options = {
  // Deliberately NO `thresholds` key anywhere.
  scenarios: {
    stress: {
      executor: 'ramping-arrival-rate',
      startRate: START_RATE,
      timeUnit: '1s',
      preAllocatedVUs: 100,
      maxVUs: 600,
      stages: buildStressStages(START_RATE, STEP_RATE, MAX_RATE, STEP_DUR),
    },
  },
};

function buildStressStages(startRate, stepRate, maxRate, stepDur) {
  const stages = [];
  for (let rate = startRate + stepRate; rate <= maxRate; rate += stepRate) {
    stages.push({ duration: stepDur, target: rate });
  }
  if (stages.length === 0) {
    stages.push({ duration: stepDur, target: maxRate });
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
}

export default function () {
  const params = {
    headers: { Authorization: TOKEN, 'Content-Type': 'application/json' },
    tags: { leg: '3-stress', tenant: TENANT },
  };

  const candidateId = `SYN-cand-${uuidv4()}`;
  const startMillis = Date.now();

  const optRes = http.post(
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

  const schedRes = http.post(
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

  // Backpressure observations: 429s from the P11.14 limiter are EXPECTED
  // under stress and are recorded via their status codes in the output —
  // they are data about the system, not pass/fail results.
  check(optRes, {
    'candidate registration not an auth rejection (401 means the env contract is wrong)':
      (res) => res.status !== 401,
  });
  check(schedRes, {
    'schedule proposal not an auth rejection (401 means the env contract is wrong)':
      (res) => res.status !== 401,
  });

  http.get(`${BASE_URL}/api/v1/dispatch-commands?limit=50`, {
    headers: { Authorization: TOKEN },
    tags: { name: 'GET /api/v1/dispatch-commands', leg: '3-stress' },
  });
}
