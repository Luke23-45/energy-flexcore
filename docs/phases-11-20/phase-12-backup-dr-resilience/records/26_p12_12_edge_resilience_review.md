# P12.12 — Edge-agent resilience review (offline windows, buffer bounds, clock-drift interplay, prolonged outage)

| Field | Value |
|---|---|
| Subtask | P12.12 (edge-agent resilience review: offline windows, buffer growth bounds, clock-drift policy interplay (P6.31 hold-on-drift), store-and-forward limits under prolonged outage) |
| Wave / agent | Phase 12 wave 1, Agent B |
| Status | **STATIC CODE-REVIEW ARTIFACT.** The findings below are results of reading written code cited by file and symbol; NOTHING was executed, no outage was simulated, no buffer grew, no clock drifted. Every finding is a property of the WRITTEN code (which is itself WRITTEN-UNRUN — the first human build is the gate for even compile-time confirmation). No finding claims observed behavior. |
| Sources | `crates/edge-agent/src/buffer_file.rs` (P6.09 telemetry journal), `crates/edge-agent/src/audit_buffer.rs` (P6.18 audit journal), `crates/edge-agent/src/runner.rs` (P6.05 supervision loop), `crates/edge-agent/src/clock_quality.rs` (P6.31 hold-on-drift), `crates/edge-agent/src/config.rs` (bounded defaults), `crates/edge-agent/src/command_verify.rs` (P6.15 expiry seam, cited via P9.24 E-5), P9.24 §2.2/§6 and its R-P9.24-5 (cited, not re-derived) |
| Files touched | none (this record is the deliverable) |

## 1. Buffer growth bounds (as written)

| Bound | Value (written code) | Citation |
|---|---|---|
| Telemetry buffer capacity | default 10 000 samples, config-bounded to 1..=1 000 000; capacity 0 refused (`assert!` + config validation) | `config.rs` (ENV_BUFFER_CAPACITY), `buffer_file.rs :: open` |
| Telemetry record size cap | `MAX_RECORD_BYTES = 1 << 20` (1 MiB) per record — a record above the cap is a framing error, never an append | `buffer_file.rs` const |
| Telemetry age retention | `max_age` on `ingest_time`; default 3 600 000 ms (1 h), config-bounded to 1..=86 400 000 ms; `None` = unlimited is a choice the composition must make explicitly | `config.rs` (ENV_BUFFER_MAX_AGE_MS), `buffer_file.rs :: open` doc |
| Audit buffer capacity | bounded by construction (capacity 0 refused at open); the capacity value is a composition parameter, not an env default in `config.rs` — the composition owns the number | `audit_buffer.rs :: open` |
| Audit record size cap | `MAX_RECORD_BYTES = 1 << 16` (64 KiB) | `audit_buffer.rs` const |
| Overflow policy (both journals) | audited drop-of-OLDEST: the oldest record is dropped (`dropped_oldest` counter), the `has_overflowed` health flag LATCHES, the drop is logged — never a silent overwrite, never unbounded growth | both modules' overflow paths |
| Per-record framing | fixed magic header + version byte; `[u32 LE len][u32 LE crc32][payload]`; strict decode; truncate-at-first-corrupt recovery, counted (`corrupt_tail_truncated`) | both modules |

**Derived static bound (hand-computed from the written constants, not measured):** with the shipped defaults, the telemetry journal's worst-case size is capacity × (8 framing bytes + payload ≤ 1 MiB) — for capacity 10 000 an upper bound of ~10 GiB only if every record were 1 MiB, which the supervisory telemetry payloads (small JSON samples) do not approach in the written encoders; the REAL operating bound is capacity × typical-record-size, a value no written artifact states and this review does not invent. The config bound of 1 000 000 samples × 1 MiB is the absolute ceiling the configuration permits, ~1 TiB — recorded as a FINDING (F-P12.12-1): the configured ceiling allows a journal size no disk plan has been written for; sizing against the site's outage-duration × telemetry-rate assumption is the recorded P9.18/P9.39 obligation (`deployment-upgrade-model.md` §4 buffer-bounds row).

