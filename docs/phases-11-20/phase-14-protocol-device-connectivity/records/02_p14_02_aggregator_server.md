# P14.02 — IEEE 2030.5 AggregatorServer: design + model types for the CSIP resource tree

| Field | Value |
|---|---|
| Subtask | P14.02 — server design + model types for the CSIP resource tree (FunctionSetAssignments, DERProgram/DefaultDERControl/DERControlList, ResponseList, PricingList, LogEventList), resource addressing and query semantics, DNS-SD discovery notes, mTLS server skeleton [S-1][S-3][S-5] |
| Wave / agent | Phase 14 wave 1, Agent A |
| Status | WRITTEN-UNRUN throughout: model types, addressing, scheduling, server skeleton, and the PROPOSED mapping are written; never compiled, never executed. **No server runs, no device connects, no message is exchanged.** The HTTP/TLS transport is a designed UNRUN seam (`AggregatorTransport`), not an implementation. |
| Sources | [S-1] IEEE 2030.5-2018 (gap analysis §11), [S-3] SunSpec CSIP Implementation Guide v2.1, [S-5] QualityLogic Intro to 2030.5, [S-156] SunSpec IEEE 2030.5 in production; the P14.01 architecture record (01) |
| Files touched | `crates/adapters/protocol-20305/src/lib.rs`, `src/model.rs`, `src/addressing.rs`, `src/scheduling.rs`, `src/server.rs`, `src/proposal.rs` (all NEW), crate `Cargo.toml` (NEW), root `Cargo.toml` member entry (record 01), checker entries (record 01) |

## 1. What was implemented (written, never run)

| Piece | Where | Honest status |
|---|---|---|
| CSIP resource-tree model types: EndDevice(+List), FunctionSetAssignments(+List), DERProgram(+List), DefaultDERControl, DERControl(+List), DERControlBase, Response(+List), Pricing(+List), LogEvent(+List), Sfdi/Lfdi/TimeType/ListMeta | `model.rs` | Implemented code with serde derives (camelCase design surface); mRID/SFDI/LFDI constructors validate fail-closed. UNRUN. Attribute spellings UNVERIFIED (§4 below) |
| Resource addressing: validated `ResourceHref`, the declared `AddressPlan` (with the REGISTERED RECOMMENDATION `csip_default()` paths — UNVERIFIED), list `ResourceQuery` paging semantics | `addressing.rs` | Implemented; paths declared, never hardcoded guesses; paging window clamped and deterministic. UNRUN |
| DERControl scheduling windows: `DerControlWindow` (start/duration/randomization), `effective_start`, `active_at` (half-open), `overlaps`; saturating arithmetic | `scheduling.rs` | Implemented pure functions; randomization applied as DECLARED data (UNVERIFIED semantics). UNRUN |
| AggregatorServer skeleton: deterministic `BTreeMap` store, declared-plan routing (GET reads, bounded POST writes to responses/log events only), paged serving, declared `ServerLimits` caps (oldest dropped), fail-closed `MtlsRequirements` (client certs STRUCTURALLY required; non-empty trust-anchor label set), `DnsSdAnnouncement` (UNVERIFIED default), the `AggregatorTransport` seam + `ResourceRequestHandler` trait | `server.rs` | Implemented model/skeleton code. UNRUN. The transport trait has NO implementation in this repository — that is the designed seam (§5) |
| The DERControl → command-lifecycle mapping: `ExternalControlProposal` with CONST `ENTRY_STATE = PROPOSED`, scope-gated construction, `reject_at_gate`/`adapter_may_transition_to` running the FROZEN `flexcore-domain` transition table, primacy-based `resolve_competing_controls` | `proposal.rs` | Implemented; the frozen table is the enforcement point (the architecture record 01 §3 rule). UNRUN |
| Unit tests for all of the above (construction gates, paging, caps, PROPOSED-entry, gate-skipping refusals) | `#[cfg(test)]` modules | Written complete; UNRUN |

## 2. The CSIP resource tree as modelled

The tree mirrors the CSIP-required aggregator hierarchy [S-3] — every node a
serde value type with UNVERIFIED spellings (§4):

| Resource | Modelled type | Notes |
|---|---|---|
| EndDeviceList / EndDevice | `model::EndDeviceList`, `model::EndDevice` | Enrollment surface; SFDI (non-zero, fail-closed) + optional LFDI; links to FSA and DERP |
| FunctionSetAssignments(+List) | `model::FunctionSetAssignments` | Binds DERP + RSPS; carries the DECLARED `primacy` value (UNVERIFIED field semantics) |
| DERProgram(+List) | `model::DERProgram` | Links to DefaultDERControl and DERControlList |
| DefaultDERControl | `model::DefaultDERControl` | The no-active-control fallback base |
| DERControl(+List) | `model::DERControl` | Timing (start/duration/randomization) + `DERControlBase` limits in NATIVE watts; canonical MW/MVAr conversion is the ACL's declared mapping, never inferred |
| Response(+List) | `model::Response` | Device acknowledgements collected by the aggregator; bounded by declared caps |
| Pricing(+List) | `model::Pricing` | MINIMAL placeholder record — the 2030.5 pricing class inventory is UNVERIFIED and intentionally not guessed |
| LogEvent(+List) | `model::LogEvent` | Device audit events; bounded; details carried as untrusted data (P1.08 at presentation) |

## 3. Addressing, query semantics, and DNS-SD

- **AddressPlan is declared, never hardcoded.** `AddressPlan::csip_default()`
  carries the registered-recommendation paths (`/edev`, `/fn`, `/derp`,
  `/derc`, `/derp/dderc`, `/rsps`, `/prc`, `/logevent`) — each UNVERIFIED
  against the standard text; a deployment declares its own plan and every
  path is validated (rooted, non-empty, distinct) at construction. The
  server serves only declared paths; anything else is a 404 that is
  indistinguishable from a missing resource by design.
