# P6.13 / P6.14 / P6.17 — Adapter conformance harness, command-path gate, site policy and health: IMPLEMENTED (awaiting human build/test)

Date: 2026-11-13. Subtasks: P6.13, P6.14, P6.17, Phase 6 ledger
(`ledger_phase_6.md`). Owner roles: Rust engineer + security (per ledger).

**Run-status honesty (AGENTS.md rule 7; the loop bans in
`ledger/phase-06-edge-protocol/ENTRY_NOTE.md`):** the code and tests in this
wave are WRITTEN, NOT BUILT and NOT RUN. No `cargo` command of any kind was
executed. Every test below is UNRUN until the human build/test pass executes
it. Nothing in this document claims a test passed, a build succeeded, or a
measurement was taken.

Files (complete list; nothing outside it was touched):

| File | Status |
|---|---|
| `crates/edge-agent/src/conformance.rs` | NEW (P6.13) |
| `crates/edge-agent/src/command_path.rs` | NEW (P6.14) |
| `crates/edge-agent/src/policy.rs` | NEW (P6.17) |
| `crates/edge-agent/src/health.rs` | NEW (P6.17) |
| `crates/edge-agent/src/lib.rs` | EDITED (additive `pub mod` lines only) |
| `crates/edge-agent/tests/conformance_p6_13.rs` | NEW (P6.13) |
| `crates/edge-agent/tests/command_path_p6_14.rs` | NEW (P6.14) |
| `crates/edge-agent/tests/policy_health_p6_17.rs` | NEW (P6.17) |
| `ledger/phase-06-edge-protocol/ledger_phase_6.md` | EDITED (P6.13/P6.14/P6.17 ticks only) |
| this file | NEW (decision record) |

## 1. P6.13 — conformance harness (coverage and semantics)

`crates/edge-agent/src/conformance.rs` is a PROTOCOL-AGNOSTIC, reusable
test-side harness. It is the P6.19 BUILDING BLOCK only — P6.19 owns the
conformance-suite boundary record (which official suite applies, its access
terms, the CI wiring, and the claim-boundary statement). Passing this
harness proves only that an adapter honours the edge read-path contract
under the §19.2(6) fault set; it is not, and must never be quoted as,
protocol conformance, certification, or compliance (report §2.3;
AGENTS.md rule 2).

Harness API (public in the crate, driven from `tests/conformance_p6_13.rs`):

- `ConformanceCase` — the closed case set: `valid`, `malformed`,
  `truncated`, `out_of_order`, `duplicate_delivery`, `clock_skew_future`,
  `clock_skew_past`, `stale_data`, plus the strict-validation
  `unmapped_point` case (P6.13 step 1).
- `ConformanceHarness::run_fault_injection_set(&mut dyn EmsAdapter,
  &PointKey) -> ConformanceReport` — reads ONE valid template reading from
  the adapter, synthesises every faulted reading from it by explicit,
  reviewable field substitution, and runs all cases in a fixed
  deterministic order.
- `ConformanceReport::contract_violations()` — the pass condition: every
  case must land in its required outcome class (accept / reject / flag /
  idempotent skip), every rejection must carry a NON-EMPTY named
  diagnostic, and a stale-data case that mapped to GOOD is a violation
  (a forbidden quality upgrade). An empty violations list is the only pass.

The required behaviour per case (§19.2(6), each reject-or-flag, never
crash, never fabricate continuity):