## 2. Store-and-forward limits under a prolonged outage (as written)

1. **The loop's degradation ladder** (`runner.rs`): forward failure →
   DEGRADED (keep polling + buffering, batch re-prepended IN ORDER);
   buffer full → polling PAUSED (audited backpressure — `poll_paused_backpressure`,
   backpressure REPLACES loss in the loop); successful forward while
   degraded → recovery, buffer drains, `DegradedBufferingExited` audited.
2. **Two independent loss windows exist, both audited and counted** (the
   P9.24 R-P9.24-5 finding, confirmed by this review against the code):
   - the CAPACITY window: beyond capacity, the loop pauses polling — but
     the journal's own append path still applies drop-of-OLDEST if an
     append would exceed capacity (and `enforce_capacity` applies it to a
     reopened journal larger than the configured bound, `buffer_file.rs`
     load path); anything dropped is counted + latched, never silent;
   - the AGE window: samples older than `max_age` (default 1 h, ceiling
     24 h) are dropped (`dropped_expired`) — under an outage LONGER than
     `max_age`, the oldest buffered telemetry ages out even though the
     capacity bound was never reached. The effective retention window is
     therefore min(capacity-limited window, age window) — a DERIVABLE
     property of the two written bounds, recorded here as the review's
     summary finding (F-P12.12-2), never a measured one.
3. **The audit journal is a SEPARATE, smaller loss window** (P6.18):
   same drop-of-oldest + latch discipline, 64 KiB record cap, its own
   capacity; audit sequences are monotonic and PERSIST in the journal;
   upload is in-order at-least-once with the fail-closed `commit_uploaded`
   sequence check (an out-of-order or stale commit is refused, nothing
   marked uploaded that was not). The central-outage posture this produces:
   audit evidence survives within the audit buffer's bound and the OLDEST
   audit records are the counted loss beyond it.
4. **Restart behavior** (written): the journal is disk-first (append
   before forward — one code path, no fast path), so unforwarded records
   survive a restart; the drop/latch counters are IN-MEMORY AND RESET PER
   OPEN — recorded in the code as a limitation and carried here as
   F-P12.12-3: after a restart, "how much was dropped historically" is
   operator-visible state only if the composition preserves it; the
   journal contents themselves are the durable evidence.
5. **Exactly-once drain is NOT claimed** — the written contract is
   at-least-once, in-order, with the deduplication window (default 4 096
   samples, `config.rs`) and the platform-side idempotency layers
   (P1.16/P11.12) as the duplicate neutralizers. The E-4 rehearsal step
   (P9.24 §6) is where the exactly-once OBSERVATION would be made, if it
   holds.

## 3. Clock-drift policy interplay (P6.31 hold-on-drift) — the review's central finding

The written pieces compose as follows (each step cites its code):

1. **No NTP client is written** (`clock_quality.rs` header): offset
   estimation uses ONLY the platform's event_time-vs-ingest_time honesty
   pairs; sync-source reachability is a DEPLOYMENT REQUIREMENT.
2. **Quality derivation** (`quality_at`): no observation ever → `Unknown`;
   the last sync observation older than `sync_timeout_millis` (defaults:
   lab 60 000 ms, pilot 30 000 ms — `clock_quality.rs` consts) →
   `Unsynchronized`; otherwise the last derived state. A pair whose
   absolute offset exceeds `MAX_ABS_OFFSET_MILLIS` (86 400 000 ms — one
   day) is refused as a corrupt pair, never folded into the median-of-
   window estimate.
3. **The hold-on-drift gate** (`expiry_check_allowed`): `Ok(())` only on
   `Synchronized`; any other state holds commands, and the P6.15 verifier
   fails the expiry check closed on the same rule (E-5's recovery-path
   criterion, P9.24 §6). Tolerances: lab 500 ms, pilot 100 ms (consts).
