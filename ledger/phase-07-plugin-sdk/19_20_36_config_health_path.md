# P7.19 + P7.20 + P7.36 — Plugin configuration, health/readiness, and plugin-path protection: IMPLEMENTED (awaiting human build/test)

Date: 2026-11-13. Subtasks: P7.19, P7.20, P7.36 of Phase 7
(`ledger_phase_7.md`); P7.36 is the plugin-path leg of P7.42
(determinism) and touches the P7.34 (mTLS trust boundary) and P7.25
(console) seams as RECORDED seams only. Owner roles: Security + Rust
engineer (P7.19), Rust engineer (P7.20), Security engineer (P7.36).
Depends on: P7.19 ← P7.05, P7.07 (both landed); P7.20 ← P7.07, P7.17
(P7.17 is the concurrent agent's crate — this wave touches NOTHING in
`python/workers/**` and only RECORDS the worker-health handoff); P7.36 ←
P7.11, P7.12, P7.18 (all landed or recorded). Report trace: §5.5; §5.6;
§5.7; §9.2; §13.4; §14.3; §15.4–§15.5; §17.3–§17.5; §21.3; §21.4;
ADR-005/ADR-010.

**Run-status honesty (AGENTS.md rule 7; the loop bans carried over from
Phase 6 and the P7.05/P7.06 record):** the code and tests in this wave are
WRITTEN, NOT BUILT and NOT RUN. No `cargo` command of any kind was executed
(no build, no check, no test, no clippy, no fmt). Every test below is UNRUN
until the human build/test pass executes it. Nothing in this document
claims a test passed, a build succeeded, or a compile completed. The only
tools run were the three permitted verification scripts (§11, output
quoted verbatim).

## 1. Files (complete list; nothing outside it was touched)

