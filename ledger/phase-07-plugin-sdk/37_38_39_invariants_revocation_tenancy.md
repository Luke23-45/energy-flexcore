# P7.37 + P7.38 + P7.39 — §17.5 invariant map, plugin revocation/rollout/removal, and plugin-path tenant scope: IMPLEMENTED (awaiting human build/test)

Date: 2026-11-13. Subtasks: P7.37, P7.38, P7.39 of Phase 7
(`ledger_phase_7.md`). Owner roles: Security engineer + architect
(P7.37), Security + DevOps (P7.38), Security engineer (P7.39). Report
trace: P7.37 — §17.5, §14, §21.4, §13.4, ADR-005/ADR-010; P7.38 — §17.4
(supply-chain compromise: signed artifacts, provenance, SBOM, pinned
dependencies, staged rollout), §5.6, §9.3; P7.39 — §17.4 (cross-tenant
leakage, compromised plugin), §5.7, §16.2, [R25], ADR-007.

**Run-status honesty (AGENTS.md rule 7; the loop bans carried from the
P7.05/P7.06 and 19_20_36 records):** the code and tests in this wave are
WRITTEN, NOT BUILT and NOT RUN. No `cargo` command of any kind was
executed (no build, no check, no test, no clippy, no fmt). Every test
below is UNRUN until the human build/test pass executes it. Nothing here
claims a test passed, a build succeeded, or a compile completed. The only
tools run were the three permitted verification scripts (§10, output
quoted verbatim).

## 1. Files (complete list; nothing outside it was touched)

