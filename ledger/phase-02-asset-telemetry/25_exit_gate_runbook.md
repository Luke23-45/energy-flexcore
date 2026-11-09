# P2.25 — Exit-gate runbook: registration, 24-hour run, ingestion, verification (the human's execution)

| Field | Value |
|---|---|
| Purpose | The exact command sequence for the P2.25 exit-gate steps the implementing subtask could NOT run (loop bans): portfolio registration via the API, the 24-hour simulation run, ingestion through the pipeline, and the coverage/canonical/replay verification |
| Scenario | `scenarios/synthetic-portfolio/scenario-v2.json` (`synthetic-portfolio-exit-gate` v2, master seed `20260601`, 24 h from `2026-06-01T00:00:00Z`, 1-minute cadence, faults disabled) |
| Status | TEMPLATE TO EXECUTE — every command below must be run and its output recorded in the P2.26 demonstration record. Nothing in this file has been executed by the implementing subtask |

---

## 0. Recorded blocker — read before starting

The worker composition (`apps/worker`, `compose_ingestion`) currently
returns `None`: **no `TelemetrySource` is linked at composition**, so the
worker starts without the ingestion task (the recorded P2.10/P2.14 wiring
status). Step 4 below (the run + ingest) is executable only after that
composition lands the simulator source:

```rust
// apps/worker/src/main.rs, compose_ingestion:
let source: Arc<dyn TelemetrySource> = Arc::new(flexcore_simulator::...); // the P2.14 source
let query = TelemetryQuery::new(&ctx, asset_ids, from, until, quantities)?;
Some((source, query))
```

Everything else (steps 1–3, 5–7) is executable today. Record this blocker
and its resolution in the P2.26 demonstration record.

## 1. Bring up the stack and apply migrations

```sh
docker compose up -d          # PostgreSQL 18 + NATS (the P1.25 Compose stack)
cargo run -p flexcore-control-plane -- serve   # the API (see its config docs for FLEXCORE_* env)
cargo run -p flexcore-worker    # ingestion + maintenance (needs the step-0 wiring for the run)
# Migrations apply on control-plane/worker start (run_migrations); verify:
psql "$FLEXCORE_DATABASE_URL" -c "\d telemetry" | grep telemetry_version   # 0012 applied
```

Authentication: every API call below needs the deployment's identity
resolver (Phase 2 ships the dev/test-profile resolver only — the recorded
P1.17 `dev-auth` status). Export the caller's credentials per the
deployment's auth setup; the examples use `$AUTH` for that header.

## 2. Register the portfolio through the API (P2.25 step 2 — never SQL)

The bodies below are derived from `scenario-v2.json`. Capture every
returned UUID with `jq` as shown; later steps reference them.

```sh
API=http://localhost:8080/api/v1

# 2.1 Tenant
TENANT=$(curl -sS -X POST "$API/tenants" -H "$AUTH" -H 'content-type: application/json' -d '{
  "name": "tenant-flexcore-demo",
  "identity_provider_reference": "idp:flexcore-demo",
  "policy_profile": "profile:default"
}')
TID=$(jq -r .data.id <<<"$TENANT")

# 2.2 Portfolio (market_region is a LABEL — P2.02, D1 open)
PID=$(curl -sS -X POST "$API/portfolios" -H "$AUTH" -H 'content-type: application/json' -d '{
  "name": "portfolio-demo",
  "operator": "Neryva simulated operator",
  "market_region": "DE-LABEL-ONLY",
  "aggregation_rules": "rules:constrained-aggregation"
}' | jq -r .data.id)

# 2.3 Sites (timezone display-only; the grid model is a reference, D3 open)
NID=$(curl -sS -X POST "$API/sites" -H "$AUTH" -H 'content-type: application/json' -d '{
  "portfolio_id": "'"$PID"'",
  "name": "site-north",
  "location": "{}",
  "timezone": "Europe/Berlin",
  "point_of_common_coupling": "pcc-north",
  "grid_model_reference": "synthetic-feeder-a:v1",
  "edge_identity": "edge:simulator"
}' | jq -r .data.id)
SID=$(curl -sS -X POST "$API/sites" -H "$AUTH" -H 'content-type: application/json' -d '{
  "portfolio_id": "'"$PID"'",
  "name": "site-south",
  "location": "{}",
  "timezone": "Europe/Berlin",
  "point_of_common_coupling": "pcc-south",
  "grid_model_reference": "synthetic-feeder-a:v1",
  "edge_identity": "edge:simulator"
}' | jq -r .data.id)
```