4. **THE INTERPLAY (F-P12.12-4, the review's summary finding):** during a
   prolonged CENTRAL outage, the edge's sync source (the platform's time
   pairs) goes quiet; after `sync_timeout_millis` the quality degrades to
   `Unsynchronized` and the edge HOLDS COMMANDS (fail-closed expiry) while
   telemetry store-and-forward CONTINUES within the §2 bounds. The written
   posture is therefore: data capture survives the outage; the command
   path degrades to hold — which is the safe direction for a supervisory
   layer (no command can be validated for expiry on an untrusted clock),
   and it is exactly the posture P9.24 §6 E-5 asserts on recovery (zero
   stale/expired/replayed commands issued). This finding is a COMPOSITION
   of written rules, not an observed behavior.
5. **Health seam**: `Unknown` renders `DEGRADED_UNVERIFIED` (never
   healthy-by-default), `Degraded`/`Unsynchronized` render
   `DEGRADED_CLOCK_DRIFT`, only `Synchronized` renders `HEALTHY` — the
   closed health vocabulary gains `CLOCK_DRIFT` as the reviewed additive
   extension (the module's own record).

## 4. Findings register (static review; none is an observed defect)

| # | Finding | Type | Disposition path |
|---|---|---|---|
| F-P12.12-1 | The config-permitted buffer ceiling (1 000 000 samples × up to 1 MiB/record) admits journal sizes no disk plan addresses; the sizing obligation sits with the recorded P9.18/P9.39 assumption work | gap in deployment sizing, not in code | sizing at deployment per `deployment-upgrade-model.md` §4 |
| F-P12.12-2 | The effective offline retention window is min(capacity window, `max_age` window) — two bounds compose; an operator sizing only the capacity bound under-sizes the real window | documentation/composition finding | recorded here; the composition must set `max_age` against the same outage-duration assumption |
| F-P12.12-3 | Drop/latch counters reset per open (in-memory); historical overflow evidence lives only in the composition/log layer, not in the journal | recorded code limitation (the code says so itself) | carried to operator visibility design (P6.17 health) |
| F-P12.12-4 | Prolonged central outage → sync source quiet → `Unsynchronized` after `sync_timeout_millis` → command path holds (fail-closed) while telemetry buffers continue | intended composition of written rules (safe direction) | rehearsal evidence via P9.24 §6 E-1..E-6 when run |
| F-P12.12-5 | The `has_overflowed` latch is one-way in-memory: once latched it stays latched until restart; a recovered edge reads as overflowed even if the buffer drained — honest but coarse | behavior of written code | acceptable as written; noted for health-signal refinement |

## 5. Honest boundaries

- Everything above is a property of WRITTEN code, which is itself
  WRITTEN-UNRUN; even the "derived static bound" is paper arithmetic on
  written constants, never a measurement.
- No resilience guarantee is stated or implied: §13.3 semantics — zero
  observed violations from finite tests would be a test outcome, never a
  guarantee — and this review ran no tests.
- The edge is a supervisory component; nothing here touches
  protection-grade behavior, and the central-outage rule
  (`deployment-upgrade-model.md` §5) stays the authority for site-local
  safety independence (cited, not re-argued).
- ADR-010: the review adds no dispatch authority; the command path it
  describes holds commands on clock distrust — the opposite of authority
  expansion.

## 6. Handoff notes

1. **Wave 2 / P12.15**: findings F-P12.12-2 and F-P12.12-4 are inputs to
   the resilience acceptance-criteria record (what "resilient" may mean:
   bounded, audited, fail-closed — not loss-free, not guarantee-bearing).
2. **Human gate**: confirming these findings at runtime is exactly the
   E-1..E-6 edge-outage test (P9.24 §6) — a named human gate, UNRUN.
3. **Reconciliation**: Agent A's P12.05/P12.02 records may add the
   central-store recovery context; the interplay finding composes with,
   and does not modify, their designs.