- **Query semantics** are an explicit limit/offset paging model with exact
  meta recount (`all`/`results`) — a design simplification; the standard's
  own wire parameter names/mechanics are UNVERIFIED and nothing here speaks
  wire syntax.
- **DNS-SD discovery**: `DnsSdAnnouncement::csip_default(instance, port)`
  carries the service-type spelling `"csip"` as a REGISTERED RECOMMENDATION
  transcribed from secondary material [S-3][S-5] — **UNVERIFIED**; the
  implementation-time check must confirm the exact DNS-SD service type
  string, TXT record contents, and discovery semantics against the CSIP
  Implementation Guide. No discovery lookup is performed anywhere in this
  program.

## 4. UNVERIFIED-ASSUMPTION register (the openems.rs pattern)

1. **Attribute spellings** (camelCase, e.g. `opModGenLimW`, `mrid`) follow
   the 2030.5 schema as transcribed in [S-3][S-5] — UNVERIFIED against the
   IEEE 2030.5-2018 schema. A confirmed mismatch is a rename (serde
   `rename`) plus a record edit, never a silent reinterpretation.
2. **TimeType** is i64 seconds since the Unix epoch — the 2030.5
   TimeType/TimeOffsetType pair UNVERIFIED (epoch base, offset sign).
3. **SFDI/LFDI formats** are shape-only here (non-zero decimal / lowercase
   hex); the derivation (hash input, bit length, the SFDI shortening rule)
   is the security record 03 §3 implementation-time check.
4. **List semantics** (`all`/`results`, `pollRate`) UNVERIFIED.
5. **Response status vocabulary** — modelled as a declared u8, never
   interpreted; the standard's response-code semantics UNVERIFIED.
6. **Primacy ordering** (lower value wins; creation; deterministic mRID)
   follows [S-156] — UNVERIFIED, declared in code so a mismatch is a
   one-line rule edit.
7. **Wire encoding** (EXI per the standard, UNVERIFIED) is a transport-seam
   concern; the serde surface here is the JSON-facing design record.

## 5. The mTLS server skeleton and the transport seam

`server.rs` defines `AggregatorTransport` — **a trait with NO
implementation in this repository** (the P13.02 `JwksSource` precedent:
network behaviour is a runtime act, deliberately outside the repo). The
seam is a HUMAN-GATED implementation decision: the implementer must (a)
bind a TLS listener enforcing `MtlsRequirements` (client certificates are
structurally required — the type has no field that could disable them;
the trust-anchor set must be non-empty; revocation checking defaults
fail-closed), (b) map wire requests onto `ResourceRequest` and
`ResourceResponse` back, (c) choose the HTTP framework and wire codec —
both open implementation decisions under conventions §4 discipline (this
batch adds no dependency of any kind; the workspace dependency line is
unchanged). `ServerError`, `TransportError` name every failure; the store
is a deterministic `BTreeMap`; response/log collections are bounded by
declared caps (oldest dropped) — unbounded device-facing lists are a
denial-of-service surface, not a feature. The only write the skeleton
accepts is a device Response or LogEvent to its declared collection; a
write anywhere else is refused (NotAllowed). This is not a command path:
**the aggregator never writes controls into a device — devices poll.**

## 6. The PROPOSED-entry rule (phase-14 context §3 constraint 1)

`proposal.rs` maps an external DERControl into an `ExternalControlProposal`
whose lifecycle entry state is the CONST `CommandState::Proposed` — no
constructor names any other entry state. The adapter may record exactly
{PROPOSED, REJECTED-at-the-PROPOSED-gate}; every other transition runs
`flexcore_domain::command_state::try_transition`, and the frozen table has
no PROPOSED → ISSUED, PROPOSED → APPROVED, or PROPOSED → FEASIBLE edge.
The scope gate runs BEFORE any mapping: an adapter whose declared scope
does not cover the Control resource class cannot ingest control objects
at all. The validation, feasibility, and authorization gates are the
platform's; this adapter runs none of them and submits nowhere (the
ingest port is a later seam, P14.17/P14.05).

## 7. ASSUMPTION / UNRUN register

1. **UNRUN** — everything: no compile, no test run, no server bind, no
   TLS handshake, no device connection, no message exchange. Evidence
   cells EMPTY. No conformance claim of any kind (conventions §7.3).
2. **UNVERIFIED** — the register in §4 (spelling, time, SFDI/LFDI, list,
   response, primacy, wire encoding, DNS-SD naming).
3. **REGISTERED, not tuned** — the collection caps (defaults are
   test-constructed values, not production numbers), the revocation-check
   default (fail-closed), the paging model, and the CSIP default paths.
   Production values are deployment declarations pending the X-8
   pre-registration discipline.
4. **No dependency added** — the crate depends on
   flexcore-protocol-ports + flexcore-domain + serde + serde_json, all
   already in the workspace line.

## 8. Handoff notes

1. **P14.03 (this wave, Agent A)** consumes the skeleton's
   `MtlsRequirements` as the design anchor for the CA-chain / SFDI-LFDI /
   trust-anchor record.
2. **P14.04 (this wave, Agent A)**: the `tests/` suite (written-unrun)
   exercises this API; see record 04.
3. **Wave 3 (P14.17)**: the simulated 2030.5 client is the counterpart
   that makes this skeleton testable with zero hardware; **P14.18/P14.20**:
   the manifest cadence and the `ConnectionState::name()` vocabulary are
   the observability seams.
4. **Human gates**: first build; the standard-text verification pass (§4);
   the transport implementation decision (§5) — which is ALSO where the
   real TLS stack and HTTP framework dependency review happens.