2.4 The ten assets (five per site; the load's `rated_peak_mw` maps to
`rated_power_mw`). Repeat the pattern — the full bodies, one per asset:

| asset_key | site | asset_type | rated_power_mw | rated_energy_mwh | bus_reference | capability_profile |
|---|---|---|---|---|---|---|
| site-north-bess-1 | `$NID` | `bess` | 2.0 | 4.0 | bus-north-lv-1 | cap/bess-default |
| site-north-pv-1 | `$NID` | `pv_inverter` | 1.5 | — | bus-north-lv-1 | cap/pv-curtailable |
| site-north-load-1 | `$NID` | `flexible_load` | 0.8 | — | bus-north-lv-1 | cap/load-reducible |
| site-north-evse-1 | `$NID` | `evse_ev_fleet` | 0.05 | — | bus-north-lv-1 | cap/evse-windowed |
| site-north-meter-1 | `$NID` | `site_meter` | 3.0 | — | pcc-north | cap/meter-pcc |
| site-south-bess-1 | `$SID` | `bess` | 1.0 | 2.0 | bus-south-lv-1 | cap/bess-default |
| site-south-pv-1 | `$SID` | `pv_inverter` | 2.5 | — | bus-south-lv-1 | cap/pv-curtailable |
| site-south-load-1 | `$SID` | `flexible_load` | 1.2 | — | bus-south-lv-1 | cap/load-reducible |
| site-south-evse-1 | `$SID` | `evse_ev_fleet` | 0.15 | — | bus-south-lv-1 | cap/evse-windowed |
| site-south-meter-1 | `$SID` | `site_meter` | 3.5 | — | pcc-south | cap/meter-pcc |

```sh
post_asset() {  # site_id name type rated energy bus cap
  curl -sS -X POST "$API/assets" -H "$AUTH" -H 'content-type: application/json' -d '{
    "site_id": "'"$1"'", "name": "'"$2"'", "asset_type": "'"$3"'",
    "vendor_model": "synthetic", "rated_power_mw": '"$4"',
    "rated_energy_mwh": '"${5:-null}"', "phase": "phase:abc",
    "bus_reference": "'"$6"'", "capability_profile": "'"$7"'"
  }' | jq -r .data.id
}
ASSET_site_north_bess_1=$(post_asset "$NID" site-north-bess-1 bess 2.0 4.0 bus-north-lv-1 cap/bess-default)
ASSET_site_north_pv_1=$(post_asset   "$NID" site-north-pv-1 pv_inverter 1.5 "" bus-north-lv-1 cap/pv-curtailable)
ASSET_site_north_load_1=$(post_asset "$NID" site-north-load-1 flexible_load 0.8 "" bus-north-lv-1 cap/load-reducible)
ASSET_site_north_evse_1=$(post_asset "$NID" site-north-evse-1 evse_ev_fleet 0.05 "" bus-north-lv-1 cap/evse-windowed)
ASSET_site_north_meter_1=$(post_asset "$NID" site-north-meter-1 site_meter 3.0 "" pcc-north cap/meter-pcc)
ASSET_site_south_bess_1=$(post_asset "$SID" site-south-bess-1 bess 1.0 2.0 bus-south-lv-1 cap/bess-default)
ASSET_site_south_pv_1=$(post_asset   "$SID" site-south-pv-1 pv_inverter 2.5 "" bus-south-lv-1 cap/pv-curtailable)
ASSET_site_south_load_1=$(post_asset "$SID" site-south-load-1 flexible_load 1.2 "" bus-south-lv-1 cap/load-reducible)
ASSET_site_south_evse_1=$(post_asset "$SID" site-south-evse-1 evse_ev_fleet 0.15 "" bus-south-lv-1 cap/evse-windowed)
ASSET_site_south_meter_1=$(post_asset "$SID" site-south-meter-1 site_meter 3.5 "" pcc-south cap/meter-pcc)
```

