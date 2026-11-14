# P6.15 + P6.16 — Command verification at the edge and forwarding only to allowlisted adapters: IMPLEMENTED (awaiting human build/test)

Date: 2026-11-13. Subtasks: P6.15, P6.16, Phase 6 ledger
(`ledger_phase_6.md`). Owner roles: security + Rust engineer (per ledger).
Depends on: P6.14 gate (`13_14_17_adapter_command_policy.md`), P6.12
`EmsAdapter` port (`05_12_20_edge_foundation.md`), the Phase 4 signed-command
model (`crates/flexcore-application/src/dispatch_security.rs`, P4.28) and
the P4.17 fallback-policy precedent
(`ledger/phase-04-grid-feasibility-dispatch/17_19_fallback_timing.md`).

**Run-status honesty (AGENTS.md rule 7; the loop bans in
`ledger/phase-06-edge-protocol/ENTRY_NOTE.md`):** the code and tests in this
wave are WRITTEN, NOT BUILT and NOT RUN. No `cargo` command of any kind was
executed. Every test below is UNRUN until the human build/test pass executes
it. Nothing in this document claims a test passed, a build succeeded, or a
measurement was taken.

Files (complete list; nothing outside it was touched):

| File | Status |
|---|---|
| `crates/edge-agent/src/command_verify.rs` | NEW (P6.15) |
| `crates/edge-agent/src/command_forward.rs` | NEW (P6.16) |
| `crates/edge-agent/src/lib.rs` | EDITED (additive `pub mod` lines anchored after `pub mod conformance;` only) |
| `crates/edge-agent/src/command_path.rs` | EDITED (ADDITIVE ONLY: the `GatedCommandPipeline` section appended after `audit_timestamp`; no existing behaviour, type, or test changed) |
| `crates/edge-agent/tests/command_verify_p6_15.rs` | NEW (P6.15) |
| `crates/edge-agent/tests/command_forward_p6_16.rs` | NEW (P6.16) |
| `ledger/phase-06-edge-protocol/ledger_phase_6.md` | EDITED (P6.15/P6.16 ticks only) |
| this file | NEW (decision record) |

## 1. Pipeline shape (how the layers compose)

The full test-env chain, in fixed order, is composed by
`command_path::GatedCommandPipeline::receive_verify_and_forward` (ADDITIVE
wiring — the P6.14 `CommandReceiver` is untouched and remains usable alone):

```text
response side of the OUTBOUND mTLS session (no inbound listener, §3.3)
  → P6.14 gate (CommandReceiver::receive — four conditions, per-command
    allowlist, P6.17 policy admission; unchanged)
  → P6.15 verifier (CommandVerifier::verify — the four checks below)
  → P6.16 engine (CommandForwardingEngine::forward_verified — SEPARATE
    allowlist re-check with its own audit event, fallback-stop check,
    P6.17 policy re-check through the CommandPolicyGate seam, issue,
    forwarder port, §14.1 states, per-asset fallback)
```

Every stage fails closed and audits its own refusals; a refusal at ANY
stage leaves nothing behind (no queue, no buffer, no retry). The pipeline
REQUIRES the verifier and the engine at construction — a command path that
admits without verification, or verifies without the separately-gated
forwarder, does not exist in this composition.

## 2. P6.15 — the four-check verifier (`command_verify.rs`)

Run on EVERY gate-admitted command BEFORE any forwarding. Every rejection
carries a DISTINCT named reason — never one combined "valid" boolean (the
ledger names that as the recorded risk; a test pins the six codes pairwise
distinct).

| # | Check | Rejection reason (audit code) | Named failure cases |
|---|---|---|---|
| 1 | Payload-bound signature against the central platform's key reference | `signature` | unsigned (empty reference); malformed reference; wrong signer (key-reference mismatch, checked BEFORE the digest); command-id binding mismatch; tampered payload (any bound field changed) |
| 2 | `valid_from`/`valid_until` against the edge clock WITH the clock-quality check | `expiry` | edge clock quality not `Synchronized` (FAILS CLOSED — see §3); command not yet valid; command expired; authorization reference expired; enablement admission expired |
| 3 | Monotonic sequence per asset/control channel, restart-persisted | `duplicate` (sequence equals the last accepted — a replay) / `sequence` (below the last accepted, or zero = unsequenced, or a checkpoint-store failure refusing fail-closed) | — |
| 4 | Target adapter on the site allowlist + command within the approved scope/limits/expiry of its authorization reference (§17.5 rule 6) | `allowlist` / `scope` | non-allowlisted adapter; asset without a scope; quantity outside the approved set; value outside the approved limits |

