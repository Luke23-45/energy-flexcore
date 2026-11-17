# ADR-010 Extension 01 — Protocol adapters are supervisory transports and hold no dispatch authority

| Field | Value |
|---|---|
| Kind | **Extension record** — additive companion to [ADR-010](ADR-010.md). ADR-010 itself is UNTOUCHED (byte-identical). |
| Status | **Proposed** — inherits ADR-010's status discipline; it decides nothing ADR-010 left open and closes nothing ADR-010 reserved. |
| Date | 2026-11-14 |
| Decided by | Phase 14 (P14.16), recorded per the P14.16 subtask and the Phase 11–20 conventions §6.3 ("ADR-010 holds and extends") |
| Extends | ADR-010 (ML/RL outputs are proposals only and cannot directly dispatch) |
| Phase trace | `docs/phases-11-20/phase-14-protocol-device-connectivity/records/64_p14_16_adapter_permission_model.md` (P14.16); wave-1 architecture record `01_p14_01_adapter_architecture.md` (P14.01 §3) |
| Code trace | `crates/flexcore-protocol-ports/src/manifest.rs` (`AuthorityDeclaration` — one variant); `crates/flexcore-kernel/src/adapter_permissions.rs` (kernel-side enforcement); `crates/flexcore-kernel/src/manifest.rs` (`NEVER_GRANTED_PERMISSIONS` — byte-identical, reviewed untouched) |
| Open decisions | None touched. D1–D6 (report §25.2) remain OPEN exactly as ADR-010 records; P14-E-1/E-2/E-3 remain OPEN in phase-14 plan §4 |

## Why an extension record and not an ADR-010 edit

ADR-010 is in **Proposed** status pending the §23 Stage 0 exit-gate review
(P0.29) and its evidence chain is a reviewed Phase 0 baseline. Its own
revisit conditions reserve authority changes for "a new ADR superseding
this one"; this extension grants no authority and changes no ADR-010
decision, so superseding is neither warranted nor honest. Editing a
Proposed ADR mid-program would also blur the provenance of the Phase 0
review chain. The additive companion-record form keeps both histories
exact: ADR-010 governs ML/RL components as written; this record extends
the SAME decision to the NEW subject class Phase 14 introduced (protocol
adapters), with the Phase 14 records and code as evidence.

## Decision

**Protocol adapters are supervisory transports. They hold no dispatch
authority.** Concretely:

1. **External control objects enter the command lifecycle at PROPOSED.**
   A 2030.5 DERControl, an OpenADR event, an IEEE 1547 settings change,
   or any other external control object is a *proposal*, never an issued
   setpoint. It passes the frozen lifecycle's validation, feasibility,
   and authorization gates — the same three distinct gates as every other
   candidate — before any device surface sees it. An adapter that
   constructs an APPROVED or ISSUED state from external input is a defect,
   not a shortcut.
2. **Relay is not authority.** An adapter that declares the
   `CommandRelay` capability may carry an already-APPROVED command to a
   device and report the transport outcome — nothing more. Authorization,
   issuance, mode selection, and command sequencing stay in the platform.
3. **The type surface cannot express dispatch authority.**
   `flexcore_protocol_ports::manifest::AuthorityDeclaration` has exactly
   one variant, `SupervisoryRelayOnly`; there is no dispatch variant to
   remove, and the kernel-side check
   (`flexcore_kernel::adapter_permissions::AdapterBinding::
   check_command_relay_only`) re-matches the one-variant enum explicitly —
   a tripwire that refuses at the kernel boundary if a future variant is
   ever added against this decision.
4. **The never-granted permission set is untouched.** `dispatch.authorize`,
   `dispatch.issue`, `secret.read`, `network.any` stay byte-identical
   (conventions §6.4). Adapters request no §5.7 permission ids at all:
   adapter authority is a TYPE (`AuthorityDeclaration`), not a permission
   string, and the adapter capability vocabulary is disjoint from the §5.7
   permission vocabulary (pinned by test in
   `adapter_permissions.rs`).
5. **Settings and DOEs ride the same rails.** An IEEE 1547-2018 settings
   change is a lifecycle command with the approval gate, never a direct
   adapter write; a DOE envelope is optimizer constraint data only. Neither
   creates an adapter-side authority path.

## Relation to ADR-010's own text

ADR-010 decides that ML/RL outputs enter only as proposals and "cannot
call device protocols directly". This extension is the converse surface of
the same principle: the protocols themselves — the adapters that speak
them — are transports whose outputs (relayed commands, ingested external
events) enter the platform only through the same protected proposal path.
Neither the adapter nor the ML component authorizes; the protected
validation/feasibility/authorization path (report §14) remains mandatory
for both. The "Constrains later phases" consequences of ADR-010 extend
verbatim: a protocol adapter manifest, like a research profile, carries no
dispatch authority, is not enabled for field control in the 0.x line
(simulation/shadow ceiling, conventions §6.7), and every adapter-produced
record carries its source, protocol id, and manifest identity into the
command record for audit.

## Consequences

**Positive.** One authority model for every non-platform input source
(ML/RL, research plugins, protocol adapters): proposals in, gates in the
middle, relay out. The adapter tier can grow (eight protocol adapters
proposed in Phase 14) without ever touching the authority path; the kernel
tripwire makes an authority-variant addition mechanically visible.

**Negative.** DR-program and utility surfaces (2030.5, OpenADR, 1547
settings) cannot act natively even when their payload shape looks like a
direct command; every external action pays the three-gate cost. Adapters
that legitimately need fast local loops (none in the Phase 14 scope) are
out of scope here and would need the post-1.0 bounded-automation
qualification chain, per deployment, site, and asset class.

**Constrains later phases.** Wave-3 simulator counterparts (P14.17) wire
only to the relay and ingest seams; P14.20/P14.21 observability consumes
the manifest cadence and connection states without gaining any control
surface; no Phase 15–20 feature may add an `AuthorityDeclaration` variant
or a §5.7 permission id to an adapter without a superseding ADR and the
never-granted review.

## Revisit conditions

Identical in kind to ADR-010's: granting any adapter direct or
bounded-automation authority requires a separate engineered and qualified
architecture, the §22.3 release gates, site-specific acceptance, and a new
ADR superseding ADR-010 AND this extension — post-1.0 only, per the
release ladder (v1.0.0 operator-approved control is already the
operator-approval gate, not an adapter authority).