| Case | Required outcome | Enforced by |
|---|---|---|
| valid | accepted, canonical sample complete | the ACL mapping table (P6.12) |
| malformed (unknown quality code; non-finite value) | rejected, NAMED diagnostic, never partially applied | `AclError::UnknownQualityCode` / `AclError::NonFiniteValue` |
| truncated (missing quality; cut RFC 3339 timestamp) | rejected, NAMED diagnostic | `AclError::QualityNotDeclared` / `AclError::NonUtcNativeTimestamp` |
| out_of_order (sequence regression) | flagged, never re-ordered or re-numbered | the harness records the regression; the P4 `SequenceGuard` discipline is cited, not reimplemented (P6.15 owns edge-side sequence checks) |
| duplicate_delivery | idempotent skip, counted, one stored value | `ingest::EdgeDedupWindow` (the P4.24 discipline, cited) |
| clock_skew_future | rejected (ingest >= event, P0.09) — never a silent clock "fix" | the `EdgeTelemetrySample` constructor |
| clock_skew_past | accepted with the skew recorded; beyond `ConformanceLimits::max_clock_skew_past_millis` → flagged | the harness (recorded, never shifted) |
| stale_data | flagged with canonical STALE, never upgraded to GOOD | the ACL quality table (P0.08) |
| unmapped_point | rejected + the unknown field RECORDED | `AclError::PointUnmapped` + the declared-unmapped inventory |

Strict-validation semantics (P6.13 step 1) live in the P6.12 ACL and are
EXERCISED here, not reimplemented: unknown fields are recorded (the
declared-unmapped inventory), malformed messages are rejected WHOLE with a
named diagnostic (no partial mapping — the P2.11 step-3 rule at the adapter
boundary), and there is no `panic!`/`unwrap` on untrusted input on this
surface — every ACL outcome is a `Result`, and the harness runs all faulted
readings through the same total path (the no-panic property is asserted in
`tests/conformance_p6_13.rs`, including against a forced-unreachable
adapter, which degrades to a named violating report).

Drive target: the TEST-ONLY `SimulatedEmsSource` fixture
(`adapters::simulated`, registry id `simulated-test`, `AdapterStatus::TestOnly`)
composed through `AdapterIngestSource` — in-process, deterministic, no
network, exactly as in the P6.08 suite. When P6.10 lands the `openems-rest`
adapter, the same harness composes it unchanged (protocol-agnostic by
construction).

**Reference vectors: NONE RUN (none obtainable in this loop); UNRUN.**
OpenEMS provides no offline-obtainable reference vectors in this wave (the
EdgeConfig API surface is itself UNVERIFIED — RESEARCH.md §1); running any
official protocol suite is P6.19's scope, not done here. This is the honest
P6.13 step-3 record: no vector was run, none passed, none failed.

**Read-path latency/throughput measurement (P6.13 step 4) — TEMPLATE, cells
INTENTIONALLY EMPTY (no figures are fabricated; AGENTS.md rule 1):**

| Metric | Value | Hardware profile | FlexCore version | Adapter id | Protocol version | Conditions |
|---|---|---|---|---|---|---|
| Read-path latency p50 (ms/point) | *(empty — UNRUN)* | | | | | |
| Read-path latency p95 (ms/point) | *(empty — UNRUN)* | | | | | |
| Read-path latency p99 (ms/point) | *(empty — UNRUN)* | | | | | |
| Throughput (samples/s) | *(empty — UNRUN)* | | | | | |
| Per-poll allocation note | *(empty — UNRUN)* | | | | | |

## 2. P6.14 — the command-path enablement gate

`crates/edge-agent/src/command_path.rs` implements the edge command
receiver, DISABLED BY DEFAULT (code default, config default, and every
shipped profile — `CommandPathGateInputs::default()` is fully off).

### 2.1 The four-condition gate (all-or-nothing, fail-closed)