**Evaluation order (recorded, deliberate):** 1 → 2 → 4a (allowlist) → 4b
(scope) → 3 (sequence LAST). The ledger numbers sequence as check 3; it is
evaluated last so a command refused by ANY earlier check never advances the
sequence window (the P4.28 `SequenceGuard` rule: a refused sequence never
moves the state — no attacker-driven window advance). All reasons remain
distinct; the ordering is a recorded decision, not a semantics change.

**Reused by citation (adapted, not reinvented), per the ledger's
"reuse-by-citation" instruction:**

- The signature scheme is the P4.28 payload-bound pattern
  (`crates/flexcore-application/src/dispatch_security.rs`): reference form
  `edge-sig:<key_reference>:<command_id>:fnv1a64:<digest>` over the
  canonical command binding (`edge-cmd:<id>|asset:<key>|seq:<n>|adapter:
  <a>|qty:<q>|val:<f64-bit-pattern>|via:<transport-identity>`) — the
  `DispatchPayloadBinding::canonical_string` discipline, with the value
  carried by its exact bit pattern.
- The sequence guard is the P4.28 `SequenceGuard` semantics (monotonic per
  asset/channel; duplicate vs out-of-order distinguished; refusals never
  update state), adapted to the edge-local asset key.
- The rejection audit record is the P6.14 `CommandRejectionAuditRecord`
  (itself the P4.28 `TenantDenialAuditRecord` pattern), reused verbatim
  with the verifier's distinct reason codes.

### 2.1 Signature honesty and key material (deployment requirement)

- The digest is a keyed FNV-1a fold (key mixed first, explicit
  domain-separation byte) — the same HMAC-STYLE shape as the P6.14
  enablement digest. It is a keyed CHECKSUM, NOT a cryptographic MAC, and
  detects corruption and casual tampering only. **Replacing it with a real
  signature (HMAC-SHA-256 or an asymmetric scheme) under a reviewed
  key-management scheme is a DEPLOYMENT REQUIREMENT**; the versioned scheme
  prefix (`edge-sig`) makes that replacement a detectable format change,
  never a silent one.
- **The key material itself is a DEPLOYMENT REQUIREMENT** (§17.3): the
  FlexCore code base generates, stores, ships, and distributes NO key
  material. The deployment provisions the central platform's
  command-signing key out-of-band (secret manager / protected volume); the
  composition root loads it into `CommandSigningKey` (redacted
  `Debug`/`Display`; `expose()` is the one named accessor). The key never
  enters an environment-variable default, a shipped profile, the signature
  reference (which carries only the non-secret key REFERENCE), or any
  log/audit line.

### 2.2 The clock-quality input (the P6.31 seam — NOT implemented here)

