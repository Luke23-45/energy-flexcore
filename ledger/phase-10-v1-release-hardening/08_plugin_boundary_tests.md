<!-- P10.08 — Plugin permission-boundary tests (decision record). REAL test files WRITTEN-UNRUN over the actual crate seams (ENTRY_NOTE boundary 3): zero observed failures are NOT claimed from an unrun suite; the run is the human build/test gate. Release-candidate identifier PENDING P10.18. -->
# P10.08 — Plugin Permission-Boundary Tests

Date: 2026-11-14. Subtask: P10.08 (`ledger_phase_10.md` §P10.08). Owner role:
Security (with platform engineering). Report trace: §22.3 release gate 7; §5.7;
§9.3; §5.4–§5.6; §13.4 (ADR-010); §17.5 invariant 7. Inputs: P10.01 scope
freeze; the §5.5/§5.7 manifest and permission model as implemented
(`crates/flexcore-kernel/src/manifest.rs` P7.05, `lifecycle.rs` P7.07); the
plugin-host broker/enforcement/limits seams (`apps/plugin-host/src/broker.rs`
P7.11, `enforcement.rs` P7.12, `limits.rs` P7.10, `host.rs` P7.10); the Phase
7 conformance suites (P7.12/P7.13/P7.07) — cross-referenced, not redone.
Deliverable: two additive test files (`apps/plugin-host/tests/` and
`crates/flexcore-kernel/tests/`); no `src` logic touched; no other crate
touched.

## 1. Honest status — the plain statement

**The suite is WRITTEN, never executed.** The loop bans were held; per
ENTRY_NOTE boundary 3 this is "implemented, awaiting human build/test".
**Zero observed failures are NOT claimed** — an unrun suite has no observed
failures and no observed passes; the ledger acceptance "every escalation
attempt failed and was logged — zero successes" is an outcome the human run
must produce, and this record claims only that the attempts are WRITTEN with
hand-computed expected refusals from the named modules' own refusal codes.
The suite also runs against a release candidate that does not exist as a
built artifact yet (ENTRY_NOTE boundary 1): `<PENDING P10.18>`.

| Field | Value |
|---|---|
| Release-candidate identifier | `<PENDING P10.18 — no candidate build exists>` |
| Suite result | `<EMPTY — WRITTEN-UNRUN; the human run fills this>` |
| Escalation attempts executed | 0 executed / 100+ WRITTEN (see §2) |
| Plugin classes covered | A (`builtin-rust`), B (`wasm-component`), C (`grpc-worker`) — at the manifest gate; every holder held to identical broker/door rules |

## 2. What was produced (files)

