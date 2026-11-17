# Phase 18 — Edge Fleet Lifecycle and OTA Updates — Ledger

Date: 2026-11-14. Status: **IMPLEMENTED — awaiting human build/test** — 16 of 16 subtasks ticked. Phase close: HANDOVER.md, CHECKLIST.md, records/43_phase_18_close_and_consistency_audit.md. No update produced, signed, staged, or applied; no device exists; fail-closed preserved by construction; the human-gate queue is in CHECKLIST.md.

**Gate.** No implementation of this phase starts before (a) human review of
this roadmap and (b) the first human build of the Phase 0–10 workspace is
green (program conventions §3; program README, hard status boundary 1).
Every run, scan, sign-off, threshold, and date produced during
implementation is a HUMAN GATE INPUT recorded UNRUN / EMPTY / UNSIGNED.

**Tick convention (carried verbatim from Phases 0–10).** A checkbox is
ticked only as "implemented, awaiting human build/test" with a
decision-record citation — `- [x] ... (decision record:
\`records/NN_slug.md\`)` — never as "tested", "verified", "passed", or
"works". Tick flips use the exact-match count==1 method or Write/Edit;
decision records are created at implementation time under `records/`.

## Subtask register

- [x] **P18.01 — OTA architecture decision record.** TUF-style metadata (decision record: `records/01_p18_01_ota_architecture_adr.md`)
      repository + SUIT-shaped signed-manifest architecture ADR; P18-E-1
      registered (TUF-full vs minimal-first — DEFAULT: minimal signed
      manifest with TUF-shaped role separation designed-in, full TUF
      repository above a recorded fleet threshold) [S-161][S-162].
      Evidence: the record cites the ADR and the open E-decision; no
      repository exists and none is claimed running.
- [x] **P18.02 — Update-bundle format specification.** Versioned, signed (decision record: `records/02_p18_02_update_bundle_format.md`)
      format extending the P9.34 `UpdateManifest`: applicability metadata
      (vendor / device class / device IDs), sequence-number anti-rollback
      beyond the v1 minimum-version floor, payload digests, size caps
      [S-162]. Evidence: the record cites the format spec; no bundle is
      produced and none is claimed.
- [x] **P18.03 — Metadata repository implementation plan.** Root offline (decision record: `records/03_p18_03_metadata_repository_plan.md`)
      key; targets with per-device-class delegations; snapshot; timestamp;
      N+1 root rotation [S-161]; expiry discipline; every key ceremony is a
      human gate (P10.21 interplay). Evidence: the record cites the role
      and key plan; no repository is stood up and no key exists.
- [x] **P18.04 — Device update flow specification.** download → verify → (decision record: `records/04_p18_04_device_update_flow.md`)
      install → status notifications incl. explicit failure states (OCPP
      2.0.1 L01 firmware-update flow as a checklist, gap analysis §7
      [S-121][S-122]); A/B slots per `UPDATE_SLOT_DECISION`; health-check
      via watchdog/boot counters; auto-rollback; anti-rollback counters.
      Evidence: the record cites the flow spec; no device update occurs.
- [x] **P18.05 — Edge-agent update module extension.** Real verifier behind (decision record: `records/05_p18_05_edge_agent_update_extension.md`)
      the P9.34 seam — fail-closed preserved when unconfigured
      (`NoUpdateVerifier` semantics stay); manifest validation, sequence
      checks, update-progress state machine. Evidence: the record cites the
      extension design and the preserved default; nothing is wired into a
      build here.
- [x] **P18.06 — Staged rollout orchestrator (control plane).** Cohorts (decision record: `records/06_p18_06_rollout_orchestrator_scope_distinction.md`)
      (canary 5–10% of a block per practice — a DEFAULT recommendation, not
      a sourced figure, pre-registered before any run), abort thresholds on
      failure rates, per-device attestation before authorization,
      fleet-state visualization data; explicit scope-distinction record vs
      the kernel plugin-rollout policy (which stays cohort/percentage-free
      BY CONSTRUCTION). Evidence: the record cites the design and the scope
      distinction; no rollout runs.
- [x] **P18.07 — Update evidence + audit pack.** Per-device update records (decision record: `records/07_p18_07_update_evidence_audit_pack.md`)
      into `flexcore-audit` (attempt / verify / install / result /
      rollback), digest-chained, exportable. Evidence: the record cites the
      record shapes and chain linkage (the P11.15 production chain is
      consumed, LR-203 honored); no update record exists.
