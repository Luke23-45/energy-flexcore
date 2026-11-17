# Phase 13 wave 1, Agent A — close-out (P13.01–P13.08, identity core)

| Field | Value |
|---|---|
| Scope | Agent A's wave-1 range: P13.01–P13.08 (identity core), records 01–08, and this close-out (19) |
| Status | ALL WRITTEN-UNRUN: no IdP deployed, no OIDC token validated, no SCIM object provisioned, no MFA enrollment, no API key issued, no webhook secret created, no approval workflow executed. Every operational evidence cell EMPTY/UNRUN. The first human build of the Phase 0–10 workspace and the P13.20 lab gate remain the human boundaries. |
| Date | 2026-11-14 |

## 1. Subtask → record → files

| Subtask | Record | Artifacts |
|---|---|---|
| P13.01 identity architecture (P13-E-1 registered OPEN) | `records/01_p13_01_identity_architecture.md` | design record (OIDC-first; SAML via broker; per-tenant SSO connections as tenant data) |
| P13.02 OIDC token validation | `records/02_p13_02_oidc_token_validation.md` | `apps/control-plane/src/oidc.rs` (NEW); `apps/control-plane/src/lib.rs` (additive module line); `apps/control-plane/Cargo.toml` (additive `jsonwebtoken = "11.1.0"` pin); `apps/control-plane/src/auth.rs` (additive `site-approver` arm of `parse_role`) |
| P13.03 SCIM 2.0 provisioning | `records/03_p13_03_scim_provisioning.md` | design record (lifecycle; group→role mapping; deprovisioning SLA + audit evidence) |
| P13.04 MFA/step-up (P13-E-3 registered OPEN) | `records/04_p13_04_step_up_mfa.md` | `apps/control-plane/src/stepup.rs` (NEW); `apps/control-plane/src/lib.rs` (additive module line); `apps/control-plane/src/state.rs` (additive optional `step_up` field + `with_step_up` builder, fail-closed default) |
| P13.05 role model refinement | `records/05_p13_05_role_model.md` | `crates/flexcore-authz/src/policy.rs` (additive `Role::SiteApprover` + `id()` arm); `crates/flexcore-authz/src/action.rs` (additive `apikey.*` variants + arms + negative tests); `crates/flexcore-authz/src/phase13_policies.rs` (NEW); `crates/flexcore-authz/src/lib.rs` (additive module line) |
| P13.06 multi-step approval engine | `records/06_p13_06_approval_workflow_engine.md` | `crates/flexcore-application/src/command_gates.rs` (ADDITIVE section appended after the existing tests: engine + types + its own test module) |
| P13.07 machine identity | `records/07_p13_07_machine_identity.md` | design record (client-credentials JWT; scope subsets; revocation) |
| P13.08 API keys + webhook secret FORMAT | `records/08_p13_08_api_keys_webhook_secret_format.md` | design record; §4 defines the webhook secret FORMAT ONCE (`whsec_<43 unpadded base64url chars>`, 32 bytes) — **Agent B's P13.09 consumes it by citation** |

## 2. The byte-identical invariants (verified, evidence quoted)

1. **The frozen 14-state / 27-edge transition set** —
   `crates/flexcore-domain/src/command_state.rs` was NOT edited; `git
   diff --numstat crates/flexcore-domain/src/command_state.rs
   crates/flexcore-domain/src/tenant_context.rs` reports **0 added / 0
   removed for both files**.
2. **The P4.13 gate file's pre-existing content** —
   `crates/flexcore-application/src/command_gates.rs`: `git diff
   --numstat` reports **1277 added / 0 removed** (a pure append anchored
   at line 2045, the original 2048-line file's tail; the grep for
   removed non-marker lines returns **0**). The never-granted guard
   tests in the pre-existing content are untouched.
3. **The never-granted permission set** — quoted verbatim, unchanged:
   > The never-granted permission set (`dispatch.authorize`,
   > `dispatch.issue`, `secret.read`, `network.any`) stays byte-identical

   None of the four names was added to the `Action` surface; the
   `action.rs` and `phase13_policies.rs` negative tests assert all four
   still parse to `None` (structural `UnknownAction` deny for every
   principal class — human, service account, API key, machine identity).