Record the key → UUID mapping in the demonstration record (it is the
registration record P2.25 requires; the mapping scenario-key → registry ID
exists ONLY here, never in the scenario file — P2.13 §1).

## 3. Expected counts (the gate's arithmetic, faults disabled)

1440 ticks; per (asset, quantity) exactly 1440 records:

| Asset type | Quantities | Records/asset |
|---|---|---|
| BESS | active_power, reactive_power, soc, availability, operating_mode, health | 8 640 |
| PV | active_power, reactive_power, availability | 4 320 |
| Flexible load | active_power, baseline_reference | 2 880 |
| EVSE | active_power, soc, energy, connection_state | 5 760 |
| Site meter | active_power, reactive_power | 2 880 |

Total: **48 960 records**. With the fault plan enabled, the ONLY coverage
change is the drop window (`site-north-bess-1` 02:00–02:10 UTC → 10 ticks
per quantity covered by synthesized MISSING markers); all other fault
windows still store one value per tick — their expected quality states are
in the scenario's `expected_quality` section.

## 4. Run the simulator for the 24-hour horizon and ingest (P2.25 step 3)

```sh
# The ingestion path runs in the worker (step 0 wiring required); the
# worker's TelemetryQuery is configured to the ten registered asset UUIDs
# and the horizon 2026-06-01T00:00:00Z .. +24 h at the 1-minute cadence.
cargo run -p flexcore-worker
```

Record: wall-clock start/end, ingestion rate observed (an observation,
never a target — §19.4), and the quarantine count (0 expected with faults
disabled). The scenario clock vs wall clock note (P0.09 §5): a run against
a wall clock before the scenario start is a legitimate backtest-shaped run.

## 5. Verify coverage and canonical values (P2.25 step 4)

Per-asset, per-quantity counts (expect 1440 everywhere; run per asset UUID):

```sql
SELECT quantity, count(*) AS records,
       count(*) FILTER (WHERE quality = 'GOOD') AS good,
       count(*) FILTER (WHERE quality = 'MISSING') AS missing_markers
FROM telemetry
WHERE asset_id = '<ASSET_UUID>' AND tenant_id = '<TENANT_UUID>'
  AND event_time >= '2026-06-01T00:00:00Z' AND event_time < '2026-06-02T00:00:00Z'
GROUP BY quantity ORDER BY quantity;
```

Canonical spot-checks over the stored rows (all must return zero rows):

```sql
-- exactly the six canonical quality states (P0.08)
SELECT count(*) FROM telemetry WHERE quality::text NOT IN
  ('GOOD','SUSPECT','BAD','MISSING','ESTIMATED','STALE');
-- units follow the frozen quantity map (P2.04 §4); no kW/kWh anywhere
SELECT count(*) FROM telemetry WHERE (quantity, unit) NOT IN
  (('active_power','MW'),('reactive_power','MVAr'),('soc','1'),('energy','MWh'),
   ('availability','1'),('operating_mode','1'),('health','1'),
   ('connection_state','1'),('baseline_reference','MW'));
-- SOC is a fraction in [0,1] (P0.07 §4)
SELECT count(*) FROM telemetry WHERE quantity = 'soc' AND (value_num < 0 OR value_num > 1);
-- exactly one value leg, none exactly for MISSING (P2.04 §3 totality)
SELECT count(*) FROM telemetry WHERE
  ((value_num IS NOT NULL)::int + (value_text IS NOT NULL)::int + (value_bool IS NOT NULL)::int)
  <> CASE WHEN quality = 'MISSING' THEN 0 ELSE 1 END;
```