- [x] **P18.08 — Device identity lifecycle formalization.** IDevID/LDevID (decision record: `records/21_p18_08_identity_lifecycle.md`)
      model mapped onto the P9.33 ceremony; credential rotation schedule;
      revocation propagation (kernel revocation interplay) [S-165][S-166].
      Evidence: the record cites the lifecycle mapping; no certificate is
      issued or revoked.
- [x] **P18.09 — Zero-touch onboarding design.** FDO-pattern late binding: (decision record: `records/22_p18_09_zero_touch_onboarding.md`)
      ownership-voucher analog, rendezvous service seam, claim flow for
      factory-provisioned devices; the manual provisioning path (P9.33)
      stays the default and the fallback [S-165][S-166]. Evidence: the
      record cites the design; no device is claimed or onboarded.
- [x] **P18.10 — Decommissioning runbook.** Revoke operational (decision record: `records/23_p18_10_decommissioning_runbook.md`)
      certificates; release the ownership voucher; deregister from the
      fleet registry and protocol trust chains (P14 interplay); data-wipe
      verification; second-life note. Evidence: the record cites the
      runbook; no device is decommissioned.
- [x] **P18.11 — eSIM/cellular lifecycle tracking.** Data model only: (decision record: `records/24_p18_11_esim_lifecycle.md`)
      profile state, carrier-agnostic fields; carrier specifics out of
      scope [S-167]. Evidence: the record cites the data model; no cellular
      profile is managed.
- [x] **P18.12 — Fleet device-registry hardening.** 0018 registry (decision record: `records/25_p18_12_fleet_registry_hardening.md`)
      extension: firmware state, cohort membership, attestation state,
      last-seen health; migration spec additive, expand/contract per
      P11.18. Evidence: the record cites the schema delta and migration
      spec; no migration runs.
- [x] **P18.13 — Attestation design.** Boot-time evidence, measurement (decision record: `records/26_p18_13_attestation_design.md`)
      reporting, verification policy (what the platform checks before
      authorizing an update); hardware-absent fallback honestly specified
      (P18-E-2). Evidence: the record cites the policy and the with/without
      difference; no TPM/secure element is claimed or measured.
- [x] **P18.14 — Update test suite (written-unrun).** Mock device + manifest (decision record: `records/41_p18_14_update_test_suite_mock_device.md`)
      validation + rollback scenarios + anti-rollback negative tests +
      interrupted-update (power-loss) simulation + concurrent-command
      blocking tests. Evidence: the record cites the test files; execution
      is a human gate and no test has run.
- [x] **P18.15 — Update-safety interplay with command lifecycle.** Updates (decision record: `records/28_p18_15_update_command_interlock.md`)
      BLOCKED while commands are ISSUED/TRACKING on the device;
      drain-before-update (report §5.6 discipline); post-update conformance
      re-check (edge conformance module interplay). Evidence: the record
      cites the blocking rule and drain procedure; no command and no update
      interact on hardware.
- [x] **P18.16 — Decision records + close.** P18-E-1 (TUF-full vs minimal), (decision record: `records/43_phase_18_close_and_consistency_audit.md`)
      P18-E-2 (hardware trust anchors assumption set), P18-E-3 (rollout
      cohort policy defaults); consistency audit across every phase-18
      artifact. Evidence: the records compile options, defaults, and
      closing evidence; agents close none.

## Decision points (all OPEN — human-owned)

| ID | Question | DEFAULT (label) | Closer |
|---|---|---|---|
| P18-E-1 | TUF-full repository vs minimal-signed-manifest first | Minimal signed manifest with TUF-shaped role separation designed-in; full TUF repository when the fleet exceeds a recorded threshold | human owner |
| P18-E-2 | Hardware trust anchors (TPM/secure element) assumption set | Hardware-optional: the platform works with and without; the difference stated honestly per the P6.34 posture (PENDING PILOT HARDWARE SPEC) | human owner |
| P18-E-3 | Rollout cohort policy defaults (canary size, block progression, abort thresholds) | Canary 5–10% → blocks; abort thresholds pre-registered before any run (X-8); all values configuration, never code | human owner |