| # | Condition | Input (env name defined in `command_path.rs`) | Absent/wrong → named refusal |
|---|---|---|---|
| a | Explicit signed per-site configuration change | `FLEXCORE_EDGE_COMMAND_PATH=enabled` (exactly `enabled` enables; `disabled`/absent off; anything else is a named config error) | `config_flag_off` |
| b | Signed enablement record + matching test-lab token | `FLEXCORE_EDGE_COMMAND_ENABLEMENT_RECORD` (the serialized signed record), `FLEXCORE_EDGE_COMMAND_ENABLEMENT_TOKEN` (must equal the record's token), plus the enablement KEY supplied out-of-band by the deployment | `no_signed_record` / `record_digest_mismatch` / `enablement_token_mismatch` / `key_reference_mismatch` / `record_site_mismatch` |
| c | Target adapter on the site command allowlist | `FLEXCORE_EDGE_SITE_COMMAND_ALLOWLIST` (comma-separated adapter ids; enforced at evaluation AND per command) | `command_allowlist_empty` / `adapter_not_allowlisted` |
| d | The site's D6 review record exists | `FLEXCORE_EDGE_D6_RECORD_REFERENCE` (a reference to the site's D6 decision record — the P6.03 output) | `d6_record_absent` |

Evaluation order is FIXED (deterministic refusal reason): a → c → d → b
(record presence → body completeness → site binding → key reference →
digest → token) → expiry. Enablement is TIME-BOUNDED (`valid_until_unix_millis`
on the record): the bound is enforced at READ time
(`CommandPathRuntime::enablement_at`), so a long-lived runtime auto-disables
at expiry without any mutation, and the disable transition is audited
exactly once.

Audit events (through the edge `AuditSink` port; enrollment.rs naming
style): `EDGE_COMMAND_PATH_ENABLED` (at composition, naming approver,
scope, expiry), `EDGE_COMMAND_PATH_DISABLED` (at composition when off, and
once on the expiry transition), `EDGE_COMMAND_ACCEPTED` (gate + policy
admission), `EDGE_COMMAND_REJECTED` (every refusal, carrying the
P4-`TenantDenialAuditRecord`-pattern record — cited, adapted to the edge:
`command_denial command=… asset=… adapter=… code=… at=… reference=…`).

**Recorded audit limitation:** `EdgeAuditKind` (ports.rs) is a CLOSED
vocabulary this wave does not own, so every command-path audit event
records kind `EDGE_STARTUP` with the dedicated event name LEADING the
detail string. The dedicated name is the stable, greppable audit identity;
adding the four dedicated variants to `EdgeAuditKind` is item W4 in §7.

### 2.2 The signed-enablement-record format (`flexcore-edge-command-enablement-v1`)

```text
flexcore-edge-command-enablement-v1
site = <site id>
approver = <named approver, per the gate procedure>
adapters = <comma-separated adapter scope>
enablement_token = <test-lab marker value>
issued_at_unix_millis = <i64>
valid_until_unix_millis = <i64, strictly after issued_at>
key_reference = <non-secret key label, e.g. deployment:lab-enablement-key-1>
digest = <16 lowercase hex chars>
```

- The digest is an HMAC-STYLE keyed digest over the canonical body (every
  field above except `digest`, fixed order). HONESTY (AGENTS.md rule 1):
  the implementation is a keyed FNV-1a fold (key mixed first, explicit
  domain-separation byte, then the body) — a keyed CHECKSUM, NOT a
  cryptographic MAC. It detects corruption and casual tampering; it is not
  proof against a determined adversary. **Replacing it with a real HMAC
  (HMAC-SHA-256 or better) under a reviewed key-management scheme is a
  DEPLOYMENT REQUIREMENT**; the versioned format id makes that replacement
  a detectable format change, never a silent one.
- **The key material itself is a DEPLOYMENT REQUIREMENT**: the FlexCore
  code base generates, stores, ships, and distributes NO key material. The
  deployment provisions the key out-of-band (secret manager / protected
  volume, §17.3); the composition root loads it and hands it to
  `CommandPathRuntime::evaluate` as an `EnablementKey` (redacted
  `Debug`/`Display`; `expose()` is the one named accessor). The key is
  never an environment variable default, never in a shipped profile, never
  in the record, and never in any log or audit line. The record carries
  only the non-secret `key_reference` label, checked against the loaded
  key's reference before the digest is compared.
