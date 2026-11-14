<!-- P10.12 — Upgrade/rollback test on the release candidate. TEMPLATE + RC
     test plan: the upgrade path is defined BY CITATION of
     crates/edge-agent/src/update_verification.rs (verify-before-stage, the
     refusal order, the append-only register, the never-silent downgrade) and
     the NoUpdateVerifier gap is CARRIED (no real crypto, no update-bundle
     producer). The test matrix is written; the RUN RECORD IS EMPTY. HONEST
     STATEMENT (binding): NO UPDATE HAS EVER BEEN BUILT OR APPLIED, and NO
     SIGNATURE SCHEME IS IMPLEMENTED AT PRODUCTION GRADE. -->
# P10.12 — Upgrade/Rollback Test on the Release Candidate (RC test plan + matrix — nothing has been upgraded, rolled back, or applied)

Date: 2026-11-14. Subtask: P10.12 (`ledger_phase_10.md` §P10.12). Owner role:
Platform engineering. Report trace: §22.3 release gate 9 (second half); §21.2;
§5.6 (plugin lifecycle). Inputs: P10.01 scope freeze; the deployment model
(P10.24 draft — concurrent wave-4 sibling, cited by expected path
`ledger/phase-10-v1-release-hardening/24_deployment_upgrade_model.md`; it does
not exist yet and no number is taken from it); P10.13 (schema-migration
compatibility — concurrent wave-3 sibling, cited by expected path
`ledger/phase-10-v1-release-hardening/13_schema_migration_compat.md`); the
P9.34 verification seam (`crates/edge-agent/src/update_verification.rs` +
`ledger/phase-09-pilot-qualification/34_signed_update_verification.md`); the
P10.02 contract freeze (`02_api_contract_freeze.md`); ENTRY_NOTE boundaries 1
and 3.

## 1. Honest status — the plain statement (the required sentence, stated first)

**No update has ever been built, signed, staged, or applied — not on the
candidate, not on any prior build — and no signature scheme is implemented at
production grade.** The default verifier is `NoUpdateVerifier` and refuses
EVERYTHING; the only other verifier is the loudly-labelled test-grade
FNV-1a-64 checksum, which is NOT a security primitive; "signature" in this
repository is honestly a digest-and-record scheme (tamper-EVIDENCE, not
tamper-PROOF — `UPDATE_STATEMENT` in `update_verification.rs`). No
update-bundle producer exists anywhere (P9.34 §8 finding 1 — the largest
recorded gap), so there is literally nothing that could be upgraded TO. No
release candidate exists (ENTRY_NOTE boundary 1), no staging environment
mirrors a pilot deployment, and no upgrade or rollback has ever been
performed. This record delivers the upgrade path definition (§3), the
rollback register semantics (§4), the test matrix (§5), and the EMPTY run
record (§7). The ledger's acceptance criteria are NOT met and NOT claimed.

## 2. What the verification seam provides — by citation (not re-specified)

The edge update/rollback surface is IMPLEMENTED (P9.34, awaiting human
build/test) at `crates/edge-agent/src/update_verification.rs`. This record
cites it; it does not restate or extend it:

