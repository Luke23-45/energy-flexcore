# Phase 13 wave 2, Agent A — close-out (P13.15–P13.21, console + ops)

| Field | Value |
|---|---|
| Scope | Agent A's wave-2 range: P13.15–P13.21, records 41–47, and this close-out (48). Agent B owns P13.22 and the HANDOVER/CHECKLIST/close records (61–69) — untouched. |
| Status | ALL WRITTEN-UNRUN: no tenant created/exported/deleted, no session created/revoked, **no login performed**, no step-up challenge issued from the console, no IdP image pulled, no realm imported, no notification sent, no management endpoint called, no test executed. Every operational evidence cell EMPTY/UNRUN. The first human build/test of every artifact and the P13.20 lab run remain the human boundaries. |
| Date | 2026-11-14 |

## 1. Subtask → record → files

| Subtask | Record | Artifacts |
|---|---|---|
| P13.15 tenant lifecycle | `records/41_p13_15_tenant_lifecycle.md` | design record (onboarding checklist O-1..O-10, offboarding D-1..D-9 with revocation-first order, export+reconciliation under RLS [S-70], the specified-not-implemented register) — no tenant operated on |
| P13.16 session management | `records/42_p13_16_session_management.md` | `apps/control-plane/src/session.rs` (NEW: SessionRecord/SessionDirectory/InMemorySessionDirectory/SessionEnforcingResolver + in-module tests), `apps/control-plane/src/lib.rs` (additive module line, marked) |
| P13.17 console auth hardening | `records/43_p13_17_console_auth_hardening.md` | `console/src/auth/session.ts`, `console/src/auth/oidcLogin.ts`, `console/src/auth/stepUpFlow.ts` (NEW); `console/src/components/sessionBanner.tsx` (NEW); their four test files; `console/src/App.tsx` (ADDITIVE marked edit: one import + one `<SessionBanner />` render) |
| P13.18 admin workspace completion | `records/44_p13_18_admin_workspace_completion.md` | `console/src/workspaces/adminManagement.ts`, `AdminManagementSection.tsx` (NEW); two test files; `console/src/App.tsx` (ADDITIVE marked edit: `AdministrationWithManagement` wrapper + route entry — the P5.07 workspace component UNCHANGED) |
| P13.19 identity test suite | `records/45_p13_19_identity_test_suite.md` | `crates/flexcore-authz/tests/phase13_authz_matrix_tests.rs`, `apps/control-plane/tests/phase13_stepup_negative_tests.rs`, `phase13_session_resolver_tests.rs`, `phase13_webhook_delivery_tests.rs`, `phase13_scim_role_mapping_tests.rs` (all NEW, all WRITTEN-UNRUN; the console tests landed with records 43/44) |
| P13.20 IdP lab spec | `records/46_p13_20_idp_lab_spec.md` | `deploy/compose/idp-lab/docker-compose.idp-lab.yml` (NEW draft, image `quay.io/keycloak/keycloak:26.8.1` pinned, version verified against keycloak.org 2026-11-14 — no pull), `deploy/compose/idp-lab/realm-flexcore-lab.json` (NEW draft, JSON syntax validated, content UNVERIFIED against a live importer); no existing compose file edited |
| P13.21 notification/audit interplay | `records/47_p13_21_notification_audit_interplay.md` | `apps/control-plane/src/notification_vocab.rs` (NEW: the versioned 26-row disposition registry + agreement invariants + tests), `apps/control-plane/src/lib.rs` (additive module line, marked) |

## 2. The byte-identical invariants (verified this close, evidence quoted)

1. **The frozen 14-state / 27-edge command lifecycle**
   (`crates/flexcore-domain/src/command_state.rs`), **the frozen 26-event
   vocabulary** (`crates/flexcore-events/src/events.rs`), and **the
   single-construction-path claims contract**
   (`crates/flexcore-domain/src/tenant_context.rs`):
   `git diff --numstat HEAD -- <the three files>` reports **no lines —
   all three byte-identical to HEAD**. The P13.21 registry IMPORTS
   `FROZEN_EVENT_NAMES` (never re-lists it) and a test pins the length
   equality, so the module cannot silently diverge.