- The record parses STRICTLY: wrong version, unknown field, missing field,
  malformed digest → named error; unknown fields are never silently
  accepted (the P6.13 step-1 discipline applied to the gate's own input).

### 2.3 Structural no-plugin-enable (§5.7)

The enablement state lives in `CommandPathRuntime` as a PRIVATE field,
computed ONCE by `CommandPathRuntime::evaluate` — the only constructor.
There is no public setter, no `&mut self` method on the runtime, and no
code path that turns a disabled runtime into an enabled one: `enablement_at`
RE-DERIVES the state (including the expiry read) from the immutable inputs
on every call. A plugin/adapter/optimizer can hold, at most, a shared
reference to the runtime — and a shared reference exposes reads only. The
receiver's `&mut self` bookkeeping is the expiry-transition audit flag
only; the enablement itself is never stored mutably. Plugins hold no
reference capable of mutation, and the `CommandPolicyGate` seam they could
reach is `&self`-only. Behaviourally pinned in
`tests/command_path_p6_14.rs` (`a_plugin_holding_a_shared_reference_cannot_enable_the_path`);
the compile-level guarantee is the absence of any setter signature, which
the human review confirms by reading `command_path.rs`.

### 2.4 Network shape

Even when enabled, the command path creates NO inbound listener (§3.3): the
receiver is an in-process method (`CommandReceiver::receive`) that the P6.07
outbound mTLS channel adapter calls after demarshalling a response-direction
frame ON THE OUTBOUND SESSION the edge dialled. There is no socket code in
`command_path.rs` (or anywhere new this wave), and
`tests/no_inbound_listener.rs` (the crate-wide lexical scan over `src/`)
passes unchanged — no forbidden token appears in any of the four new
modules, and the test was not weakened. When P6.16 wires the real response
demarshalling into `uplink_mtls`, the same structural rule applies: the
session is outbound-initiated; nothing binds or accepts.

### 2.5 Scope honesty

With the gate ENABLED, a receipt from this module is a GATE + POLICY
admission only. Edge-side signature/expiry/sequence verification is P6.15;
forwarding to an allowlisted adapter and fallback is P6.16. Neither exists
this wave, and the `EmsAdapter` port has no write method (P6.12: absence is
structural). The receipt's `scope_note` says exactly this; acknowledgement
is not physical delivery (§14.1).

## 3. P6.17 — site-local policy and adapter health

### 3.1 Policy engine (`crates/edge-agent/src/policy.rs`)

Surface: per-adapter enablement (an adapter with NO policy entry is
UNKNOWN and refused — never defaulted to enabled), per-adapter command rate
limits (max commands per window; sliding window; a refused command consumes
no budget), and allowed operating windows (UTC millis-of-day, no wrap — a
wrap is a validation error). Violation behaviour: reject + audit, never
silently proceed — the engine returns named `PolicyViolation`s
(`policy_unknown_adapter`, `policy_adapter_disabled`,
`policy_rate_limit_exceeded`, `policy_outside_operating_window`) and never
logs-and-continues.

Gate re-check wiring (no cyclic module dependency): the command path
DEFINES the `CommandPolicyGate` port (in `command_path.rs`); the policy
engine IMPLEMENTS it (policy → command_path, one direction). The receiver
takes `Arc<dyn CommandPolicyGate>` at construction — a command path without
an independent local policy gate does not exist (fail-closed by
composition).

Versioned site-policy configuration format (`flexcore-site-policy-v1`,
line-oriented, strict parse — unknown fields are named errors):

```text
flexcore-site-policy-v1
adapter = <adapter id>
enabled = true|false
rate_limit = <max_commands>/<window_millis>
operating_window = <HHMM>-<HHMM>   (UTC; start < end; no wrap)
```

(one block per adapter; every field of every block is REQUIRED — the
no-defaults rule). The DEPLOYMENT config format is this document; the
in-code `SitePolicyConfig` struct is its code form. Extending the format is
a versioned, reviewed change.

### 3.2 Health monitoring (`crates/edge-agent/src/health.rs`)

Closed vocabulary, exactly: `HEALTHY`, `DEGRADED_<reason>`,
`FAILING_<reason>`; reasons from the closed set `UNVERIFIED`,
`UNRESPONSIVE`, `STALE_DATA`, `ERROR_RATE`, `POLICY_BLOCK`,
`CREDENTIAL_EXPIRY`. Extending the set is a reviewed change (a closed
vocabulary is the point).

Derivation rules (first match wins; FAILING outranks DEGRADED; every rule
is a positive check on OBSERVED inputs — health is NEVER inferred from the
absence of errors, and unknown is `DEGRADED_UNVERIFIED` until the first
observation, never healthy):

1. never observed → `DEGRADED_UNVERIFIED`;
2. last responsive check failed, or no observation within
   `unresponsive_after_millis` (the detection window on the last SEEN
   fact) → `FAILING_UNRESPONSIVE`;
3. credential expired → `FAILING_CREDENTIAL_EXPIRY`;
4. policy block noted → `DEGRADED_POLICY_BLOCK`;
5. credential inside `credential_rotation_window_millis` →
   `DEGRADED_CREDENTIAL_EXPIRY` (visible at least one rotation window
   before expiry — the P6.06 acceptance rule);
6. data older than `freshness_limit_millis` (or no data event ever seen) →
   `DEGRADED_STALE_DATA`;
7. errors within `error_rate_window_millis` at/above
   `max_errors_in_window` → `DEGRADED_ERROR_RATE`;
8. only otherwise → `HEALTHY`.

Certificate expiry (from the P6.06 credential store's
`DeviceCredentialRecord.expires_at_unix_millis`) is a monitor input via
`set_credential_expiry`; the composition feeds it on the expiry-assessment
cadence. `None` (the shipped-seam "no expiry recorded" shape) is kept in
the record detail and never interpreted as "valid forever".

Blocked dependent actions: while an adapter is `FAILING_*`,
`check_action_allowed` blocks ingesting from it and any command targeting
it, returning the reason for the caller's AUDITED refusal (reject + audit,
never silent). `DEGRADED_*` still flows — data arrives flagged (P0.08) and
the command gate is the policy engine's job; the block boundary is
`FAILING_*`, as the ledger names it.