The §17.5 rule 5 fail-closed rule is implemented: ANY clock quality other
than `Synchronized` — including `Unknown` (never silently assumed good,
P0.09 §4) — fails the expiry check closed, even for an otherwise-valid
command. The quality status itself is a PARAMETER
(`CommandVerifier::verify(..., clock_quality: ClockQuality)`); this module
does not detect drift, query NTP, or own the synchronisation work. **The
in-repo owner is ledger P6.31** ("Edge time synchronization and
clock-quality monitoring" — its objective text explicitly names the
clock-quality status that P6.15's command verification depends on). The
composition feeds the verifier from the P6.31 monitor once it exists; until
then a test/lab composition must supply the recorded status explicitly, and
the fail-closed default means an UNSUPPLIED status cannot be mistaken for
synchronized. (Recorded note: the wave brief named P6.32 for this work; the
ledger assigns it to P6.31, and the ledger is the in-repo authority per
AGENTS.md §2. P6.32 is edge decommissioning.)

### 2.3 Sequence checkpoint persistence (the restart-replay risk)

The per-channel last-accepted sequence survives an edge restart through the
`SequenceCheckpointStore` trait (defined in `command_verify.rs`):

- `FileSequenceCheckpoint` — the file-backed implementation, in the SAME
  framing discipline as `buffer_file.rs`: magic header
  (`FLEXCORE-EDGE-COMMAND-SEQ-CKPT` + version byte 0x01), length-prefixed
  CRC-32-checked records, truncate-and-audit recovery (the first malformed
  frame ends replay; the tail is truncated and the loss COUNTED —
  `corrupt_tail_truncated`; a wrong magic is a hard open error, never a
  guess), and a channel-length sanity cap.
- **Durability before state:** `verify` persists the sequence advance
  (flush + `sync_data`) BEFORE the in-memory window moves — a crash cannot
  lose a stored advance, and a store failure REFUSES the command
  fail-closed (an unpersisted advance could be replayed after a restart).
- The verifier loads the full history at construction (fail-closed: no
  history, no verification). The post-restart replay of a pre-restart
  command is rejected as a `duplicate` (tested).

## 3. P6.16 — forwarding and fallback (`command_forward.rs`)

### 3.1 RECORDED DESIGN DECISION — the `CommandForwarder` port vs the read-path `EmsAdapter` port

The ledger's P6.16 step 2 says "forward via the adapter's `propose_setpoint`
path". The P6.12 `EmsAdapter` port DELIBERATELY has NO dispatch/write method
— its absence is structural (decision doc `05_12_20_edge_foundation.md`).
Adding a write method to `EmsAdapter` would give every read-path consumer a
reachable write path, destroying the structural guarantee.

**Resolution:** the command path forwards through a SEPARATE
`CommandForwarder` port, defined IN `command_forward.rs` (in-module, not
`ports.rs` — the `EmsAdapter`-in-`adapters/port.rs` precedent),
implemented ONLY by:

1. the scripted TEST DOUBLE in `tests/command_forward_p6_16.rs`, and
2. `FailClosedCommandForwarder` in production composition — every call
   fails with a named transport error, so a production edge records
   ISSUED → FAILED and applies the per-asset fallback; it never fabricates
   an adapter interaction.

The forwarder is reachable ONLY behind the P6.14 gate AND the P6.15
verifier AND the P6.17 policy re-check. The READ path's no-write guarantee
is untouched (`EmsAdapter` still has no write method; no read-path type can
reach `CommandForwarder`). When a real adapter write path is later
qualified (post-v0.4.0, per deployment), it implements THIS port behind its
own reviewed composition.

### 3.2 The separate allowlist re-check (its own audit event)

The forwarder re-checks the target adapter against the forward-side
`SiteCommandAllowlist` before EVERY forward, emitting
`EDGE_COMMAND_FORWARD_ALLOWLIST_CHECKED` — deliberately SEPARATE from the
P6.15 verification-side pre-check (which reads the authorization
reference's allowlist). A non-allowlisted adapter is refused with code
`allowlist` (`EDGE_COMMAND_FORWARD_REJECTED`) and the forwarder port is
NEVER invoked for it (asserted against the test double's call recorder).

### 3.3 §14.1 state recording (acknowledgement ≠ delivery)

| Outcome | §14.1 handling |
|---|---|
| forward accepted by the port | `ISSUED` recorded, then `ACKNOWLEDGED` on the adapter ack — the record carries the P4/P5 honesty wording **"acknowledgement is not physical delivery (§14.1)"**; tracking observations, not the ack, establish what happened |
| adapter NACK | `FAILED` (the frozen ISSUED → FAILED trigger, per P4.28), then fallback per policy |
| adapter timeout / transport failure | `FAILED` (transport is timeout-class for the fallback policy; the detail keeps the distinction), then fallback per policy |
| no acknowledgement in the window | `FAILED` with the honesty wording, then fallback per policy |
| tracking on-target / deviation / completion | `TRACKING` / `FAILED` + fallback (`tracking_deviation` trigger) / `COMPLETED` |
| expiry mid-flight | `EXPIRED`; **FALLBACK only where the per-asset policy configures the `expiry` trigger** (the P4.17 precedent: EXPIRED → FALLBACK is the only EXPIRED outgoing edge and fires only where the policy requires it) |

Every transition is audited (`EDGE_COMMAND_LIFECYCLE` with
`command=… asset=… adapter=… state=… at=…`). A premature expiry call (clock
before `valid_until`) is refused (`not_yet_expired`) — fail-closed, no
premature expiry.

### 3.4 The per-asset fallback policy (§14.4) — closed option set, NO zero-power fallback

The closed variant set (a closed enum; no seventh variant; the universal
"set power to zero" fallback is STRUCTURALLY ABSENT — a strict-parse test
proves `SET_POWER_TO_ZERO` is a load error):

| Variant | Meaning | Engine enforcement |
|---|---|---|
| `HOLD_LAST_ACCEPTED_SETPOINT` | hold the last accepted setpoint for a BOUNDED interval | interval REQUIRED positive at load (an unbounded hold is a load error); the activation record carries the interval and the §14.4 caution that the hold is not asserted safe after topology/state change |
| `RETURN_TO_LOCAL_EMS_AUTONOMOUS` | hand control back to the site EMS's autonomous mode | recorded on the activation |
| `PRESERVE_RESERVE_SOC_CONSTRAINTS_AND_STOP_ACCEPTING_NEW_COMMANDS` | keep reserve/SOC constraints intact; refuse new commands | the engine ENFORCES the stop half: subsequent commands for the asset are refused (`fallback_stop`) before the forwarder is ever invoked |
| `REQUIRE_OPERATOR_INTERVENTION` | no automatic action | recorded on the activation |

Three honest non-activation outcomes (the P4.17 engine precedent, cited):

| Outcome | FALLBACK transition? | Audit |
|---|---|---|
| named policy, trigger configured | YES (frozen edge) | `EDGE_COMMAND_FALLBACK` |
| named policy, trigger NOT configured | no | `EDGE_COMMAND_FALLBACK_TRIGGER_NOT_CONFIGURED` |
| no policy for the asset (the explicit DEFAULT) | no — `REQUIRE_OPERATOR_INTERVENTION`, no automatic setpoint change | `EDGE_COMMAND_FALLBACK_INTERVENTION_REQUIRED` |

**Configuration** (`flexcore-edge-fallback-policy-v1`, line-oriented,
strict parse — unknown fields/versions/variants/triggers are named load
refusals), one block per asset:

```text
flexcore-edge-fallback-policy-v1
asset = ess-01
variant = HOLD_LAST_ACCEPTED_SETPOINT
bounded_interval_millis = 300000
triggers = nack,timeout,no_ack,tracking_deviation
reviewed_by = A. Operator (site operator)
reviewed_on = 2026-11-13
```

**The operator-review field is REQUIRED to load** (`reviewed_by` +
`reviewed_on`; missing, empty, or absent → the WHOLE config is refused).
**IN THIS LOOP THE REVIEW IS NOT SIGNED**: the name and date are recorded
as HUMAN GATE INPUT — an unsigned text field, never renderable as a
cryptographic sign-off. Signing the review (and binding it to the config
digest) is a recorded deployment follow-up.

## 4. DEFAULT-labelled decisions (this wave's defaults, each reversible by a reviewed change)

- **DEFAULT:** the command signature digest is keyed FNV-1a (HMAC-STYLE),
  not a cryptographic MAC. Same reason and consequence as the P6.14
  enablement digest (§2.1).
- **DEFAULT:** the sequence check is evaluated LAST (after allowlist and
  scope) so refused commands never advance the window; the ledger's 1–4
  numbering is preserved in the reason taxonomy, not the evaluation order.
- **DEFAULT:** clock quality `Unknown` fails the expiry check closed
  (never silently assumed good, P0.09 §4), not just `Unsynchronized`.
- **DEFAULT:** the P6.17 policy gate is consulted TWICE per forwarded
  command (receiver admission + engine re-check), so a forwarded command
  consumes two rate-limit budget units. Recorded consequence: effective
  per-adapter budgets halve for fully-forwarded commands. The
  independence of the re-check is deliberate (P6.16 step 1).
- **DEFAULT:** an asset with NO fallback entry resolves to
  `REQUIRE_OPERATOR_INTERVENTION` with NO FALLBACK transition (the P4.17
  default-entry precedent) — silence never becomes a setpoint change.
- **DEFAULT:** transport failures map to the `timeout` fallback trigger
  (the adapter did not accept the command); the audit detail keeps the
  transport/timeout distinction.
- **DEFAULT:** the fallback operator review is recorded but NOT SIGNED
  (human gate input; §3.4).
- **DEFAULT:** the engine's tracking state is in-memory (a restart clears
  it); persistence of command-tracking state is P6.18's
  audit-buffer/offline scope (the same limitation the P6.17 rate-window
  state recorded).
- **DEFAULT:** the `GatedCommandPipeline` lives in `command_path.rs`
  (additive section) because the task wiring point is the command path;
  the module-level type references (`command_verify`,
  `command_forward`) are one-directional consumers — intra-crate module
  references resolve without a dependency cycle at the crate level.

## 5. Human gate inputs recorded (what a deployment MUST supply)

| Input | Owner | Status |
|---|---|---|
| Command-signing key material + its non-secret reference | deployment secret manager (§17.3); loaded by the composition root into `CommandSigningKey` | DEPLOYMENT REQUIREMENT — no key material exists in code, tests, or fixtures (test keys are labelled lab fixtures) |
| Cryptographic upgrade of the `edge-sig` scheme | deployment + reviewed key management | DEPLOYMENT REQUIREMENT (recorded; the versioned prefix makes it detectable) |
| Edge clock-quality status | P6.31 (time sync + clock-quality monitoring); supplied as a `verify` parameter | SEAM — not implemented here; fail-closed without `Synchronized` |
| Signed operator review of the per-asset fallback policy | site/equipment operator | recorded, NOT SIGNED in this loop (human gate input) |
| Per-asset fallback policy content | site/equipment operator, reviewed per asset (never copied across assets unreviewed — the ledger risk) | config format delivered; content is deployment input |

## 6. Honest limitations

- Everything is UNRUN (see the header): no build, no test execution, no
  measurement. The three permitted verification tools were run (output in
  the handover note); they check documentation and dependency direction,
  not Rust compilation.
- The signature is a keyed checksum, not a cryptographic MAC (§2.1).
- No real adapter write path exists or is simulated on the wire: the
  forwarder port has a scripted test double (in-process) and a fail-closed
  production stub only. No protocol bytes are parsed anywhere.
- The engine's tracking/stop state is in-memory; a restart clears it
  (P6.18 scope). The sequence checkpoint, by contrast, IS durable.
- The fallback activation records the §14.1 FALLBACK transition at the
  edge; the platform-side frozen-lifecycle journal (P4.10 engine) is not
  driven from here — the edge does not re-encode frozen lifecycle edges,
  it records the transitions the §14.1 table defines for its own surface.
- acknowledgement ≠ delivery is carried as wording on every
  acknowledgement record; nothing in this wave can observe a physical
  device effect (and must not claim to).

## 7. Seams served (not implemented)

- **P6.22 (performance/scale):** the verifier and engine are allocation-
  light per command (a hash-map lookup, one append record per accepted
  command); no figures are invented — P6.22 owns measurement.
- **P6.23 (security testing):** the distinct rejection codes
  (`signature`/`expiry`/`sequence`/`duplicate`/`allowlist`/`scope`) and the
  audited forwarding refusals (`allowlist`/`policy`/`fallback_stop`) are
  the hooks P6.23's forgery/replay/auth-bypass cases assert against. The
  keyed-checksum limitation (§2.1) is a P6.23-relevant finding recorded at
  implementation time.
- **P6.25 (console surfacing):** `CommandForwardingEngine::lifecycle_state_of`
  and `stopped_assets` are the read-only views a Sites-workspace command
  panel can render; enabling anything happens exclusively through the
  P6.14 procedure.
- **P6.31 (time sync):** supplies the `ClockQuality` input; the fail-closed
  rule here is the consumer contract P6.31's objective text names.
- **P6.18 (offline audit buffer):** the command-path audit events ride the
  `AuditSink` port and inherit P6.18's durable buffering when composed.

## 8. Verification status (honest)

- `python tools/verify_docs.py`, `python tools/validate_agent.py`,
  `python -I tools/check_dependency_rule.py` — run at wave end; their
  ACTUAL output is recorded in the agent handover note. They do not
  compile Rust.
- `cargo build -p flexcore-edge-agent`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test -p flexcore-edge-agent` —
  NOT RUN (banned this loop). Status: UNRUN — the human build/test pass
  owns them.