| File | Status |
|---|---|
| `crates/flexcore-kernel/src/plugin_config.rs` | NEW (P7.19) |
| `crates/flexcore-kernel/src/plugin_path_protection.rs` | NEW (P7.36) |
| `crates/flexcore-kernel/tests/plugin_config_p7_19.rs` | NEW (P7.19 tests) |
| `crates/flexcore-kernel/tests/plugin_path_protection_p7_36.rs` | NEW (P7.36 tests) |
| `apps/plugin-host/src/health.rs` | NEW (P7.20) |
| `apps/plugin-host/tests/health_p7_20.rs` | NEW (P7.20 tests) |
| `crates/flexcore-kernel/src/lib.rs` | EDITED (ADDITIVE ONLY: two module-table rows + two `pub mod` lines; no existing line reordered or changed) |
| `apps/plugin-host/src/lib.rs` | EDITED (ADDITIVE ONLY: one module-table row + one `pub mod` line) |
| `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | EDITED (P7.19/P7.20/P7.36 ticks ONLY — P7.17/P7.18 belong to the concurrent agent and were not touched) |
| this file | NEW (decision record) |

No dependency was added to either crate (the kernel stays
`serde_json`-only via the existing workspace pin; the plugin host stays
std + tracing + the wasmtime pin). The root `Cargo.toml` and
`tools/check_dependency_rule.py` were NOT touched.

## 2. P7.19 — what was implemented

`crates/flexcore-kernel/src/plugin_config.rs`: the composition-side
configuration resolver for one plugin instance. The manifest (P7.05)
declares the SCHEMA; the operator supplies the VALUES; the module
validates, applies defaults EXPLICITLY, resolves secret REFERENCES to
handles, and emits the deterministic resolved view that the lifecycle's
`Compatible → Configured` gate (`lifecycle.rs::GATE_SCHEMA_SECRETS`)
consumes as its `GateOutcome` evidence.

1. **Schema validation (the P7.05 subset, reused unchanged):** the closed
   property set (`additionalProperties: false` is already enforced by the
   P7.05 validator, so an unknown property is a rejection by construction —
   and this module re-refuses over the resolved surface with
   `UNKNOWN_PROPERTY`), per-property types from the closed
   string|number|integer|boolean|array|object vocabulary
   (`CONFIG_WRONG_TYPE`; `integer` additionally requires a whole number),
   and the required list (`MISSING_REQUIRED`). Validation runs at
   configuration time — invalid config is refused with field-level reasons
   (ledger P7.19 acceptance), never deferred to first use.
2. **THE DOCUMENTED EXTENSION (D-P7.19-1):** the manifest subset
   (05_06 record §2.1) accepts only `{type, description}` per property —
   `enum`, `minimum`, `maximum`, `default` are NOT in it. The manifest
   validator was NOT widened (it is a reviewed baseline; no existing logic
   was touched). The extension is a LAYERED CONSTRAINTS DOCUMENT
   (`flexcore-plugin-config-constraints-v1`, parsed strictly in
   `plugin_config.rs`: closed key sets, named errors, reject-never-warn —
   the P7.05 stance): per property the manifest ALREADY declares and only
   as a NARROWING it adds `enum` (string properties),
   `minimum`/`maximum` (number/integer properties, inclusive), and
   `default` (any property). A constraint on an undeclared property is
   `CONSTRAINT_UNDECLARED`; a constraint of a mismatched type is
   `CONSTRAINT_TYPE_MISMATCH`; a default is validated against its own
   property's type AND constraints at parse time (a default that would be
   refused as a value is refused as a default), and a default may never be
   a secret reference. This keeps the manifest schema and the config
   validator from drifting (the explicit 05_06-record requirement) while
   providing the enum/bounds/defaults surface the ledger names.
3. **Defaults applied EXPLICITLY, never silent:** every default applied is
   recorded in `ResolvedConfig::defaults_applied()` (sorted) and
   `ResolvedConfig::is_default()` — the operator can always see which
   values were not supplied by hand (ledger risk: silent defaults).
   Required-with-default resolves instead of refusing; required-without-
   default refuses `MISSING_REQUIRED`.
4. **Secret references (handles only):** a config string value matching one
   of the closed `env:` / `file:` / `secret-store:` schemes is a REFERENCE
   and resolves to a `SecretRef` HANDLE (scheme + location + property —
   the same non-secret location string the manifest already carries in the
   clear). There is NO value field and NO read path anywhere on the type:
   resolution returns the reference; the READING is a host-side capability
   (§3 below). Form validation at configuration time: a scheme prefix with
   an empty location is `SECRET_REF_INVALID` — rejected at `Configured`,
   never at first use (ledger step 3's fail-early rule, to the depth the
   kernel can enforce). All three schemes resolve (pinned by tests).
5. **Secret-VALUE refusals, echoing nothing:** two closed rules:
   (a) the closed secret-field vocabulary (`SECRET_FIELD_VOCABULARY`:
   `password`, `secret`, `token`, `credential`, `api-key`, `private-key`,
   `access-key` — separator-normalised so `api_key`/`site.password` match)
   — a plain string value in a secret-shaped field is
   `SECRET_VALUE_IN_CONFIG`, an inline-credential attempt;
   (b) a config property named exactly like a manifest secret is
   `SECRET_FIELD_COLLISION` (secrets live in the manifest's `secrets`
   object). Neither error echoes the VALUE (the P7.05 rule, mirrored;
   pinned by a test that asserts the error text does not contain the
   planted credential).
6. **Config drift:** `resolve_for_instance` anchors resolution to the
   manifest version the RUNNING instance was instantiated against; a
   different manifest version is `CONFIG_DRIFT` — re-instantiation through
   drain-before-replace is required, never a silent re-resolution.
7. **Determinism (P7.42):** the resolved view is built over `BTreeMap`s
   only — property iteration, default application, error selection, and
   the JSON rendering are all sorted; input key order (config AND
   constraints) never leaks into an output. Tests pin resolved-view
   equality across runs and across reordered inputs.

## 3. The secret-reference seam (recorded, not implemented)

The kernel has NO secrets-manager port and must never grow one. The
division of authority is:

| Step | Owner | Status |
|---|---|---|
| Declare the reference (name + scheme-prefixed location) | the manifest (P7.05) / the config value (P7.19) | implemented |
| Validate the reference FORM (scheme + non-empty location) | the kernel (`plugin_config.rs`) | implemented |
| Resolve the reference to a handle | the kernel (`plugin_config.rs`) | implemented |
| Check reference EXISTENCE in the store at `Configured` | the HOST (secrets adapter wired at the composition root) | RECORDED SEAM — the kernel cannot check a store it does not own |
| READ the secret value | the HOST/adapter only — the broker's scoped-grant mechanism (P7.11) | RECORDED SEAM — `secret.read` is in the §5.7 never-granted set for PLUGINS: the host hands the adapter the value, the plugin receives a handle or nothing |
| Cache discipline | the host deployment | RECORDED: the ledger's "secret resolved once and cached in plaintext in plugin memory" risk is closed by the handle-only design — no value ever crosses the kernel→plugin boundary |

## 4. P7.20 — what was implemented

`apps/plugin-host/src/health.rs`: the host-side per-plugin health /
readiness monitor, mirroring the P6.17 edge health discipline
(`crates/edge-agent/src/health.rs` — cited, adapted, not reinvented).

1. **The closed vocabulary** (exactly; extending it is a reviewed change):
   `HEALTHY`, `DEGRADED_UNVERIFIED`, `DEGRADED_MEMORY_LIMIT`,
   `DEGRADED_FUEL_EXHAUSTED`, `DEGRADED_DEADLINE`, `DEGRADED_OUTPUT_LIMIT`,
   `DEGRADED_CALL_RATE`, `DEGRADED_ERROR_RATE`, `FAILING_UNRESPONSIVE`,
   `FAILING_BINDINGS`. It is consistent with the kernel lifecycle states
   (the monitor's verdict feeds the `Started → Ready` `GATE_HEALTH` gate —
   readiness is REPORTED here, the lifecycle enforces the gate) and with
   the P6.17 `HEALTHY` / `DEGRADED_<reason>` / `FAILING_<reason>` shape;
   only `FAILING_*` blocks dependent actions.
2. **The hard rule (mirrored):** health is NEVER inferred from the absence
   of errors. A registered plugin with no observation is
   `DEGRADED_UNVERIFIED` and stays there until the first observation lands;
   readiness is NEVER ready before the first observation. Pinned by the
   first test.
3. **Readiness (§5.5 `health.readiness` semantics; the P7.05-recorded
   seam):** the closed readiness vocabulary is `UNOBSERVED` /
   `PROBE_FAILED` / `READY`. READY exactly two ways: the FIRST successful
   call through the binding seam, or an explicit readiness probe through
   the binding-seam hook (`record_readiness_probe` — the P7.20 task's
   "explicit readiness probe through the binding seam"). A failed probe
   latches `PROBE_FAILED` (its own recorded verdict, not cleared by a later
   successful call — only a re-registration or a passed probe resets it).
4. **Liveness via the host's epoch/probe hooks:** a positive, timestamped
   rule on the LAST OBSERVED fact — `now − last_seen > unresponsive_after`
   (the detection window) is `FAILING_UNRESPONSIVE`, and a failed explicit
   liveness probe (`record_liveness_probe`, the host's epoch/probe hook)
   fails immediately. Silence produces FAILING only through the window on
   the last seen fact, never by default.
5. **Resource exhaustion → health degradation (the closed mapping):**
   `exhaustion_reason()` maps the `limits.rs`/`host.rs` enforcement errors
   to the closed reasons — `MemoryLimitExceeded` → `MEMORY_LIMIT`,
   `FuelExhausted` → `FUEL_EXHAUSTED`, `DeadlineExceeded` → `DEADLINE`,
   `OutputLimitExceeded` → `OUTPUT_LIMIT`, `CallRateExceeded` →
   `CALL_RATE`; `BindingsNotGenerated` (not an exhaustion) →
   `FAILING_BINDINGS`. A host error that maps to nothing is a NAMED
   refusal (`UNMAPPABLE_HOST_ERROR`) — never folded into healthy. An
   exhaustion DEGRADES and LATCHES until a later successful call clears it
   (the recovery rule, D-P7.20-2); the instance teardown is the limits
   layer's contract, not re-decided here.
6. **Audit and metrics hooks:** every transition is audited through the
   host-local `HealthAuditSink` port (defined HERE — this crate depends on
   no flexcore crate, the `broker.rs` §6 seam pattern; the kernel's
   `AuditSink` is mirrored in shape, not imported) via
   `audit_transitions(before_snapshot, now)` — transitions are derived by
   comparing assessments, so the monitor stays pure. Refused observations
   (unknown plugin) are audited too. Counters (calls, errors, timeouts,
   fuel used, last transition) live in the fixed-size `Copy` `PluginStats`
   — a BOUNDED, CLONABLE snapshot with no per-call record anywhere; the
   error-sample list feeding the error-rate derivation is pruned to the
   window and hard-capped (`MAX_ERROR_SAMPLES = 64`, oldest dropped and
   still counted). Solver duration for optimizer plugins rides the
   `fuel_used` counter's seam (recorded: per-call duration histogramming
   needs the Phase 1 metrics pipeline — §5 below).
7. **The observable snapshot:** `HealthSnapshot` (one `PluginHealthRecord`
   per registered plugin, ordered by plugin id) renders through
   `HealthSnapshot::json()` — the serializable snapshot method the P7.25
   console consumes. THE TRANSPORT IS A RECORDED SEAM, NOT IMPLEMENTED
   (§5). No secret, no config value, no raw backend text ever enters a
   snapshot or a log line (the P1.08 discipline; pinned by a test).

## 5. The recorded seams (NOT implemented here — on purpose)

| Seam | What is recorded | Where |
|---|---|---|
| P7.20 gRPC transport (host ↔ control plane) | The health snapshot and stats travel to the platform over the host's gRPC transport seam (`10_11_plugin_host_broker.md` §1: "the host↔control-plane gRPC transport is recorded as the P7.20 seam"). This module only produces the serializable snapshot; NO transport, client, or server is implemented | `health.rs` header; `HealthSnapshot::json` |
| P7.34 mTLS trust boundary | mTLS with short-lived certificates, rotation, and revocation for host ↔ control plane — INCLUDING the health/telemetry traffic (the ledger's risk: "mTLS added to the happy path but skipped for worker traffic") | recorded here and in `health.rs`; owned by P7.34 |
| P7.25 console | The Plugins & Integrations workspace consumes `HealthSnapshot`/`PluginStats`; the snapshot fields are the console's data contract (status, readiness, last-seen, counters, detail) | §4 item 7 |
| P7.11 broker = the secret reader | `plugin_config.rs` resolves references to HANDLES; the broker (scoped grants, `secret.read` never granted to plugins) is where a value is read | §3 |
| P7.18 worker health handoff | The P7.17/P7.18 supervision module owns worker-side health; this monitor owns Class B host-side health. The correlation fields (plugin id + version, tenant/site/schedule/command ids, trace id — §21.3) are the snapshot's `plugin_id`/`version` plus the caller's correlation context; full trace-ID correlation is the P7.20 gRPC-seam wiring | §4 item 6; ledger P7.20 step 3 |

## 6. P7.36 — what was implemented

`crates/flexcore-kernel/src/plugin_path_protection.rs`: the P4.28/P6.15
discipline adapted to the plugin path (cited, not reinvented —
`crates/flexcore-application/src/dispatch_security.rs` and
`crates/edge-agent/src/command_verify.rs`).

1. **The four checks, each a DISTINCT named refusal** — never one combined
   "valid" boolean. Evaluation order is FIXED: envelope → digest → window →
   sequence LAST, so a request refused by ANY earlier check never advances
   the sequence window (the P6.15 ordering decision, restated; the P4.28
   `SequenceGuard` rule: a refused sequence never moves the state).

   | # | Check | Refusal (`PluginPathError::name`) |
   |---|---|---|
   | 1 | envelope self-validity (non-empty correlation id, sequence ≥ 1, payload ≤ 1 MiB, `valid_from < valid_until`) | `REQUEST_ENVELOPE_INVALID` |
   | 2 | payload-bound identity: the recorded digest recomputes exactly over the canonical binding (correlation id, plugin id@version, sequence, window, algorithm label, payload) | `REQUEST_DIGEST_MISMATCH` |
   | 3 | validity window against the INJECTED clock (half-open: valid at `valid_from`, expired at `valid_until`) | `REQUEST_NOT_YET_VALID` / `REQUEST_EXPIRED` — expired → refused, never executed |
   | 4 | monotonic per-plugin/channel sequence | `SEQUENCE_REPLAYED` (== last accepted — duplicate delivery, §15.4) / `SEQUENCE_OUT_OF_ORDER` (< last accepted) |

2. **Payload-bound request identity:** correlation id + request digest,
   FNV-1a 64 — the recorded TEST-GRADE binding (`fnv1a64-test`, the
   P4.28/P6.14/P7.14 pattern). Production digest = DEPLOYMENT REQUIREMENT,
   the same honesty as `signing.rs`'s `NoVerifier` default; the algorithm
   label rides the canonical string, so a scheme change is a detectable
   format change, never a silent one. Pinned by the statement constant and
   by the field-by-field tamper test (every bound field — correlation id,
   plugin id, version, sequence, window, payload — changes the digest).
3. **Sequence checkpoint (restart survival):** the per-channel
   last-accepted sequence survives a host-process restart through the
   `SequenceCheckpointStore` trait; `FilePluginSequenceCheckpoint` is the
   file-backed implementation in the `buffer_file.rs` FRAMING DISCIPLINE
   (`crates/edge-agent/src/buffer_file.rs` — magic header + version byte,
   `[u32 LE len][u32 LE crc32][payload]` records, last-record-wins,
   truncate-and-audit recovery with the loss COUNTED
   (`corrupt_tail_truncated`), wrong magic a hard open error). Durability
   BEFORE state advance: a sequence is admitted only after its checkpoint
   record is flushed (`sync_data`), so a crash cannot lose a stored advance.
   The edge-agent crate is NOT imported — the framing is reimplemented in
   full inside the kernel (the kernel depends on no flexcore crate). The
   verifier loads the whole checkpoint at construction (fail-closed: no
   history, no admission — `CHECKPOINT_UNAVAILABLE`). Pinned by the
   restart-survival test (admit 1..=3, new verifier over the same file,
   replay of 3 refused, 4 admitted, third reopen sees 4).
4. **Duplicate vs out-of-sequence DISTINCT; duplicate-with-conflicting-
   payload handled:** a same-sequence replay is `SEQUENCE_REPLAYED`
   regardless of payload; a lower sequence is `SEQUENCE_OUT_OF_ORDER`; a
   captured request replayed with a tampered payload is
   `REQUEST_DIGEST_MISMATCH` (check 2 runs first). The two refusal names
   are pinned pairwise-distinct. The at-least-once duplicate delivery
   (§15.4) is thus processed exactly once — the outbox/inbox PERSISTENCE
   itself is the transport seam's obligation (recorded, not implemented
   here; §15.5 wiring is the P7.20 gRPC-seam / P7.18 supervision scope).
5. **Time:** the verifier reads the INJECTED `lifecycle::Clock` only —
   `SystemTime::now()` appears nowhere; same clock readings → same
   decisions (P7.42; the ledger's "expiry checked against wall-clock time
   without a clock bound" risk).
6. **Response echo:** a plugin response carries back the request's
   correlation id and sequence; a non-matching echo is
   `RESPONSE_CORRELATION_MISMATCH` / `RESPONSE_SEQUENCE_MISMATCH`, and the
   record path re-verifies the echo before recording an outcome.

## 7. ADR-010 on the path — the proposal-envelope proof

A plugin's RESPONSE to an optimizer-class contract is recorded as a
PROPOSAL envelope (`PluginProposalEnvelope`: proposal id, producer plugin
id + version, payload, sequence — typed, self-validating). The structural
half of the proof is TYPE-LEVEL: the path outcome enum
(`PluginPathOutcome`) has exactly two variants — `ProposalRecorded` and
`ResultObserved` — and NEITHER authorizes, issues, or approves anything;
there is no dispatch-authorizing variant, therefore a dispatch-authorizing
response is not constructible through this module (the same type-level
honesty as the P7.11 broker's `()`-returning validation-forward). The
pinned statement constants (`PROPOSAL_ENVELOPE_STATEMENT`,
`PLUGIN_PATH_PROTECTION_STATEMENT`) restate ADR-010/§13.4: a proposal is a
CANDIDATE entering the protected validation → feasibility → authorization
path, gaining no authority from having been proposed.

The dynamic half is the protected-contract guard, enforced at
resolve/registry: the AUTHORITATIVE refusal is the P7.06 resolver's
`resolve::PROTECTED_KERNEL_REPLACE` over
`resolve::PROTECTED_KERNEL_CONTRACT_IDS` (ADR-005/§5.7/§9.1 —
`kernel.command-authorizer`, `kernel.dispatch-gateway`,
`kernel.dispatch-validation`, `kernel.command-lifecycle`,
`kernel.audit-integrity`, `kernel.tenant-isolation`); this module RESTATES
it (`refuse_protected_contract_claim` → `PROTECTED_CONTRACT_CLAIM`) for a
manifest handed to the path directly, closing the ledger's "checked at the
broker but not at the protected gate, so a replayed proposal that bypasses
the broker still lands" bypass shape. Cited, restated, and pinned by a
test that asserts BOTH refusals fire on the same hostile manifest.

## 8. Named-error tables

### 8.1 P7.19 `ConfigError::name` — 18 names

`CONFIG_TOO_LARGE`, `CONFIG_NOT_AN_OBJECT`, `UNKNOWN_PROPERTY`,
`MISSING_REQUIRED`, `CONFIG_WRONG_TYPE`, `ENUM_VALUE_NOT_ALLOWED`,
`BOUND_VIOLATION`, `SECRET_VALUE_IN_CONFIG`, `SECRET_FIELD_COLLISION`,
`SECRET_REF_INVALID`, `CONSTRAINTS_DOCUMENT_INVALID`,
`CONSTRAINTS_MISSING_FIELD`, `CONSTRAINTS_UNKNOWN_FIELD`,
`CONSTRAINT_UNDECLARED`, `CONSTRAINT_TYPE_MISMATCH`,
`CONSTRAINTS_TOO_LARGE`, `CONFIG_DRIFT`, `PLUGIN_ID_MISSING`.

### 8.2 P7.20 `HealthError` — 3 names

`UNKNOWN_PLUGIN`, `INVALID_LIMITS`, `UNMAPPABLE_HOST_ERROR` (machine names
via the variant shapes; the health STATUS vocabulary is the ten closed
names in §4 item 1).

### 8.3 P7.36 `PluginPathError::name` — 12 names

`REQUEST_ENVELOPE_INVALID`, `REQUEST_DIGEST_MISMATCH`,
`REQUEST_NOT_YET_VALID`, `REQUEST_EXPIRED`, `SEQUENCE_REPLAYED`,
`SEQUENCE_OUT_OF_ORDER`, `CHECKPOINT_UNAVAILABLE`,
`CHECKPOINT_FILE_FOREIGN`, `RESPONSE_CORRELATION_MISMATCH`,
`RESPONSE_SEQUENCE_MISMATCH`, `PROPOSAL_ENVELOPE_INVALID`,
`PROTECTED_CONTRACT_CLAIM`. Plus `CheckpointError` (the store's I/O
surface: invalid path / I/O / foreign file) and the two statement
constants pinned by tests.

Both tables are pinned pairwise-distinct by the test suites.

## 9. DEFAULT-labelled decisions (recommendations awaiting human sign-off)

| # | Decision | Label |
|---|---|---|
| D-P7.19-1 | The enum/bounds/defaults surface lands as the LAYERED constraints document (`flexcore-plugin-config-constraints-v1`) parsed by `plugin_config.rs`, NOT as new manifest-schema keys (the P7.05 validator's accepted per-property keys stay `{type, description}`) | DEFAULT — keeps the manifest↔config-validator no-drift property; promoting the keys into manifest schema v2 is a reviewed schema change |
| D-P7.19-2 | The closed secret-field vocabulary is the seven tokens of `SECRET_FIELD_VOCABULARY`, separator-normalised matching | DEFAULT — extending the list is a reviewed change |
| D-P7.19-3 | ANY config string matching `env:` / `file:` / `secret-store:` with a non-empty location resolves to a handle (references are a value CLASS, not a per-property declaration) | DEFAULT — a literal string legitimately starting with a scheme prefix would be mis-filed; rename the property or use the constraints document if that ever bites |
| D-P7.19-4 | Reference EXISTENCE-in-store is checked host-side at `Configured` wiring (the kernel validates form only) | DEFAULT — the kernel has no secrets port and must not grow one |
| D-P7.20-1 | Readiness resets only via re-registration or a passed probe; a latched `PROBE_FAILED` is not cleared by a later successful call | DEFAULT — the probe verdict is its own recorded observation |
| D-P7.20-2 | An exhaustion DEGRADES and latches until a later successful call (or passed probe) clears it; the instance teardown stays the limits layer's contract | DEFAULT |
| D-P7.20-3 | `unresponsive_after_millis` and window/threshold values are composition configuration (validated, no silent defaults inside the monitor) | DEFAULT |
| D-P7.20-4 | Per-call duration histogramming for optimizer plugins is deferred to the Phase 1 metrics pipeline via the recorded gRPC seam; this wave ships `fuel_used` + call/error/timeout counters | DEFAULT |
| D-P7.36-1 | FNV-1a 64 as the recorded test-grade request binding; production digest = DEPLOYMENT REQUIREMENT (the `signing.rs` honesty pattern) | DEFAULT |
| D-P7.36-2 | Checkpoint channel key = `plugin-id/invocation` (the §14.3 "per asset or control channel" reading for the plugin path; `sequence_channel(plugin, channel)` is public so a deployment may name more channels) | DEFAULT |
| D-P7.36-3 | Payload cap 1 MiB per request envelope; checkpoint record cap 64 KiB; error-sample cap 64 | DEFAULT |
| D-P7.36-4 | Checkpoint framing magic `FLEXCORE-PLUGIN-PATH-SEQ-CKPT` + version `0x01` (distinct from the edge command checkpoint and the telemetry journal, so the three journals are never cross-read) | DEFAULT |

## 10. Honest limitations (UNRUN and otherwise)

1. **UNRUN:** everything above is written, not built and not run. In
   particular the hand-checked compile reasoning (borrow structure in the
   config resolver's property loop, the checkpoint replay loops, the
   health monitor's assessment borrows, trait-object lifetimes in the
   verifier) has not been checked by a compiler. The human build/test pass
   must run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D
   warnings`, `cargo test -p flexcore-kernel`, and `cargo test -p
   flexcore-plugin-host` and record results.