Console surfacing (§20.1 Sites workspace) is P6.25's job. Exposed here:
`HealthSnapshot` + `AdapterHealthRecord` — plain data, ordered by adapter
id, with `status_name()` (the closed vocabulary) and a flat `json()`
rendering (the crate carries no serde dependency this wave; when P6.25's
console work needs typed serialization, adding serde to this crate is a
P6.02-audited dependency addition, recorded then). Edge logs carry the
adapter id and human-safe detail — never raw backend text (P1.08), never
secrets (§17.3).

## 4. DEFAULT-labelled decisions (this wave's defaults, each reversible by a reviewed change)

- **DEFAULT:** the enablement digest is keyed FNV-1a (HMAC-STYLE), not
  HMAC-SHA-256. Reason: no crypto dependency is added this wave; the
  format id makes the upgrade detectable. Consequence: the digest is a
  tamper-evident CHECKSUM, not cryptographic authentication; the real HMAC
  + key management is a DEPLOYMENT REQUIREMENT.
- **DEFAULT:** the gate env inputs are read by
  `CommandPathGateInputs::from_env` inside `command_path.rs`, not
  `config.rs` (config.rs/main.rs are owned by another agent this wave).
  Composition-root wiring is itemised in §7.
- **DEFAULT:** `CommandReceiver` requires a `CommandPolicyGate` at
  construction (fail-closed): no receiver exists without the independent
  policy gate.
- **DEFAULT:** all command-path audit events record kind `EDGE_STARTUP`
  with the dedicated event name leading the detail (closed
  `EdgeAuditKind`; §2.1).
- **DEFAULT:** the policy operating window does not wrap (`start < end`);
  a wrap is a load error, not an interpretation.