2. **The never-granted permission set** — quoted verbatim, unchanged
   (conventions §6.4):
   > The never-granted permission set (`dispatch.authorize`,
   > `dispatch.issue`, `secret.read`, `network.any`) stays byte-identical

   No source file of the authz enums or policy sets was opened for edit
   this wave; the new test files ASSERT the set's structural
   unreachability (parse → `None`; `evaluate_action_id` denies for all
   five roles). `apps/control-plane/src/auth.rs`, `oidc.rs`, `stepup.rs`,
   `state.rs`, `main.rs`, `crates/flexcore-authz/src/*`,
   `apps/control-plane/src/notifications.rs` were all consumed by
   citation and NOT edited by this wave (their wave-1 state stands).
3. **`apps/control-plane/src/notifications.rs` (record 23)** — not
   edited: the P13.21 registry consumes its three public items by typed
   import and pins the row-for-row agreement; the map stays byte-identical
   (B's handoff note honored: rows, never logic).

## 3. Permitted verification tools — VERBATIM OUTPUTS (run 2026-11-14, close-out)

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0
```

```text
$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0
```

```text
$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 20 workspace members.
EXIT=0
```

(The `ù` is the checker's own output byte as printed in this shell —
quoted verbatim per the honesty rule; the check is green, exit 0.
20 workspace members = no crate was added by this wave; all Rust code is
modules in, or test files of, existing members.)

## 4. E-decisions (all stay OPEN — agents never close them)

| ID | Status | What this wave recorded |
|---|---|---|
| P13-E-1 (IdP) | OPEN, closer = human owner | the lab spec executes the DEFAULT (Keycloak) with the dependence recorded (record 46 §3) |
| P13-E-2 (channel providers) | OPEN | untouched; P13.21's registry rows carry EMAIL-only Notify channels, consistent with the email-first DEFAULT |
| P13-E-3 (MFA mechanism) | OPEN | the lab realm's OTP policy is the fallback's lab shape; the DEFAULT (IdP-delegated) is what the lab verifier composition implements (record 46 §3) |

## 5. Deviations and honest notes

1. **`state.rs` / `main.rs` untouched** (deviation from "composition
   roots: minimal marked edits" — App.tsx and lib.rs WERE the composition
   edits): no IdP exists and no session-aware middleware exists, so no
   composition change is honest to make on the Rust side; the session
   resolver's bare-trait path fails closed on a wrong composition by
   design (record 42 §5).
2. **`stepUpApproval.tsx` and `Administration.tsx` NOT edited** (the
   P7.25 additive precedent): P13.17's enforced-protocol flow coexists
   with the UI-only panel, each carrying its own honesty note, and the
   P13.18 section composes BELOW the unchanged P5.07 workspace; both
   structural facts are pinned by tests.
3. **New console directory `console/src/auth/`** — additive, no new
   package (console work uses the existing package.json set only; the
   gap register: the login flow needs no package because the console
   never performs the OIDC exchange).
4. **The Keycloak image pin (26.8.1)** was verified against keycloak.org
   on 2026-11-14 (a version-existence verification only — the pull and
   every runtime behavior stay UNRUN); the realm draft's field spellings
   and the exact health-port convention are recorded ASSUMPTIONS
   verified at the first run (record 46 §6).
5. **No ledger tick was applied** (per instruction): P13.15–P13.21 remain
   unticked; the conventions-disciplined tick process belongs to the
   coordinator, citing records 41–47.
6. **The `deploy/compose/idp-lab/secrets/` file does not exist** (the
   compose draft references a gitignored path whose creation is the
   human step R-1) — no secret material was created.
7. **Compile-correctness caveats recorded, never hidden**: the five Rust
   test files and the two new Rust modules are compile-ASSUMPTIONS
   verified at the first human build (record 45 §5); the four console
   test files run at the first console install (`npm test`).

## 6. Handoff notes

1. **Agent B (P13.22 close)**: consume records 41–47 + this record; the
   three byte-identical invariants (§2) are the consistency-audit
   inputs; the E-decisions (§4) stay OPEN for the register; the
   specified-not-implemented register spans records 41 §5, 43 §4, and
   44 §3 (the same endpoints named consistently — audit that, don't
   re-derive it).
2. **Human gates outstanding**: the first workspace build (compiles
   `session.rs`, `notification_vocab.rs`, and all five new Rust test
   files); the first console install (runs the six new console test
   files); the P13.20 lab runbook R-1..R-8 (first real token, first
   session, first SCIM push — all UNRUN); the record 45 §3 SCIM
   scenario cells remain EMPTY for the lab to fill.
3. **Future waves**: extend the P13.19 test files and the P13.21
   registry additively; do not fork per-phase copies (the P1.23
   precedent this batch followed).
