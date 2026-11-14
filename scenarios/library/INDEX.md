# Scenario library index (§19.3 cases; P5.20)

One line per library record: purpose, scale, expected runtime class
(DECLARED, never measured), backend dependency. All cases are synthetic;
every record carries its per-case IEEE exclusion record and pre-registered
(UNRUN) expected outcomes.

| Record | Purpose | Scale | Runtime class | Backend |
|---|---|---|---|---|
| `lib-01-minimal-single-asset.json` | The minimal replayable case: one tenant, one site, ONE BESS; record round-trip, fail-loud pin verification, replay identity at the smallest scale | minimal | seconds-class on the scripted backend (96 ticks × 1 asset + one dispatch slice) — not measured | `scripted` |
| `lib-02-small-portfolio.json` | The full synthetic-portfolio fixture (2 sites, BESS+PV+load+EVSE+meter per site, 1-min cadence); the replay-identity workhorse and negative-control base | small | seconds-class on the scripted backend (1440 ticks × 10 assets + one dispatch slice) — not measured | `scripted` |
| `lib-03-medium-multi-site-meshed.json` | Two sites against the MESHED two-feeder fixture (common substation, balanced only, `TopologyScope::RadialAndMeshed` per P4.34) | medium | seconds-class on the scripted backend — not measured | `scripted` |
| `lib-04-stress-worker-timeout.json` | Stress: the worker_process backend's bounded-timeout fail-closed path (a hung worker is killed; never a pass, never a retry) | stress | bounded by the pinned worker timeout (30 s default) — not measured | `worker_process` (UNVERIFIED pandapower API — the leg was written, never executed) |
| `lib-05-stress-nonconvergence.json` | Stress: a non-converged power flow routes FAIL-CLOSED (retry NONE), never a pass; convergence is the §19.4 grid-feasibility metric | stress | bounded by the pinned worker timeout — not measured | `worker_process` (UNVERIFIED) |
| `lib-06-stress-pv-ramp.json` | Stress: PV ramp / forecast error (P3.26 scenario (b) + the v1-error-pv-ramp fixture); the stressor is the forecast, the telemetry stays all-GOOD | stress | seconds-class on the scripted backend (identical volume to lib-02) — not measured | `scripted` |
| `lib-07-stress-asset-dropout.json` | Stress: asset dropout mid-horizon (P3.26 scenario (c)); the drop window is also the communication-loss analogue; the REAL P3.24 roll fires via the roll pin | stress | seconds-class on the scripted backend — not measured | `scripted` |
| `lib-08-stress-reverse-power-flow.json` | Stress: reverse power flow (+1.5 MW export dispatch target); the scripted-backend limitation is recorded honestly; the violation expectation is pre-registered for the worker leg | stress | seconds-class on the scripted backend — not measured | `scripted` |

Scale limit: the first feeder tests do NOT establish capability for a
10,000-bus utility network (§19.3).

Dependencies: `scripted` cases need nothing beyond the workspace build;
`worker_process` cases additionally need the Python worker environment
(`apps/worker/python/requirements.txt`, pandapower 3.5.6 — never installed
in the authoring loop; the human build/test pass owns that execution).