- **DEFAULT:** a `ClockSkewPast` reading within
  `max_clock_skew_past_millis` is accepted (an old event is still data,
  flagged beyond the bound); future events are always rejected (P0.09).
- **DEFAULT:** credential expiry `None` ("no expiry recorded") does not
  change the health state; it is carried in the record detail and audited
  by the P6.06 expiry assessment as `NoExpiryRecorded`.
- **DEFAULT:** the `simulated-test` TEST-ONLY fixture is the P6.13 drive
  target (no OpenEMS exists in this loop); the harness is protocol-agnostic
  and composes `openems-rest` unchanged when P6.10 lands it.

## 5. Honest limitations

- Everything is UNRUN (see the header): no build, no test execution, no
  measurement. The three permitted verification tools were run (their
  output is recorded in the handover note); they check documentation and
  dependency direction, not Rust compilation.
- `from_env()`'s process-environment reads are exercised only by the pure
  flag-parse test (no process-env mutation in tests, to avoid env races);
  the full env path is composition-root territory and is UNRUN.
- The conformance harness drives the edge CONTRACT (ACL + dedup + sample
  rules), not the wire: no protocol bytes are parsed anywhere this wave
  (`openems-rest` is P6.10/P6.13's remaining work against the VERIFIED
  OpenEMS API). Reference vectors: NONE RUN (none obtainable in this
  loop); UNRUN.
- The signed-record digest is not a cryptographic MAC (§2.2).
- The command path admits nothing end-to-end: no P6.15 verification, no
  P6.16 forwarding, no adapter write method. A receipt is not an issuance.
- The rate-limit window state is in-memory (Mutex); a restart clears it —
  persistence across restarts is P6.18's audit-buffer/offline-behaviour
  scope.
- Health is per-adapter, in-memory, and reset on restart; persistence and
  history are out of scope this wave.

## 6. P6.19 / P6.23 / P6.25 seams (served, not implemented)

- **P6.19 (conformance suite + claim boundary):** `conformance.rs` is the
  reusable FlexCore-side suite driver. P6.19 adds: the official-suite
  identification and access-terms record, protocol-specific cases, the CI
  wiring, and the claim-boundary statement. The harness's
  `ConformanceReport::render()` is designed to be the per-run evidence
  line in that record. Nothing here claims certification.
- **P6.23 (security testing):** the P6.14 gate's refusal codes
  (`record_digest_mismatch`, `enablement_token_mismatch`,
  `adapter_not_allowlisted`, …) and the audited rejections are the hooks
  P6.23's forgery/replay/auth-bypass cases assert against. The keyed
  checksum limitation (§2.2) is a P6.23-relevant finding recorded NOW, at
  implementation time.
- **P6.25 (console surfacing):** consume `health::HealthSnapshot` (ordered
  records, closed status names, `json()` rendering), the receiver's
  `enablement(now)` read (disabled / test-env enabled with approver +
  expiry) for the command-path state display, and
  `policy::SitePolicyEngine::adapter_ids()/is_adapter_enabled()` for the
  policy view. The console must only DISPLAY these — enabling the command
  path happens exclusively through the P6.14 procedure (§20.2; the console
  can never flip the gate, and the runtime's structural immutability is
  what guarantees that).

## 7. Coordinator wiring list for `config.rs` / `main.rs` (NOT edited this wave — another agent owns them)

Exact additions the composition root needs to compose P6.13/P6.14/P6.17:

1. **Env names (already defined in `command_path.rs` — do not duplicate):**
   `FLEXCORE_EDGE_COMMAND_PATH` (`enabled`/`disabled`; DEFAULT `disabled`),
   `FLEXCORE_EDGE_COMMAND_ENABLEMENT_RECORD`, `FLEXCORE_EDGE_COMMAND_ENABLEMENT_TOKEN`,
   `FLEXCORE_EDGE_SITE_COMMAND_ALLOWLIST` (DEFAULT empty → gate off),
   `FLEXCORE_EDGE_D6_RECORD_REFERENCE`. `config.rs` may re-export the
   constants (`pub use crate::command_path::{ENV_COMMAND_PATH_FLAG, …}`) —
   additive only; the existing `ENV_COMMAND_PATH_ENABLED` bool stays as-is
   (independent, and a `true` there still activates nothing without the
   P6.14 gate).