| File | Status |
|---|---|
| `ledger/phase-07-plugin-sdk/37_safety_invariant_map.md` | NEW (P7.37 deliverable — the invariant→enforcement→test map) |
| `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs` | NEW (P7.37 test-side twin — the matrix suite) |
| `crates/flexcore-kernel/src/revocation.rs` | NEW (P7.38) |
| `crates/flexcore-kernel/tests/revocation_p7_38.rs` | NEW (P7.38 tests) |
| `crates/flexcore-kernel/src/tenant_scope.rs` | NEW (P7.39, kernel side) |
| `crates/flexcore-kernel/tests/tenant_scope_p7_39.rs` | NEW (P7.39 kernel tests) |
| `apps/plugin-host/src/tenant_gate.rs` | NEW (P7.39, host side — the mirrored cross-check gate) |
| `apps/plugin-host/tests/tenant_scope_p7_39.rs` | NEW (P7.39 host tests) |
| `crates/flexcore-kernel/src/lib.rs` | EDITED (ADDITIVE ONLY: two module-table rows + two `pub mod` blocks; no existing line reordered or changed) |
| `apps/plugin-host/src/lib.rs` | EDITED (ADDITIVE ONLY: one module-table row + one `pub mod` line) |
| `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | EDITED (P7.37/P7.38/P7.39 ticks ONLY — P7.28/P7.29 belong to the concurrent agent and were not touched) |
| this file | NEW (decision record) |

No dependency was added to either crate (the kernel stays
`serde_json`-only via the existing workspace pin; the plugin host stays
std + tracing + the wasmtime pin). The root `Cargo.toml`, `ci.yml`,
`docs/foundations/*`, `docs/plan/*`, `.agent/*`, and
`tools/check_dependency_rule.py` were NOT touched. Both crates' existing
modules are CLOSED: the only edits to pre-existing source files are the
additive `lib.rs` module registrations above.

## 2. P7.37 — the §17.5 safety-invariant test map

Deliverable: `37_safety_invariant_map.md` — for each §17.5 invariant
(1–10, quoted verbatim) plus the §14/§21.4 boundary rows:

- the rule → the SDK-level enforcement point(s) → the proving test(s)
  (exact file + test fn) → the residual risk, honestly;
- where the SDK genuinely cannot enforce a rule (it lives in the
  domain/application protected path — the kernel is a composition crate
  and "must not become a second domain layer"), the row says so and cites
  the Phase 3/4 enforcement and its exact tests
  (`29_safety_invariant_matrix.md` rows), never claiming SDK coverage;
- invariant 10 stays a documented boundary (no code test by design — a
  "test" of site-local independence would be a protection-grade claim,
  banned by AGENTS.md §3 rule 3);
- the one MAP-GAP is recorded, not silently filled: the invariant-9
  END-TO-END composition test (plugin proposal → feasibility gate →
  stale-model rejection in one harness) is owned by **Phase 8**
  (`31_phase8_handover.md`); the SDK slice (no feasibility-passing
  variant on the plugin path) plus the cited Phase 4 fail-closed suite
  cover the layers that exist.

Test-side twin: `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs`
— a compact matrix suite over the SDK-enforceable rows: never-granted ids
at every SDK gate; protected-kernel refusals at resolver AND path; the
ADR-010 no-authority-variant proof (exhaustive match + statement pins);
expiry fail-closed with the sequence-check-runs-LAST order proof (an
expired-and-replayed request is refused as EXPIRED and never moves the
window); the §21.4 mode lock; drain-before-replace + the P7.38 revocation
gate on the replacement path; the P7.39 tenant-scope slice; and the
honesty-statement pins (a map citing statements that no longer exist
would be invented coverage — the tests tripwire that).

Ledger acceptance mapping: all ten invariants mapped with named tests
(§4 of the map); the invariant-9 SDK slice test is written (the stale-
model REJECTION tests are the cited Phase 4 suite — the map says exactly
which layer owns which half); invariants 1 and 6 cite live Phase 4 tests
plus their SDK slices.

## 3. P7.38 — plugin revocation, staged rollout, and failed-plugin removal

`crates/flexcore-kernel/src/revocation.rs` completes the P7.14 seams
(recorded in `14_15_16_signing_profiles_recomposition.md` §2.4 as
design-for-later; this wave implements them):

1. **The append-only revocation register.** `RevocationKey` is a closed
   two-kind enum: `(plugin id, version)` and `signer key reference`
   (revoking a key revokes everything it signed — the join key is the
   P7.14 provenance record's `signer-key-reference` field; no new
   provenance field needed). The `RevocationStore` port is APPEND-ONLY
   by contract (`append` + `load_all` — there is no removal/update/
   override method on the trait), keyed deterministically
   (`plugin/<id>@<ver>`, `signer/<ref>`) in `BTreeMap` order. A
   duplicate revocation is `REVOCATION_ALREADY_REVOKED`, never a silent
   rewrite. **UNREVOCATION_SEAM (recorded, not implemented):**
   un-revocation is a NEW signed record type with its own policy
   decision and decision record; v0.5.0 implements only the safe
   direction.
2. **Two store implementations.** `MemoryRevocationRegister`
   (deterministic collector) and `FileRevocationRegister` — the
   `buffer_file.rs` FRAMING DISCIPLINE (cited, REIMPLEMENTED — the
   kernel depends on no flexcore crate), same framing as the P7.36
   sequence checkpoint: 24-byte magic + version byte, length-prefixed
   CRC-32 records, fixed-layout binary payload; corrupt tail is
   truncate-and-COUNT (never silent), a foreign file is a hard open
   error, and the append is flushed + `sync_data` BEFORE the in-memory
   state moves (durability before state).
3. **Wiring as lifecycle gate outcomes.** `revocation_gate` is consulted
   FIRST at `Discovered → Verified` — before
   `signing::provenance_gate` — so a package whose signature was revoked
   after signing is refused BEFORE any digest computation, with the
   stable tags `REVOKED_PLUGIN` / `REVOCATION_CHECK_FAILED` /
   `REVOKED_SIGNER_KEY` riding the P7.07 `GateOutcome` (the lifecycle
   refuses fail-closed and audits, exactly like any other gate refusal).
   A register that cannot be read is a FAILED gate — never "not
   revoked" (the P6.06 revocation-seam rule).
4. **Registry-time re-check.** `ensure_startable` re-consults the
   register immediately before `Configured → Started`: a plugin revoked
   AFTER verification cannot START, and the same check applies after an
   explicit restart (the register is consulted again at the fresh
   `Discovered → Verified`, so a revoked plugin cannot re-verify).
5. **Staged rollout.** `StagedRollout` is the per-profile staging plan —
   cohort/percentage-free BY CONSTRUCTION (`ROLLOUT_STATEMENT`; there is
   no cohort, percentage, or bulk field anywhere on the type): a new
   version is added to ONE profile first, then promoted profile-by-
   profile in the plan's DECLARED order. Promotion succeeds ONLY for the
   exact next un-promoted profile — promoting the last profile first
   (the flag-flip full rollout) and any out-of-order/skipped promotion
   are `PROMOTION_OUT_OF_ORDER`; an unknown profile is
   `PROMOTION_UNKNOWN_PROFILE`; a same-version plan is `ROLLOUT_NO_OP`.
   Every promotion and refusal is audited
   (`PLUGIN_ROLLOUT_PROMOTED` / `PLUGIN_ROLLOUT_PROMOTION_REFUSED`).
6. **Failed-plugin removal.** `remove_failed_plugin` is the EXPLICIT
   removal path for a `Failed` plugin (the P7.30 drill item): reason and
   actor REQUIRED; legal ONLY from `Failed` (a running plugin goes
   through the normal drain-before-replace path); it drives the §5.6
   `Failed → Draining → Stopped` edges through the plugin's `DrainPort`;
   quiesced → `PLUGIN_REMOVAL_COMPLETED`; deadline exceeded → the
   lifecycle's fail-closed restoration applies (no forced stop), the
   refusal is audited WITH the containment instruction (revoke the
   plugin / drop its broker grants while the operator decides), and
   `REMOVAL_DRAIN_DEADLINE_EXCEEDED` is returned. Removal deletes
   NOTHING: the lifecycle's records persist in the (append-only) audit
   sink/store — a removed plugin, including a malicious one, keeps its
   full evidence trail (tested explicitly).
7. **Audit.** A fourth kernel-local audit port
   (`RevocationAuditSink`/`RevocationAuditRecord`, append-only contract,
   injected `Clock` only — `SystemTime::now()` appears nowhere), with
   eight stable SCREAMING_SNAKE kinds. A separate port (rather than
   overloading the lifecycle's transition-record semantics) is the
   recorded choice; it composes into the host audit path at the
   composition root like the lifecycle port does.

Ledger acceptance mapping (all tested in `revocation_p7_38.rs`, 10
tests): revoked-at-install refused at `Discovered → Verified` (before
provenance, with a valid-provenance fixture proving the order);
revoked-while-running stopped and audited (start re-check refuses; the
drain/removal path is §6 above; the revocation audit is emitted at every
step); no un-staged full rollout (last-profile-first refused; order
enforced); a `Failed` (including malicious) plugin is removable with no
residual work (`outstanding_work() == 0` after quiescence) and the audit
trail preserved.

## 4. P7.39 — cross-component isolation and data-level tenant filtering

### 4.1 Kernel side — `crates/flexcore-kernel/src/tenant_scope.rs`

- **`TenantScope`** (tenant id + optional site narrowing): fields
  PRIVATE; `TenantScope::from_authenticated` is the ONLY public
  constructor — the scope is HOST-STAMPED from the platform-
  authenticated context, NEVER from any plugin-supplied field (report
  §16.2 verbatim intent; the `TenantContext::from_authenticated_claims`
  and `EdgeSessionScope` server-side patterns). A plugin cannot widen
  its own scope.
- **`ScopedRequestEnvelope`** — the WRAPPER (the recorded choice over
  rewriting the closed P7.36 type): composes `PluginRequestEnvelope`
  unchanged (the four plugin-path checks run on `inner()`) and binds the
  scope into the request identity — the wrapper digest covers the
  scope's canonical form PLUS the inner envelope's canonical form
  (`TENANT_SCOPE_BINDING_MISMATCH` on a scope swap or inner mutation;
  deterministic; same FNV-1a-64 test-grade digest honesty — a production
  digest remains a DEPLOYMENT REQUIREMENT).
- **Data-level filtering.** The scope TRAVELS with the request;
  `enforce_scope` / audited `enforce_scoped_request` cross-check the
  REQUESTED data scope against the envelope scope at enforcement —
  narrowing preserved (site request inside a tenant-wide envelope
  passes), everything wider/different refused as `TENANT_SCOPE_MISMATCH`
  and audited (`PLUGIN_TENANT_SCOPE_REFUSED`; §16.2 "Audit cross-tenant
  access denials"). A plugin-ECHOED scope that differs from the stamped
  one is `TENANT_SCOPE_ECHO_MISMATCH`.
- **The RLS relationship, recorded (§16.2/[R25]; ADR-007):** plugins
  never touch tables; data access flows through the platform's
  tenant-scoped services; application authorization + PostgreSQL RLS
  remain the platform's defence in depth (the table-owner/`FORCE ROW
  LEVEL SECURITY` caveat included); the SDK adds the ENVELOPE-LEVEL
  guarantee only. RLS is not re-implemented, weakened, or replaced.

### 4.2 Host side — `apps/plugin-host/src/tenant_gate.rs` (new module)

The host crate depends on NO flexcore crate (crate header;
`check_dependency_rule.py` classifies it COMPOSITION), so the scope is
MIRRORED as `HostTenantScope` — the documented alignment seam, same
pattern as the broker's permission-vocabulary mirror. The
`TenantScopeGate` composes the P7.11 broker's per-call narrowing with
the host-stamped envelope check, in a FIXED order: envelope FIRST, then
grant — so a misconfigured or over-broad GRANT can never widen what the
authenticated context stamped (two independent narrowing layers; tested
with a deliberately tenant-wide grant). Only broker `DataScope` requests
pass the data gate (`TENANT_SCOPE_KIND_INVALID` otherwise, never
coerced). Every admission and denial is audited (audit-or-refuse in BOTH
directions — an unauditable admission never reaches the caller). The
worker-tenancy model is RECORDED (`WORKER_TENANCY_STATEMENT`): one
out-of-process gRPC worker per plugin instance, launched by the host
that instantiated it — NO worker process is shared across tenants; the
tested case is the per-tenant worker; a shared multi-tenant worker
would be a new architecture with its own decision record (ledger P7.39
step 3).

### 4.3 Wiring (additive)

`plugin_path_protection.rs` was NOT modified — the envelope type gains
the scope through the wrapper, per the task's preference. The host
enforcement layer gains the cross-check as the new `tenant_gate` module
(no existing host module's logic changed; the `lib.rs` registration is
additive). Cross-component isolation in the shared host itself remains
the P7.10/P7.13 WASM sandbox + broker structure (no ambient access,
per-call re-checks, component-per-instance memory) — this wave adds the
DATA-level tenant guarantee on the request path and records the worker
model; the P7.13/P7.23 suites remain the cross-component evidence.

Ledger acceptance mapping (tested in both new suites): scope derivation
from authenticated context only; plugin-supplied scope ignored/refused
(echo mismatch); mismatch refusal at enforcement (`TENANT_SCOPE_MISMATCH`,
envelope-first, misconfigured-grant proof); narrowing preserved;
cross-tenant attempt end-to-end refused + audited (both layers); wrapper
determinism (equal inputs → equal digests; mutation-sensitive).

## 5. DEFAULT decisions recorded in this wave

| Decision | Choice | Rationale |
|---|---|---|
| Un-revocation | NOT implemented; recorded seam (`UNREVOCATION_SEAM`) | The append-only register keeps exactly one record per key; reinstatement is a new signed record type + policy decision (fail-closed: only the safe direction is implemented in v0.5.0). |
| Rollout staging model | Explicit per-profile staging ONLY; no percentage canary | The P7.15 profiles are the staging units; a percentage canary is not constructible from the type surface (the ledger risk: "a 'staged' rollout that is really a flag flip" is refused structurally). |
| Removal scope | `remove_failed_plugin` legal ONLY from `Failed` | Running plugins are removed through the normal drain-before-replace path (P7.07/P7.16); removal is the failed-plugin cleanup, not a bypass of the drain rules. |
| Drain-deadline expiry during removal | Refuse removal; the lifecycle restores the plugin to `Ready`; audit carries the containment instruction | The lifecycle's recorded no-forced-stop rule wins; containment of a possibly-malicious plugin goes through revocation/grant drops, which ARE implemented. |
| Tenant scope on the plugin path | Wrapper over the P7.36 envelope, not a rewrite | The P7.36 type is closed and proven; composition adds the scope binding without touching proven logic (the task's stated preference). |
| Host-side scope type | A documented MIRROR (`HostTenantScope`) | The host crate depends on no flexcore crate (dependency rule); the mirror is fail-closed in both directions (broker.rs seam pattern). |
| Revocation audit port | A fourth kernel-local port (`RevocationAuditSink`) | Reusing the lifecycle `AuditSink` would force revocation events into transition-record semantics; a separate port keeps every record kind honest. Composes into the host audit path at the composition root. |
| Percentage/canary cohorts | Absent (recorded) | v0.5.0 has no fleet to canary; the record is the guard against silently growing one. |

## 6. Honest boundaries (what is NOT claimed)

- No cryptographic signature, PKI, CRL/OCSP distribution, or revocation-
  list DISTRIBUTION mechanism exists: the register is the in-kernel/
  on-disk structure; how revocations reach a deployment (signed feed,
  operator action) is the recorded deployment seam, exactly like the
  P7.14 signer-identity requirement. The digest remains FNV-1a 64
  TEST-GRADE (P4.28/P6.14/P7.14/P7.36 honesty); production digests are
  DEPLOYMENT REQUIREMENTS.
- No safety, certification, or security claim is made anywhere (the
  §17.5 rules are system requirements, not a certification claim — the
  report's own closing sentence).
- The RLS layer is the platform's; the SDK adds the envelope-level
  guarantee only. Nothing here weakens §16.2.
- The invariant-9 end-to-end composition test is MAP-GAP, owning wave
  Phase 8 (map §4) — recorded, not silently assumed.
- All suites are UNRUN; the human build/test pass is the gate
  (`cargo test -p flexcore-kernel --test safety_invariant_p7_37
  --test revocation_p7_38 --test tenant_scope_p7_39` and
  `cargo test -p flexcore-plugin-host --test tenant_scope_p7_39`).

## 7. Review status

The ledger names Security + DevOps (P7.38) / Security engineer (P7.39) /
Security + architect (P7.37) review. That review has NOT happened (loop
roles are not human reviewers). Recorded as the gate input, matching the
P4.29 sign-off pattern: **NOT SIGNED**.

## 8. Ledger ticks

P7.37, P7.38, P7.39 ticked in `ledger_phase_7.md` with the exact
appendage required by the tick discipline (`implemented, awaiting human
build/test (decision record: …)`); no other line touched — P7.28/P7.29
belong to the concurrent agent.

## 9. File/test index

| File | Tests (UNRUN) |
|---|---|
| `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs` | 8 tests |
| `crates/flexcore-kernel/tests/revocation_p7_38.rs` | 10 tests |
| `crates/flexcore-kernel/tests/tenant_scope_p7_39.rs` | 7 tests |
| `apps/plugin-host/tests/tenant_scope_p7_39.rs` | 6 tests |