| Seam element (cited) | Where | What the RC test consumes |
|---|---|---|
| `UpdateGate::verify_before_staging` | `update_verification.rs` | verification BEFORE any write — the function takes NO path and performs NO filesystem write; a refusal is a return value + an audit record (`EDGE_UPDATE_VERIFICATION_REFUSED`), structurally incapable of touching disk |
| The refusal order (6 steps) | module header + `verify_inner` | 1. manifest present (`UPDATE_UNSIGNED_BUNDLE`) → 2. manifest valid (`UPDATE_MANIFEST_INVALID`) → 3. algorithm matches the configured verifier (`UPDATE_DIGEST_ALGORITHM_MISMATCH`) → 4. signer reference trusted (`UPDATE_UNTRUSTED_SIGNER`) → 5. version ≥ the manifest floor (`UPDATE_BELOW_MINIMUM_FLOOR`) → 6. digest verdict `Matches` (`UPDATE_DIGEST_MISMATCH` / `UPDATE_VERIFIER_REFUSED`) |
| `UpdateManifest` (v1 format `flexcore-edge-update-manifest-v1`) | the manifest struct + `validate` | version, bundle digest (lowercase hex), closed digest-algorithm vocabulary (`sha-256`, `fnv1a64-test`), scheme-prefixed signer key REFERENCE (never key material; the value never echoed), minimum-version floor, non-empty approval reference |
| `FileUpdateLedger` (magic `FLEXCORE-EDGE-UPDATE-REGISTER` + version byte 0x01) | the append-only register | length-prefixed CRC-32 records; truncate-and-audit recovery with a counted loss; durable BEFORE memory; active version + previously-activated set DERIVED by replay (the register is the source of truth — `UPDATE_SLOT_DECISION`) |
| `UpdateActivation::activate` / `::rollback` | the activation/rollback rules | duplicate-activation refusal; rollback rules in §4 below; audited `EDGE_UPDATE_ACTIVATED` / `EDGE_UPDATE_ROLLBACK` |
| `RollbackOverride` | the below-floor override | named actor + authority reference, BOTH mandatory; the override rides the register record AND the audit line |

## 3. The upgrade path (ledger steps 1–2) — defined for the RC, never executed

The RC upgrade path has three legs; every leg is UNRUN.

### 3.1 The central-plane leg (control plane, worker, console)

```text
source: the previous release or the pilot build  — DOES NOT EXIST (no prior
        release has shipped; the pilot build is a described procedure,
        never a built artifact). Honest status: the upgrade SOURCE is
        undefined; the first executable upgrade is v0.x/v1.0.0-rc → v1.0.0
        once both artifacts exist.
target: the v1.0.0 release candidate (the P10.18 candidate identifier).
path:   staging environment mirroring the pilot deployment → rolling
        upgrade, one component at a time → health/readiness probes between
        components → schema migrations per P10.13's compatibility windows
        → plugin upgrades per the §5.6 drain rules.
```

Rules the path is bound by (recorded now, before any run):

1. **Migrations carry their compatibility window** (P10.13's per-migration
   windows — which prior versions can run against the new schema and vice
   versa); the rolling upgrade never violates the window of any migration
   it applies. A migration with no window, or an irreversible migration,
   changes the path: rollback then requires restore-from-backup — which is
   exactly why **P10.11's RC restore must have been tested first** (the
   ledger's own risk text; P10.11 §6 is the EMPTY record that must be filled
   before a rollback-over-restore path is credible).
2. **Plugin lifecycle (§5.6 drain rules):** no plugin activation or upgrade
   mid-dispatch-transaction — drain → stop → replace, with the version
   change recorded; no hot-reload of safety or dispatch providers in the
   1.x line; no plugin self-upgrade or runtime code fetch (§9.3). The
   negative evidence for these rules is P10.08's test suite (concurrent
   wave-2 sibling); the RC upgrade exercises the same rules in a staging
   composition.
3. **In-flight dispatch transactions are not interrupted** by the rolling
   upgrade: health/readiness probes gate each component swap so a command
   mid-transaction is drained or failed per §14.4, never corrupted.
4. **Post-upgrade verification (ledger step 3):** version identifiers match
   the candidate; contract compatibility per P10.02 (the frozen REST/
   Protobuf/WIT/NATS surfaces — no silent breaking change); command/audit
   continuity (audit chain intact across the upgrade — count + chain head,
   by the P10.11 §3 RC-R-4 mechanics); a smoke dispatch-lifecycle pass in
   shadow (the §14.1 vocabulary round-trips on the upgraded build).

### 3.2 The edge leg — verify-before-stage, by citation

The edge upgrade applies a bundle through the P9.34 surface, in this order:

1. **verify-before-stage**: `UpdateGate::verify_before_staging(manifest,
   bundle)` — the six-step refusal order of §2, BEFORE any file is written;
   a refused bundle leaves ZERO records and ZERO files (the register,
   opened afterwards, holds zero new records — the property the P9.34 tests
   pin with an empty scratch directory).
