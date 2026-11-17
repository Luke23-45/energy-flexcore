// P20.02 — Load leg 1: BASELINE (written-unrun; decision record:
// docs/phases-11-20/phase-20-deployment-architecture-scale/records/
// 02_p20_02_load_test_plan.md).
//
// Shape: constant-arrival-rate at a LOW level for a short duration —
// establishes the reference envelope under near-idle load. Baseline, NOT
// target (the P3.27/P5.24 label discipline).
//
// NO THRESHOLDS: this script judges nothing against a duration or rate.
// It records; the human decides what the figures mean through the P20.01
// capacity framework (measurement -> number -> SLO, in that order).
//
// Targets (the real routes, apps/control-plane/src):
//   GET /ready                 (readiness probe)
//   GET /health                (liveness probe)
//   GET /api/v1/telemetry/query?...   (the tenant-scoped read surface)
//   GET /api/v1/sites/{id}/view       (the site-view read surface)
//
// Env contract (see ../README.md):
//   FLEXCORE_LOAD_BASE_URL       default http://localhost:8080
//   FLEXCORE_LOAD_TOKEN          dev-auth bearer token (REQUIRED)
//   FLEXCORE_LOAD_TENANT_UUID    seeded fixture tenant (REQUIRED)
//   FLEXCORE_LOAD_SITE_ID        seeded fixture site (REQUIRED)
//   FLEXCORE_BASELINE_RATE       arrival rate, iterations/s (default 5)
//   FLEXCORE_BASELINE_DURATION   leg duration (default 5m)

import http from 'k6/http';
import { check } from 'k6';

const BASE_URL = __ENV.FLEXCORE_LOAD_BASE_URL || 'http://localhost:8080';
const TOKEN = __ENV.FLEXCORE_LOAD_TOKEN || '';
const TENANT = __ENV.FLEXCORE_LOAD_TENANT_UUID || '';
const SITE_ID = __ENV.FLEXCORE_LOAD_SITE_ID || '';
const RATE = Number(__ENV.FLEXCORE_BASELINE_RATE || 5);
const DURATION = __ENV.FLEXCORE_BASELINE_DURATION || '5m';

export const options = {
  // Deliberately NO `thresholds` key anywhere (the P3.27
  // no-runtime-threshold discipline): nothing here gates on a duration.
  scenarios: {
    baseline: {
      executor: 'constant-arrival-rate',
      rate: RATE,
      timeUnit: '1s',
      duration: DURATION,
      preAllocatedVUs: 10,
      maxVUs: 50,
    },
  },
  discardResponseBodies: false,
};

function authHeaders() {
  if (!TOKEN) {
    throw new Error(
      'FLEXCORE_LOAD_TOKEN is required (the dev-auth bearer token; see deploy/load-test/README.md)'
    );
  }
  return {
    headers: {
      Authorization: TOKEN,
      'Content-Type': 'application/json',
    },
    tags: { leg: '1-baseline', tenant: TENANT },
  };
}

export default function () {
  const params = authHeaders();

  // Readiness/liveness (no auth required on the probes).
  http.get(`${BASE_URL}/ready`, { tags: { name: 'GET /ready' } });
  http.get(`${BASE_URL}/health`, { tags: { name: 'GET /health' } });

  // Tenant-scoped read surfaces (auth required).
  http.get(
    `${BASE_URL}/api/v1/sites/${SITE_ID}/view`,
    Object.assign({}, params, { tags: Object.assign({}, params.tags, { name: 'GET /api/v1/sites/{id}/view' }) })
  );
  http.get(
    `${BASE_URL}/api/v1/telemetry/query?asset_id=${SITE_ID}&limit=100`,
    Object.assign({}, params, { tags: Object.assign({}, params.tags, { name: 'GET /api/v1/telemetry/query' }) })
  );

  // Shape sanity only (never a pass/fail on any duration): the two
  // authenticated reads must not be auth rejections — an unexpected 401
  // means the env contract or the dev-auth build is wrong, and the run's
  // figures would be measuring the error path.
  const r = http.get(`${BASE_URL}/api/v1/sites/${SITE_ID}/view`, params);
  check(r, {
    'site view is not an auth rejection (expected 200; a 401 means the env contract is wrong)':
      (res) => res.status !== 401,
  });
}