4. **`crates/flexcore-domain/src/tenant_context.rs`** — byte-identical
   (0/0 diff; the P1.06 single-construction-path contract is consumed,
   never edited).

## 3. Permitted-tool verification (run 2026-11-14; outputs verbatim)

```
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0

$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 20 workspace members.
EXIT=0
```

(The `ù` is the checker's own output byte as printed in this shell —
quoted verbatim per the honesty rule; the check is green, exit 0.)

## 4. E-decisions (both stay OPEN — agents never close them)

| ID | Registered in | Status | DEFAULT implemented |
|---|---|---|---|
| P13-E-1 (IdP) | record 01 §1 | OPEN, closer = human owner | OIDC-first + Keycloak reference (records 02 and the P13.20 lab spec execute the DEFAULT, dependence recorded per conventions §4) |
| P13-E-3 (MFA mechanism) | record 04 §5 | OPEN, closer = human owner | IdP-delegated MFA primary via the `StepUpVerifier` port; shipped default verifier DENIES everything (fail closed — no step-up can succeed anywhere until a verifier is composed and signed off) |

## 5. Deviations and honest notes

1. **`main.rs` untouched** (deviation from the permitted "minimal marked
   edits"): no IdP exists, so no composition change is honest to make;
   `DenyAllResolver` remains the shipped default resolver and the
   production path is composed per deployment (records 02 §1, 04 §6).
2. **The ledger was NOT edited** (per instruction): no checkbox ticked;
   the conventions-disciplined tick process belongs to the coordinator,
   citing the records above.
3. **One new dependency** (`jsonwebtoken = "11.1.0"` in the
   control-plane manifest, a composition crate): version verified on
   crates.io on 2026-11-14 (11.1.0 published 2026-09-16; the 12.0.0
   published 2026-11-13 was deliberately not taken); the compile is an
   ASSUMPTION pending the first human build (record 02 §5). No other
   dependency was added; the JWKS fetch is an injected port (no HTTP
   client in the repo) and the approval engine, step-up service, and
   authz extensions are std-only.
4. **The `JwksCache` TTL default (15 min) and the leeway (60 s), the
   step-up TTL (5 min), the key-rotation overlap (24 h), the unused-key
   threshold (90 days), and the deprovisioning window (5 min) are
   REGISTERED RECOMMENDATIONS (X-8)** — pre-registered before any run
   that could tune them; none is a tested optimum.
5. **Agent B's P13.09 artifacts already exist in the tree**
   (`webhook_service.rs`, `webhook_signing.rs`, migration 0020). Record
   08 §4 is the single secret-format definition; B consumes it by
   citation (this record's §1 note stands as the handoff).
6. **No console file was edited** (wave-2 territory): `stepUpApproval.tsx`
   keeps its UI-only honesty record; P13.17 re-wires it onto the
   enforced protocol.
7. **Compile-correctness caveats recorded, never hidden**: the
   `jsonwebtoken` API-surface ASSUMPTION (record 02 §4); the
   exhaustive-`Role`-match audit (record 05 §7 item 3 — `Role::id()` is
   the only library-code exhaustive match found; the build verifies);
   the `api_live.rs` `AppState` literal uses functional-update syntax,
   so the additive `step_up` field breaks nothing.

## 6. Handoff notes

1. **Agent B**: cite record 08 §4 for the secret format; propose any
   `webhook.*` management actions through the same additive closed-enum
   rule (authz ownership stays with Agent A's crate, additive-only).
2. **Wave 2 (P13.15–P13.21)**: consume records 01–08 by citation; the
   session design (P13.16) hangs off the P13.02 resolver; the console
   designs (P13.17/P13.18) consume the enforced step-up protocol and the
   refined role model; P13.19's test suite extends the negative
   matrices records 04/05/06 seeded.
3. **P13.22 (close)**: the two byte-identical invariants (§2) are this
   wave's consistency-audit inputs; both E-decisions (§4) remain OPEN
   for the register.
4. **Human gates outstanding**: the first workspace build (compiles the
   pin, the new modules, the authz/action additions, and every written
   test file); the P13.20 lab run (first real token, first JWKS
   rotation, first SCIM push — all still UNRUN).