2. **activate**: only a `VerifiedUpdate` reaches
   `UpdateActivation::activate` — the register appends the activation record
   (durable before memory) and audits `EDGE_UPDATE_ACTIVATED`; the NEW
   VERSION REPORT is the register record itself.
3. **the recommended deployment shape** (`UPDATE_SLOT_DECISION`, cited): the
   register is the source of truth for the active version; a two-slot A/B
   layout (the previous version retained untouched until the new one is
   verified and activated) is the RECOMMENDED shape — it is what makes §4's
   rollback a re-activation rather than a re-download.

**The carried gaps (the NoUpdateVerifier honesty, unchanged from P9.34):**

| Gap | Status | Consequence for this test |
|---|---|---|
| No real crypto | the default verifier is `NoUpdateVerifier` (refuses everything); no SHA-256-or-stronger verifier is implemented — wiring one through the `UpdateVerifier` port under reviewed key management is a recorded DEPLOYMENT REQUIREMENT | matrix cases U-2..U-5 are exercisable TODAY only with the labelled test-grade FNV verifier; a "good update passes" result with that verifier wired proves the GATE MECHANICS, not bundle security |
| No update-bundle producer | no tool builds a bundle; no schema defines bundle content (including the configuration-vs-binary question); nothing has ever been signed (P9.34 §8 finding 1) | matrix case U-1 has NO executable producer today; the bundle for the test must be hand-constructed in the lab — recorded as such, never presented as a release artifact |
| No health-check → rollback wiring | nothing invokes `rollback` on a failed post-update health check (P9.34 §8 finding 4 — a deployment-composition contract) | matrix case U-7 (failed-update mid-apply) tests the DECISION RULES via the register; the automatic trigger does not exist and is not claimed |

### 3.3 The upgrade ordering rule (binding, mirrored from the seam)

Verification precedes staging precedes activation. Any path that writes a
bundle to disk before a verification verdict exists is a FINDING
(`VERIFY_AFTER_WRITE` — the ledger P9.34 step 1's named risk, structurally
refused by the seam because `verify_before_staging` takes no path). The RC
test asserts the property behaviourally (empty scratch directory after every
refusal case) and structurally (no `Path` parameter, no `std::fs` call in
the verification path).

## 4. The rollback register semantics (ledger step 4) — cited, never re-decided

The rollback rules ARE the register's replay semantics (§2). Restated as the
RC test's oracle, in order:

1. **The target must be a version this register has SEEN activated**
   (`UPDATE_ROLLBACK_TARGET_NEVER_ACTIVATED`) — a rollback re-activates a
   PREVIOUSLY-VERIFIED version, never an unknown one.
2. **The target must be BEHIND the active version**
   (`UPDATE_ROLLBACK_TARGET_NOT_BEHIND_ACTIVE`) — a forward "rollback" is an
   upgrade wearing a rollback's clothes and is refused. Versions are
   compared NUMERICALLY (`1.2.3 < 1.10.0`; malformed cores are
   `UPDATE_VERSION_MALFORMED`), never lexically.
3. **Below-floor override discipline (the never-silent downgrade):** a
   rollback to a version BELOW the deployment's minimum-version floor is
   refused (`UPDATE_ROLLBACK_BELOW_FLOOR_REFUSED`) UNLESS an explicit
   `RollbackOverride` is presented — named actor + named authority
   reference, BOTH mandatory (an override without them is
   `UPDATE_ROLLBACK_OVERRIDE_INVALID`) — and even then the override rides
   BOTH the register record (`override_reference`) AND the audit line. A
   below-floor rollback that is silent is a FINDING by definition.
4. **The reason is mandatory** (`InvalidUpdateInput` on an empty reason) —
   a rollback is never silent.
