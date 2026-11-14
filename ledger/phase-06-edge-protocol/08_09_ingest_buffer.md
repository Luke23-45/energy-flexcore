# P6.08 + P6.09 — Edge telemetry ingestion read path; telemetry buffering, store-and-forward, and recovery

Date: 2026-11-13. Status: **IMPLEMENTED — awaiting human build/test** (wave 2b
of Phase 6; the loop rules in `ledger/phase-06-edge-protocol/ENTRY_NOTE.md`
forbid building, running tests, and installing: everything below is written
code, never executed). Report trace: §18.1, §10.2–§10.4, §18.3, §21.2,
§15.4, §6.4, §2.3. Ledger: P6.08, P6.09.

Dependency context (recorded honestly): P6.07 (the outbound mTLS channel)
landed **concurrently** with this wave (`crates/edge-agent/src/uplink_mtls.rs`
exists in-tree, written, UNBUILT). The read path (P6.08) and the buffer
(P6.09) are composed against the P6.05 PORTS (`UplinkChannel`,
`LocalBuffer`, `EmsSource`) exactly as specified — the composition point
where the P6.07 channel replaces `FailClosedUplink` in the binary
composition root is `main.rs`/wave-coordinator territory and is NOT touched
by this wave (file ownership). The read path composes at the OTHER end
(`EmsSource`) and does not interact with the channel.

---

## 1. What this record decides

1. **The read path exists and is composable** — `crates/edge-agent/src/
   ingest.rs` provides `AdapterIngestSource`: any P6.12 `EmsAdapter` + the
   P6.12 `AntiCorruptionLayer` behind the P6.05 `EmsSource` port. The flow
   is exactly the ledger's: `EmsSource.read` (adapter) → ACL mapping →
   `EdgeTelemetrySample` → `UplinkChannel` forward, with the runner's
   poll→buffer→forward loop unchanged in shape.
2. **The durable buffer exists** — `crates/edge-agent/src/buffer_file.rs`
   provides `FileLocalBuffer`, a journal-backed `LocalBuffer` (append-only
   CRC-checked records, truncate-and-audit recovery, audited drop-oldest
   overflow, age retention, at-least-once commit semantics). The in-memory
   buffer stays for tests and is explicitly volatile (unchanged).
3. **A TEST-only executable EMS source exists** — `crates/edge-agent/src/
   adapters/simulated.rs` (`SimulatedEmsSource`, registry id
   `simulated-test`, new `AdapterStatus::TestOnly`). Deterministic,
   scenario-fixture-driven, in-process. It is NEVER a real EMS and never
   presentable as one; the id names itself and every sample's `source_id`
   carries it.
4. **The runner gained exactly one new call** — `buffer.commit_forwarded
   (batch)` after a confirmed forward (a defaulted no-op port method; a
   journal compaction for the durable buffer). Everything else in
   `runner.rs` is additive documentation.
5. **Configuration knobs recorded** (`config.rs`, additive): `FLEXCORE_EDGE_
   DEDUP_WINDOW` (default 4 096), `FLEXCORE_EDGE_BUFFER_MAX_AGE_MS`
   (default 1 h, cap 24 h), `FLEXCORE_EDGE_OVERFLOW_POLICY` (exactly one
   accepted value, `drop-oldest-audited`; anything else is a named startup
   error).

## 2. P6.08 — the read path, per requirement