| File | Content |
|---|---|
| `crates/flexcore-kernel/tests/p10_08_plugin_lifecycle_boundary.rs` | The kernel slice: (1) the manifest gate refuses EVERY never-granted permission (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`) for EVERY plugin class — 4 × 3 = 12 cells, each the DISTINCT named error `PERMISSION_NEVER_GRANTED` (the earliest point a never-granted request can be refused; the broker re-refuses later); unknown ids (`PERMISSION_ID_UNKNOWN`) and the verdict-shaped id (`dispatch.validate.verdict`) refused per class; the allowed vocabulary still parses per class; (2) the §5.6 PLUGIN lifecycle tables (the tables P10.03 recorded as complemented here) — the FULL 100-pair matrix (10 states × 10): the transcription oracle (12 frozen §5.6 edges + 5 recorded crash edges = 17 legal; 83 illegal; counts pinned 17/83/100; no edge in both tables; no outgoing edge from a terminal state except the recorded `Failed → Draining` cleanup); through the MACHINE: illegal transitions are `ILLEGAL_TRANSITION` with the state UNCHANGED (upgrade-from-Discovered, complete-drain-from-wrong-state, reject/fail from wrong states, version change from non-Stopped, revive-a-rejected-plugin); (3) gate refusals fail closed (`GATE_REFUSED`, state unchanged, audited; the §5.6 exit edges applied EXPLICITLY via reject/fail/crash); (4) drain-before-replace — replacement blocked while `Ready` and while `Draining` (`REPLACEMENT_BLOCKED_UNDRAINED`, audited), the MID-DISPATCH upgrade refused (drain deadline exceeded → stop NOT committed → incumbent RESTORED to `Ready` pending operator decision, both records audited), the version change recorded ONLY after a completed drain, the DEFAULT 30 s drain deadline pinned; (5) no auto-restart — a failed plugin STAYS `Failed` (`failed_from` evidence retained), the explicit operator restart is the ONLY path out and is audited (`EXPLICIT_RESTART`). |
| `apps/plugin-host/tests/p10_08_plugin_permission_boundary.rs` | The plugin-host slice: (1) the never-granted set refused at EVERY boundary — broker grant (`PERMISSION_NEVER_GRANTABLE` × 4), broker check (crafted calls refused AND audited, `PLUGIN_CAPABILITY_DENIED`, × 4), the enforcement door for EVERY world (4 × 3 worlds = 12 audited door refusals, before any component code runs), and a profile ceiling carrying one (misconfiguration refused); (2) broker default-deny — every capability with no grant is `CAPABILITY_NOT_GRANTED` and audited; revocation takes effect at the NEXT call (no cached authority; unknown-grant revocation an error); per-call expiry with the hand-computed exclusive boundary (`expires_at − 1` live, `expires_at` expired); every scope outside the envelope a named denial (`..` path escape, sibling prefix `/data/telemetry-evil`, wrong send-only target, another tenant, cross-kind scope); `dispatch.validate.request` grantable ONLY to the protected-validator target; (3) enforcement at the door — the verdict is FROZEN at instantiation (cloned grants; a mutated request cannot expand it) while broker revocation tightens the live authority at the next call; `instantiate` REQUIRES a verdict and the default build refuses at the binding seam (`BINDINGS_NOT_GENERATED`); audit-or-refuse both directions (an unauditable ALLOW is rolled back — the broker holds nothing; an unauditable refusal surfaces `AUDIT_UNAVAILABLE`); over-broad requests refused never trimmed (`PERMISSION_ABOVE_PROFILE`; the empty ceiling permits nothing); atomic derivation batches (never half-applied; duplicates refused); (4) the ADR-010 TYPE-SYSTEM PROOF exercised — `request_validation_forward` returns `()` and the test BINDS the return type (`let (): () = …`): a validation verdict cannot flow back through a signature that has no verdict in it; the exhaustive no-authority sweep (every allowed capability grants a proposal-only set naming no authorize/issue; every crafted authority id refuses); (5) limits/fuel/deadline refusal codes — NO default fuel budget (`LIMITS_INVALID` on `fuel-budget` = 0), every declared bound re-validated against the kernel's closed ranges per field, the exact 1 MiB = 16 wasm-page conversion, epoch-decimal round-UP boundaries hand-computed (1/10/11/3 600 000 ms → 1/1/2/360 000 ticks), the door refusing resource-bound lies and vocabulary drift (`PERMISSION_VOCABULARY_DRIFT` for `timeoutMs` and a manifest-side `fuel-budget` field — never a default interpretation), and the five enforcement-contract codes (`MEMORY_LIMIT_EXCEEDED`/`DEADLINE_EXCEEDED`/`FUEL_EXHAUSTED`/`OUTPUT_LIMIT_EXCEEDED`/`CALL_RATE_EXCEEDED`) named and distinct. |

## 3. Coverage against the subtask steps

| Step | Requirement | Where proven |
|---|---|---|
| 1 | Declared capabilities/permissions vs manifest; activation validates manifest, publisher, version, capabilities, permissions (§9.3) | The kernel manifest gate slice (never-granted × class, unknown ids); the derivation-trail fields (manifest id/version) carried on grants and asserted on the verdict |
| 2 | Privilege escalation from an untrusted plugin fails and is LOGGED | The broker grant/check/door refusal sweep (every attempt audited — `PLUGIN_CAPABILITY_DENIED` / `PLUGIN_INSTANTIATION_REFUSED`); the scope-escape sweep; `secret.read`/`network.any` ungrantable; WASM ambient access structurally absent (zero `wasi:` imports, the gated empty linker — cited from `bindings.rs`/`wit/README.md`, not re-tested here); direct device-protocol access: the crafted `device.protocolspeak` id is `PERMISSION_UNKNOWN` |
| 3 | WASM sandbox limits enforced (memory, fuel/time, concurrency, output size, call rate — §5.4 Class B); signatures/provenance required for production installation | The limits/fuel/deadline refusal-code slice (closed ranges re-validated at the door; no default fuel; the enforcement-contract codes are the host's named refusals — the RUN-time enforcement points live in the compile-gated `bindings.rs` and are UNVERIFIED until the first wasmtime build, recorded). Production-install signatures are P7.14/P9.34's seam — cross-referenced |
| 4 | Lifecycle safety (§5.6): no activation/upgrade mid-dispatch-transaction; drain → stop → replace with the version change recorded; no hot reload | The lifecycle slice (replacement blocked while Ready/Draining; deadline-exceeded restores the incumbent; version change only from Stopped; the capability set frozen at instantiation — the only path to a new set is re-instantiation through drain-before-replace) |
| 5 | No plugin self-upgrade or runtime code fetch | No auto-restart, no hot reload, explicit-only restart; the broker holds no code-fetch capability (no such id in any vocabulary); runtime fetch would be a new capability id — `PERMISSION_UNKNOWN` by construction (exercised via the crafted-id sweep) |

## 4. Reviewer-verification procedure (the human gate)

```sh
cargo test -p flexcore-kernel --test p10_08_plugin_lifecycle_boundary
cargo test -p flexcore-plugin-host --test p10_08_plugin_permission_boundary
# Independent verification (the ledger's method): execute at least ONE
# escalation attempt per plugin class and inspect the host capability-policy
# enforcement audit log for the denial records — the broker/door tests carry
# per-attempt audit assertions the reviewer can spot-check.
```

## 5. Acceptance-criteria coverage and honest gaps

| P10.08 criterion | Covered here | NOT executable in this loop |
|---|---|---|
| (a) every escalation attempt failed and was logged — zero successes | All attempts WRITTEN with hand-computed named refusals + per-attempt audit assertions | The zero-successes RESULT is the human run's outcome; not claimable from an unrun suite |
| (b) sandbox limits are enforced and evidenced | The limit-construction/door-refusal codes are proven; the DECLARED-bounds re-validation is proven | The RUN-time enforcement (a real guest exceeding memory/fuel/deadline) needs the wasmtime build (`generated-bindings` feature) — the recorded UNVERIFIED wasmtime call sites in `bindings.rs`; a real-component drill is a candidate-build gate |
| (c) lifecycle rules hold for an upgrade during an active dispatch transaction | The mid-dispatch upgrade refusal: drain deadline exceeded → stop refused → incumbent restored → replacement still blocked; the version change only after a completed drain | A drill with a REAL dispatch transaction in flight on the composed host is a candidate-build observation |
| (d) no plugin self-upgraded or fetched code at runtime | Structural: no restart path but the audited explicit one; no code-fetch capability id exists (crafted-id sweep); the capability set is frozen at instantiation | — |

## 6. Findings (recorded, not fixed — src logic is outside this subtask's ownership)

- **F-P10.08-1 (audit-completeness gap, observed behaviour pinned by test,
  no action taken):** in `crates/flexcore-kernel/src/lifecycle.rs`, an
  ILLEGAL transition refusal (`LifecycleError::IllegalTransition`) is
  currently NOT emitted as a `LifecycleAuditRecord` — the `illegal()`
  helper constructs the error without a `Refused` emission — while the
  module header states "EVERY transition — committed or refused — emits a
  `LifecycleAuditRecord`". Gate refusals, drain-deadline refusals, and
  replacement refusals ARE audited. The P10.08 suite pins the current
  behaviour with an explanatory assertion (F-P10.08-1 marker in
  `p10_08_plugin_lifecycle_boundary.rs`) so a fix makes the gap visible; the
  §5.6 step-4 audit requirement ("every transition, with plugin ID/version,
  actor, and reason") suggests the emission belongs on the illegal path too.
  Recorded as a follow-up, not fixed here.
- **F-P10.08-2 (recorded constraint):** the wasmtime call sites that apply
  the runtime limits (epoch deadline, fuel metering, memory limiter) are
  compile-gated behind `generated-bindings` and carry the recorded
  UNVERIFIED-wasmtime-API labels; the door-level proof (this suite) is
  necessary but NOT sufficient for acceptance criterion (b) — the first
  wasmtime build must evidence a real limit enforcement drill before the
  criterion is claimed met.
- **F-P10.08-3 (recorded constraint):** the plugin-host permission
  vocabulary and resource-field names are documented MIRRORS of the kernel
  manifest gate (the recorded alignment seam, broker.rs/enforcement.rs
  headers). The suite asserts the never-granted set is identical on both
  sides and disjoint from the grantable set; a deliberate mirror update on
  one side without the other is a named drift refusal
  (`PERMISSION_VOCABULARY_DRIFT` / closed-vocabulary failure) — fail-closed
  in both directions by construction.
- **F-P10.08-4 (observation, no action taken):** the enforcement door's
  audit-or-refuse rollback revokes grants whose ALLOW audit failed, but the
  revocation audit itself may also fail (the broker's recorded asymmetry —
  a revocation stands even if unaudited). Authority is dropped (safe), but
  the audit trail for such a rollback may be incomplete. Recorded for the
  audit-completeness follow-up alongside F-P10.08-1.

## 7. Ledger tick

`P10.08` in `ledger_phase_10.md` was ticked in the same wave as this record,
with the ENTRY_NOTE boundary 3 meaning: **implemented, awaiting human
build/test**. The run-time sandbox-enforcement drill (criterion b's wasmtime
half) and the executed escalation matrix remain open human-gate steps, not
done.