2. **Enablement key loading (deployment requirement):** the composition
   root loads the key from the deployment's secret store and builds
   `EnablementKey::new(value, reference)`. NO env var for the key VALUE —
   if a lab-only env fallback is ever added, it must be named
   `FLEXCORE_EDGE_COMMAND_ENABLEMENT_KEY` and its absence must leave the
   gate disabled (recommended: keep it out of env entirely).
3. **`main.rs` composition order** (after `EdgeConfig::from_env` and the
   registry check):
   ```text
   inputs   = CommandPathGateInputs::from_env()?      // fail-fast, defaults all OFF
   key      = <deployment secret store load>
   runtime  = CommandPathRuntime::evaluate(&inputs, &key, now_millis)
   policy   = Arc::new(SitePolicyEngine::new(SitePolicyConfig::parse(&policy_document)?))
   audit    = Arc::new(InMemoryAuditSink::new())       // P6.18 replaces with the durable buffer
   clock    = Arc::new(WallClock)
   receiver = CommandReceiver::new(runtime, policy, audit, clock)   // audits the gate state
   health   = AdapterHealthMonitor::new(HealthLimits { … })?        // limits from config (§7.4)
   ```
4. **Health limits config (to be added to `config.rs` by the coordinator,
   DEFAULTS shown):** `FLEXCORE_EDGE_HEALTH_FRESHNESS_MS` (default 30 000),
   `FLEXCORE_EDGE_HEALTH_UNRESPONSIVE_AFTER_MS` (default 60 000),
   `FLEXCORE_EDGE_HEALTH_ERROR_WINDOW_MS` (default 300 000),
   `FLEXCORE_EDGE_HEALTH_MAX_ERRORS` (default 3),
   `FLEXCORE_EDGE_CREDENTIAL_ROTATION_WINDOW_MS` already exists (P6.06).
5. **Per-tick wiring:** the supervision loop feeds
   `health.observe(<adapter_id>, now, AdapterObservation::…)` per poll
   outcome (`Responsive { event_age_millis }` from the poll's samples,
   `PollFailed` on `EdgeError::SourcePollFailed`); before issuing any
   ingest/command action it calls `health.check_action_allowed(...)` and
   audits the refusal when blocked; when the receiver returns
   `CommandRefusal::PolicyRefused`, the composition calls
   `health.note_policy_block(<adapter_id>)`; the P6.06 expiry assessment
   feeds `health.set_credential_expiry(...)`; the P6.25 console reads
   `health.snapshot(now)`.
6. **`EdgeAuditKind` additive extension (recommended follow-up):** add
   `CommandPathEnabled`, `CommandPathDisabled`, `CommandAccepted`,
   `CommandRejected` to the closed enum in `ports.rs` and switch
   `command_path.rs`'s `audit_event` to the dedicated kinds (the detail
   prefix stays identical, so no audit consumer breaks).
7. **Site-policy document loading:** `FLEXCORE_EDGE_SITE_POLICY_PATH`
   (proposed name) read by the composition root, parsed with
   `SitePolicyConfig::parse`; a parse failure is a fail-fast startup error
   (the edge must not run with no local policy).

## 8. Verification status (honest)

- `python tools/verify_docs.py`, `python tools/validate_agent.py`,
  `python -I tools/check_dependency_rule.py` — run at wave end; their
  ACTUAL output is recorded in the agent handover note. They do not
  compile Rust.
- `cargo build -p flexcore-edge-agent`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test -p flexcore-edge-agent` —
  NOT RUN (banned this loop). Status: UNRUN — the human build/test pass
  owns them.