| Ledger requirement | Implementation | Where |
|---|---|---|
| Read mapping: canonical units, explicit sign, SOC [0,1] | unchanged — the P6.12 ACL (`NativeUnit` scale tables, `NativeSign` negation, `from_percent` boundary); the read path ADDS no conversion | `adapters/acl.rs` (existing), composed in `ingest::AdapterIngestSource::new` |
| `event_time` / `ingest_time` distinct, source-declared vs edge-stamped | the ACL maps the native timestamp to `event_time` (UTC-or-refused, P0.09); the read path stamps `ingest_time` from the edge `Clock` at the poll that received the readings — two distinct contract types, `ingest >= event` enforced by the sample constructor | `ingest.rs` `poll` |
| Exactly one quality state, never upgraded, never derived | quality passes through untouched from the ACL (which maps or refuses); an ACL rejection produces NO sample; no default quality anywhere on the path; a degraded source value (`OUT_OF_RANGE` fixture) arrives `SUSPECT` with the original code retained | ACL table + `ingest.rs` (passthrough only) |
| Source sequence, source identity | carried from the reading; `source_id` = edge prefix + point — the reserved `flexcore:` namespace is a construction-time rejection (P2.04 D-T6, cited from the ACL) | `adapters/acl.rs` `map_reading`, `ports.rs` sample constructor |
| Idempotency keys so reconnect duplicates are tolerated (§6.3) | the edge dedup window (below) + the recorded at-least-once posture (§4 of this record) | `ingest.rs` `EdgeDedupWindow`, `buffer_file.rs` commit semantics |
| Per-poll error accounting | every whole-reading rejection is a named counter in `IngestStats` (`read_failures`, `acl_rejections`, `duplicates_skipped`, `conflicting_duplicates_skipped`, `hard_poll_failures`, `last_failure_reason`) — queryable for P6.17 | `ingest.rs` |
| Poll cadence from config | unchanged — `FLEXCORE_EDGE_POLL_INTERVAL_MS` → `RunnerParams::from_config` (P6.05) | `runner.rs` |

**Per-poll policy (recorded decision).** A per-point failure rejects that
WHOLE reading (the P2.11 step-3 no-partial-record rule at the edge) and is
counted — never silently reduced output. A poll where EVERY point failed,
or where the adapter reports `NotConnected`, is a HARD poll failure
(`Err(SourcePollFailed)`), so the runner audits `EDGE_SOURCE_POLL_FAILED`;
zero output with failures is never reported as a clean empty poll. One
broken channel must not starve ingestion of the healthy ones — that is the
deliberate divergence from a strict fail-wholesale poll, and the counters
make the reduction non-silent.

## 3. P6.09 — buffering, store-and-forward, recovery, per requirement

**Journal format (std-only, no new dependencies — the wave brief allowed
Cargo.toml additions but none were needed).** Fixed 32-byte header
(`FLEXCORE-EDGE-TELEMETRY-JOURNAL` + format version 0x01), then
length-prefixed CRC-32-checked records. The payload is a manual encoding of
`EdgeTelemetrySample` field-for-field (quantity/value discriminants,
canonical quality byte, both P0.09 timestamps as their own millisecond
values, strings length-prefixed) and decodes back through the sample's
VALIDATED constructor — a corrupt-but-decodable record cannot produce an
invalid sample silently. A bitwise std-only CRC-32 (IEEE) is used for
integrity; no checksum crate was added.