Tooling: the coverage accounting (expected-gap vs unexplained-gap) and the
shape/count checks are automated in the test suites — run with a database:

```sh
FLEXCORE_TEST_DATABASE_URL=... cargo test -p flexcore-control-plane --test exit_portfolio
FLEXCORE_TEST_DATABASE_URL=... cargo test -p flexcore-postgres-adapter --test corrections
```

## 6. Deterministic replay (P2.24) and the as-of demo (P2.31)

```sh
# Replay v2 against the stored-run reference (records file from a `run --out`).
cargo run -p flexcore-sim-runner -- run --scenario scenarios/synthetic-portfolio/scenario-v2.json --out /tmp/reference.json
cargo run -p flexcore-sim-runner -- replay --scenario scenarios/synthetic-portfolio/scenario-v2.json --reference /tmp/reference.json --code-version <code-sha>
# Expect: verdict "pass", zero mismatches, exit code 0.

# As-of demo: correct one stored record (append-only, reason + actor), then
# show T1 (pre-correction) vs T2 (corrected) from the API:
psql "$FLEXCORE_DATABASE_URL" -c "INSERT INTO telemetry
  (tenant_id, asset_id, quantity, event_time, ingest_time, source_sequence, source_id,
   quality, source_quality_code, value_num, value_text, value_bool, unit, q_sign_convention,
   clock_quality, telemetry_version, correction_of, correction_reason, corrected_by, corrected_at)
SELECT tenant_id, asset_id, quantity, event_time, now(), source_sequence, source_id,
       'GOOD', 'CORRECTED', 2.0, NULL, NULL, 'MW', q_sign_convention,
       clock_quality, 2, telemetry_id, 'simulated exit-gate correction', 'operator:exit-gate-demo', now()
FROM telemetry WHERE asset_id = '<ASSET_UUID>' AND event_time = '<EVENT_TS>'
  AND source_sequence = 1 AND quantity = 'active_power';"
curl -sS "$API/telemetry/query?asset=<ASSET_UUID>&from=<EVENT_TS>&until=<EVENT_TS>&as_of=<PRE_CORRECTION_TS>" -H "$AUTH"   # original value
curl -sS "$API/telemetry/query?asset=<ASSET_UUID>&from=<EVENT_TS>&until=<EVENT_TS>" -H "$AUTH"                            # corrected value

# As-of replay (the records-file entrypoint): export the versioned history
# (canonical fields + correction metadata), then compare the decision input.
psql "$FLEXCORE_DATABASE_URL" -At -c \
  "SELECT json_agg(row_to_json(t)) FROM (SELECT asset_id::text, (extract(epoch from event_time)*1000)::bigint AS event_time_unix_ms, quantity, value_num, value_bool, value_text, quality::text, source_sequence, corrected_at_unix_ms FROM ...) t" \
  > /tmp/versions.json
cargo run -p flexcore-sim-runner -- as-of --versions /tmp/versions.json --as-of <T1_UNIX_MS> --decision-input /tmp/decision-input.json
# Expect: pass at the pre-correction as-of; fail with a value diff at T2/latest.
```

## 7. Record the run (P2.26 input)

Record in the demonstration record: commands + outputs of steps 1–6, the
key → UUID registration table, observed counts vs the step-3 table, observed
ingestion rate and query latency (observations only), and the versions:
toolchain, PostgreSQL, NATS, scenario **v2**, seed 20260601, code version.
A second person re-runs this runbook on a clean checkout (P2.26
verification method).