5. **Central-plane rollback (ledger step 4, non-edge half):** a rollback to
   the previous version returns the system to a CONSISTENT state — data,
   schema-compatibility window (per P10.13), plugin versions — and the
   rollback artifacts are RETAINED AND IDENTIFIED (§9.3: pin plugin
   dependencies and retain rollback artifacts). Pending `outbox` entries and
   inbox/idempotency records survive the upgrade AND the rollback without
   loss or duplicate publication (§15.5; ledger step 5): entries pending
   before the upgrade are published exactly once after it; entries already
   delivered are not re-published. What a rollback does NOT do: it does not
   un-publish events already published, and an irreversible migration (per
   P10.13's list) is not reversed by rollback — the path then is
   restore-from-backup (P10.11), never an improvised down-migration.

## 5. The test matrix (ledger steps 1–4) — WRITTEN, ALL EXECUTION CELLS EMPTY

Edge cases drive the cited seam; central cases drive the staging composition.
Every row's outcome cell is EMPTY; nothing has run.

| # | Case | Drives | Expected verdict (fixed pre-run, from the cited semantics) |
|---|---|---|---|
| U-1 | Good update: valid manifest, trusted signer, version ≥ floor, digest matches | edge | `EDGE_UPDATE_VERIFIED` → activation record appended → register reports the new version; audit carries the approval reference |
| U-2 | Bad signature: bundle bytes ≠ manifest digest | edge | refused `UPDATE_DIGEST_MISMATCH` BEFORE any write; zero files, zero new register records |
| U-3 | Digest mismatch, trusted signer: correct signer, wrong digest field | edge | refused `UPDATE_DIGEST_MISMATCH` (the digest verdict is independent of trust); zero writes |
| U-4 | Untrusted signer | edge | refused `UPDATE_UNTRUSTED_SIGNER` BEFORE the digest check (the recorded refusal order); the signer-reference VALUE never echoed in any refusal |
| U-5 | Unsigned bundle (no manifest) | edge | refused `UPDATE_UNSIGNED_BUNDLE` FIRST, before any other check |
| U-6 | Below-floor manifest version at install | edge | refused `UPDATE_BELOW_MINIMUM_FLOOR` |
| U-7 | Failed-update mid-apply → rollback, edge still operational | edge | the register's decision rules: rollback to the previously-activated version with the recorded reason; `EDGE_UPDATE_ROLLBACK` audited; NOTE — the automatic health-trigger does not exist (§3.2 gap table), so the trigger is operator-invoked in this test and recorded as such |
| U-8 | Rollback target never activated | edge | refused `UPDATE_ROLLBACK_TARGET_NEVER_ACTIVATED` |
| U-9 | Forward "rollback" (target ≥ active) | edge | refused `UPDATE_ROLLBACK_TARGET_NOT_BEHIND_ACTIVE` |
| U-10 | Below-floor rollback WITHOUT override | edge | refused `UPDATE_ROLLBACK_BELOW_FLOOR_REFUSED` |
| U-11 | Below-floor rollback WITH a valid override | edge | proceeds; the override string rides the register record AND the audit line (never silent); an override missing actor or authority is `UPDATE_ROLLBACK_OVERRIDE_INVALID` |
| U-12 | Rolling upgrade of the control plane to the candidate | central | health/readiness probes pass per component; no in-flight dispatch transaction corrupted; migrations within their P10.13 windows; version identifiers report the candidate |
| U-13 | Post-upgrade smoke: shadow dispatch-lifecycle pass + contract compatibility | central | the §14.1 vocabulary round-trips in shadow; P10.02's frozen contracts unchanged; command/audit continuity holds (chain head verifies) |
| U-14 | Central rollback to the previous version | central | consistent state restored (data, schema window, plugin versions); rollback artifacts retained and identified; outbox/inbox survived with NO loss and NO duplicate publication (entries pending before the upgrade published exactly once after; delivered entries not re-published) |
| U-15 | Plugin upgrade during an active dispatch transaction | central | refused or deferred until drain (§5.6); the version change recorded after the transaction closes |

Matrix notes: U-1..U-11 require a constructed bundle + the labelled
test-grade verifier (§3.2 gap table) — their results prove GATE MECHANICS;
U-12..U-15 require a staging environment and TWO buildable versions — the
source version does not exist (§3.1), so the central rows are the least
executable of the matrix and are recorded as BLOCKED_SOURCE_BUILD until a
prior build exists. No case passes by default: a case that cannot execute is
recorded with its blocker, never as a pass and never as silently dropped.

## 6. Timings and manual interventions (ledger step 6)

Timings (per-step upgrade/rollback durations, drain times, probe wait times)
are recorded from the run's clocks only — `<EMPTY>`; no timing is estimated
or carried from any other subtask. Every manual intervention required by the
run is recorded verbatim (step, operator, justification) — an upgrade that
needs undocumented manual steps is a deployment-model finding carried to
P10.24, not a tolerated detail.

## 7. The run record — ALL CELLS EMPTY (UNRUN)

**Run metadata (all EMPTY):** dates / executor + witness / staging-environment
ID + its pilot-mirroring statement / source version artifact ID /
**release-candidate identifier (must equal the P10.18 candidate)** / plugin
versions before and after / migration set applied / register file hash
before and after.

| # | Case | Result | Measured timing | Verification evidence | Pass/fail | Disposition |
|---|---|---|---|---|---|---|
| U-1 | (EMPTY — UNRUN) | | | | | |
| U-2 | (EMPTY — UNRUN) | | | | | |
| U-3 | (EMPTY — UNRUN) | | | | | |
| U-4 | (EMPTY — UNRUN) | | | | | |
| U-5 | (EMPTY — UNRUN) | | | | | |
| U-6 | (EMPTY — UNRUN) | | | | | |
| U-7 | (EMPTY — UNRUN) | | | | | |
| U-8 | (EMPTY — UNRUN) | | | | | |
| U-9 | (EMPTY — UNRUN) | | | | | |
| U-10 | (EMPTY — UNRUN) | | | | | |
| U-11 | (EMPTY — UNRUN) | | | | | |
| U-12 | (EMPTY — UNRUN) | | | | | |
| U-13 | (EMPTY — UNRUN) | | | | | |
| U-14 | (EMPTY — UNRUN) | | | | | |
| U-15 | (EMPTY — UNRUN) | | | | | |

## 8. Reviewer-verification procedure (human)

1. Confirm §1's plain statement stands verbatim in the record and that the
   P9.34 `UPDATE_STATEMENT` in `crates/edge-agent/src/update_verification.rs`
   agrees with it (no update ever built/applied; no production-grade
   signature scheme).
2. Confirm EVERY cell of §7 is EMPTY/UNRUN and §6's timing rule forbids
   estimated values.
3. Trace §2's citations into the source (`grep -n` the symbols):
   `verify_before_staging`, `verify_inner`, `UPDATE_UNSIGNED_BUNDLE`,
   `UPDATE_UNTRUSTED_SIGNER`, `UPDATE_DIGEST_ALGORITHM_MISMATCH`,
   `UPDATE_BELOW_MINIMUM_FLOOR`, `UPDATE_DIGEST_MISMATCH`,
   `UPDATE_VERIFIER_REFUSED`, `FileUpdateLedger` and its magic
   `FLEXCORE-EDGE-UPDATE-REGISTER`, `RollbackOverride`,
   `UPDATE_ROLLBACK_BELOW_FLOOR_REFUSED`, `compare_versions`, and the
   `UPDATE_STATEMENT` / `UPDATE_SLOT_DECISION` constants.
4. Confirm §5's expected-verdict column derives from the cited semantics
   (refusal order for U-2..U-6; rollback rules for U-8..U-11) and that the
   §3.2 gap table (no crypto, no producer, no health wiring) is carried
   UNCHANGED from P9.34 §7/§8 — this record adds no capability claim.
5. Confirm §4's override discipline states the override rides BOTH the
   register and the audit line, and that the outbox/inbox no-loss/no-duplicate
   criterion is stated for BOTH the upgrade and the rollback.
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the matrix is written; nothing was upgraded, rolled back,
   built, or applied.

## 9. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) the WRITTEN-UNRUN test suites that pin the cited seam (human
#     build/test gate; nothing was built or run in this loop):
cargo test -p flexcore-edge-agent --test update_verification_p9_34
cargo test -p flexcore-edge-agent lib::update_verification

# (c) Build/obtain TWO artifacts: the source version (previous release or
#     pilot build) and the P10.18 release candidate; record both
#     identifiers. Without a source, the central rows (U-12..U-15) stay
#     BLOCKED_SOURCE_BUILD.
# (d) Provision the staging environment mirroring the pilot deployment
#     (per P10.24's model once it exists); record the mirroring statement.
# (e) Run the edge matrix (U-1..U-11) through the cited seam — verify-
#     before-stage, activation, rollback — with the constructed bundle and
#     the labelled test-grade verifier; fill §7 rows U-1..U-11 FROM THE RUN
#     ONLY (register history, audit lines, scratch-directory state).
# (f) Run the central matrix (U-12..U-15): rolling upgrade → smoke pass →
#     rollback → outbox/inbox check; fill §7 rows U-12..U-15 with timings
#     from the run's clocks and record every manual intervention verbatim.
# (g) Record dispositions; file the record as the P10.12 evidence named in
#     CHECKLIST.md; hand the result to P10.24 (deployment/upgrade model)
#     and P10.27 (go/no-go).
```

## 10. Findings (recorded, not fixed)

- **F-P10.12-1 (carried, unchanged — P9.34 §8 finding 1):** no update-bundle
  producer exists anywhere; the P9.34 surface verifies a manifest/bundle
  contract that has no upstream producer. This test matrix is therefore
  runnable only against hand-constructed lab bundles until the producer
  exists — recorded as the largest standing gap, unchanged by this wave.
- **F-P10.12-2 (carried, unchanged — P9.34 §7/§8):** no production-grade
  signature scheme is implemented; the default verifier refuses everything
  and the only other verifier is test-grade FNV-1a-64 (tamper-EVIDENCE, not
  tamper-PROOF). U-1's "pass" with that verifier wired proves gate
  mechanics only; the SHA-256-or-stronger wiring under reviewed key
  management remains a DEPLOYMENT REQUIREMENT.
- **F-P10.12-3 (carried, unchanged — P9.34 §8 finding 4):** the mid-apply
  failure path has no edge-side hook (no health-check → rollback
  composition); U-7 is operator-invoked by construction. The composition
  contract is a recorded Phase 7 follow-up, not an edge-module change.
- **F-P10.12-4 (dependency gaps, honest):** the upgrade SOURCE (a prior
  release or pilot build) does not exist; P10.13 (migration windows) and
  P10.24 (deployment model) are concurrent siblings whose records do not
  exist yet (cited by expected paths); the staging environment does not
  exist. The central matrix rows are the least executable and are labelled
  BLOCKED_SOURCE_BUILD rather than silently dropped.
- **F-P10.12-5 (observation, no action taken):** the seam's audit events use
  `EdgeAuditKind::Startup` for update verdicts (the closed
  `EdgeAuditKind` vocabulary has no dedicated update kinds — P9.34 §8
  finding 3). The refusal-tag prefix (`EDGE_UPDATE_*`) still makes the
  verdicts distinguishable in audit text; extending the closed vocabulary
  remains a reviewed change outside this subtask's ownership.

## 11. Non-claims

No update was built, signed, staged, or applied; no signature scheme exists
at production grade; no rollback was performed anywhere; no register record
exists from any run; no timing was measured. The matrix's expected-verdict
column is the cited semantics, not an observed outcome. Nothing here
establishes secure boot (the P6.34 position stands: every hardware
determination is PENDING PILOT HARDWARE SPEC), certification, or
field-update readiness; the entire evidence class is simulation/lab-grade
(AGENTS.md rules 2–5; ENTRY_NOTE boundary 1).

Status: **WRITTEN-UNRUN** — an upgrade/rollback test cannot "run" by being
written; the source build, the candidate, the staging environment, the
bundles, and the executions are the human gates and have not happened and
are not scheduled.