| Ledger requirement | Implementation |
|---|---|
| Capacity (records), persistence across restarts, ordering | `FileLocalBuffer::open(path, capacity, max_age, clock)` replays the journal into an order-preserving queue; appends are disk-first (`sync_data` per record) |
| Encryption at rest (§18.3) | **NOT implemented** — recorded open item (§6); closing it needs the P6.02-audited crypto dependency, which this wave was not permitted to add |
| Per-asset ordering by source sequence | the journal preserves arrival order (append-only); the canonical `source_sequence` travels inside every record; per-asset re-sequencing downstream remains the platform's job (P2.04/P2.08 discipline — cited, not reinvented) |
| Explicit overflow policy | **audited drop-of-OLDEST**: on an append at capacity the oldest record is removed, a hard counter increments (`dropped_oldest`), a latched health flag sets (`has_overflowed`), and the drop is logged — never a silent overwrite. Backpressure (the runner pausing polling at a full buffer) remains the FIRST line; drop-oldest engages only when the bound is hit anyway (e.g. a reopened journal larger than the configured capacity). The policy is a CONFIGURATION value (`drop-oldest-audited`) — the only accepted one; an unknown value refuses startup |
| Max buffer age | `FLEXCORE_EDGE_BUFFER_MAX_AGE_MS` (default 1 h, cap 24 h), measured on `ingest_time` (the edge's own receipt time); expired records are dropped at drain/reopen with their own audited counter (`dropped_expired`) |
| Store-and-forward, ordered drain, second outage mid-drain | drain → forward → on failure `prepend` (order preserved, journal untouched — no loss); a mid-drain channel outage therefore loses nothing; recovery resumes the drain in order |
| Mark forwarded records / crash mid-drain | the P6.09 commit point: the journal keeps drained records until the runner confirms the forward (`commit_forwarded` → temp-file + rename compaction). A crash in the drain→forward→commit window re-delivers after restart — **at-least-once by design** (§4) |
| Gap/loss event on drops | every drop path (overflow, expiry, corrupt tail) increments a named counter and logs loudly; the affected intervals downstream are the platform gap detector's `MISSING` markers (P2.04 D-T6 / P2.08 — cited; the edge emits no gap markers itself) |
| Corrupt-tail recovery | first malformed frame (bad length / CRC mismatch / undecodable payload) ends the replay, the file is TRUNCATED at the last good record, the loss is counted (`corrupt_tail_truncated`) and logged — truncate-and-audit, never silent, never guessed past. A wrong magic/version is a hard open error |
| Compaction atomicity | the commit rewrite goes to a sibling temp file, synced, then RENAMED over the journal — a crash mid-compaction cannot destroy the previous journal |

## 4. The at-least-once / deduplication honesty record

| Layer | Mechanism | Guarantee | Honest boundary |
|---|---|---|---|
| Edge dedup window (`ingest.rs`) | identity = (source identity, `event_time`); exact replay → counted skip; same identity, different content → counted refusal (the P4.24 fixture-(a)/(b) rules, cited not reinvented) | kills poll-cadence re-reads (the simulated/EMS source re-serves a tick until advanced) | BOUNDED window (default 4 096): a duplicate arriving after eviction is NOT caught at the edge |
| Durable buffer (`buffer_file.rs`) | drain keeps records on disk until the confirmed-forward commit | ZERO loss within capacity across outages AND crashes | the crash window produces a RE-DELIVERY (at-least-once): downstream duplicates are possible and expected |
| Platform ingest (existing, cited) | P4.24-style idempotent telemetry ingest (P2.09/P2.10 duplicate classification: exact duplicate acknowledged, conflicting duplicate quarantined) | the system's actual idempotency boundary | consumes both the out-of-window edge duplicates and the crash-window redeliveries — the edge does NOT reinvent an idempotency store |

Acceptance-criterion mapping (ledger P6.08: "duplicate delivery after
reconnect produces one stored value"; P6.09: "drain completes without
duplicates downstream"): the single-stored-value guarantee is OWNED by the
platform ingest layer (Phase 2, already specified and tested there), not
re-implemented at the edge. The edge contributes (a) the poll-cadence dedup
window and (b) the ordered at-least-once redelivery. This division is the
recorded decision; a fully edge-side exactly-once store would duplicate the
platform's idempotency machinery and add a second identity namespace —
rejected.

## 5. Decisions (DEFAULT-labelled where recommended, not decided)

| # | Decision | Status |
|---|---|---|
| D-1 | Overflow policy = drop-oldest-audited (counter + latched health flag + log); alternatives (stop-accepting) NOT implemented | DEFAULT — the ledger's own example policy; changing it is a reviewed change (the config knob refuses unknown values) |
| D-2 | Overflow bound measured in RECORDS only this wave (a byte bound is future work — recorded in §6) | DEFAULT |
| D-3 | Age measured on `ingest_time` (edge receipt), not `event_time` (source event) | DEFAULT — retention is about the edge's own storage age; the distinction is recorded |
| D-4 | Edge dedup identity = (source identity, `event_time`); content conflict refused deterministically (P4.24 fixture-(b)) | DEFAULT — mirrors the P4.24 key-covering-identity rule |
| D-5 | Per-poll accounting: partial success is `Ok` with counted reductions; all-points-failure / `NotConnected` is `Err` | DEFAULT — recorded in §2 |
| D-6 | Journal = std-only CRC-32 length-prefixed binary; no serde/json dependency added | DEFAULT (wave constraint: no new deps needed) |
| D-7 | Encryption at rest deferred to the P6.02-audited crypto dependency | OPEN ITEM (§6) |
| D-8 | `simulated-test` registered `TestOnly`; production composition roots must not wire it for a real site (P6.25 must surface simulated values distinctly) | RECORDED boundary |

## 6. Honest limitations and open items

| Item | Status |
|---|---|
| Code + tests | WRITTEN; **not built, tests NOT run** (`cargo build/fmt/clippy/test` banned this wave — ENTRY_NOTE). The ticks read "implemented, awaiting human build/test" |
| Encryption at rest (§18.3) | NOT implemented (D-7). Until closed, the journal holds canonical samples in the clear on the edge host — the deployment profile (P6.20/D6) must treat the buffer path as in-scope for site protection |
| Audited counters (`dropped_oldest`, `dropped_expired`, `corrupt_tail_truncated`) | in-memory per-open; a restart loses the counts (the journal loss itself is still audited at the moment it happens, and the health flag latches per instance). Persisting counters is a P6.17 candidate |
| Corruption semantics | everything from the FIRST corrupt frame is truncated (no resync is possible without inter-record markers); the loss is counted, never silent |
| P6.09 step-4 "sequence discontinuity detection" | the gap-event mechanism is implemented for DROPS (overflow/expiry/corruption); source-sequence discontinuity detection remains the platform gap detector's job (P2.04/P2.08, cited) — the edge does not duplicate it |
| `main.rs` composition | still the honest fail-closed composition of P6.05 (FailClosedEmsSource / FailClosedUplink / in-memory buffer, all logged). Wiring the read path + durable buffer + P6.07 channel into the BINARY composition root is deliberately left to the coordinator/P6.10 wave (file ownership); the library-level composition is fully exercised by the new integration tests |
| Concurrency note | P6.06/P6.07 landed additively in `config.rs`/`lib.rs`/`adapters/mod.rs`/`ports.rs` during this wave; this wave's edits to those files are strictly additive and disjoint from the identity/channel blocks |
| OpenEMS | nothing OpenEMS-specific is encoded (P6.10 owns it against the VERIFIED API); the read path is protocol-agnostic by construction |

## 7. Files touched (this wave)

```text
crates/edge-agent/src/ingest.rs                    NEW   P6.08 read path + dedup window + stats
crates/edge-agent/src/buffer_file.rs               NEW   P6.09 durable journal buffer
crates/edge-agent/src/adapters/simulated.rs        NEW   P6.08 TEST-only EMS fixture
crates/edge-agent/src/adapters/registry.rs         edit  + AdapterStatus::TestOnly, simulated-test allowlist entry, tests
crates/edge-agent/src/adapters/mod.rs              edit  + simulated module (additive)
crates/edge-agent/src/ports.rs                     edit  + LocalBuffer::commit_forwarded (defaulted no-op), append doc
crates/edge-agent/src/runner.rs                    edit  + commit_forwarded call after confirmed forward, doc note
crates/edge-agent/src/config.rs                    edit  + dedup-window / max-age / overflow-policy knobs (additive)
crates/edge-agent/src/lib.rs                       edit  + ingest / buffer_file modules, module-map rows
crates/edge-agent/tests/ingest_p6_08.rs            NEW   P6.08 integration tests (UNRUN)
crates/edge-agent/tests/buffer_p6_09.rs            NEW   P6.09 integration tests (UNRUN)
ledger/phase-06-edge-protocol/ledger_phase_6.md    edit  P6.08 + P6.09 ticks
ledger/phase-06-edge-protocol/HANDOVER.md          edit  wave 2b row
```