2. **No store-backed secret check.** The kernel validates reference FORM
   only; existence-in-store is the recorded host seam (D-P7.19-4). A
   config referencing a nonexistent secret is refused at `Configured` on
   the HOST side of the seam, not in this module.
3. **No transport.** The health snapshot's gRPC/REST carrier and its mTLS
   identity are recorded seams (§5); nothing listens, nothing dials.
4. **Duplicate-key collapse** (inherited from the P7.05 record §8.3):
   `serde_json::Value` maps silently keep the LAST occurrence of a
   duplicated JSON key in the config and constraints documents — the known
   accepted-collapse case P7.41 must treat.
5. **The kernel's checkpoint journals have no compaction** (the edge
   telemetry journal compacts; the sequence checkpoints are append-only
   with last-wins replay, like the P6.15 edge checkpoint). Growth is one
   small record per admitted request; a size-bounded rewrite is a
   follow-up if a deployment's admission rate makes the journal a space
   concern.
6. **Health correlation fields** (§21.3 full trace-ID correlation) ride the
   recorded gRPC seam's wiring; the snapshot carries plugin id/version,
   counters, and the caller-supplied correlation context but not a trace
   ID (none exists host-side yet — recorded, not assumed).
7. **P7.41 fuzz/property tests** should target `plugin_config.rs`'s
   constraints parser and `plugin_path_protection.rs`'s checkpoint replay
   (both are total: every input → Ok or a named error; no `unwrap`/
   `expect` on untrusted input in `src/`).

## 11. Verification actually run (the three permitted tools)

Run 2026-11-13, from the repository root (Windows, Git Bash, `python -I`
where applicable):

```text
$ python -I tools/check_dependency_rule.py
OK — dependency rule holds for 19 workspace members.
[exit 0]

$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
[exit 0]

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
[exit 0]
```

All three tools pass after the ledger tick. 19 workspace members: the 18 of
the P7.05/P7.06 record plus the member the concurrent P7.17/P7.18 wave
registered (this wave added NO member and NO dependency — the tool's count
is quoted verbatim from the run above; the console renders the tool's
em-dash as a mojibake byte on this terminal, the file content is UTF-8).
No cargo command was executed.

## 12. Ledger ticks

Only P7.19, P7.20, and P7.36 were ticked, in the exact ledger wording,
with the required suffix. P7.17 and P7.18 belong to the concurrent agent
and were not touched.
