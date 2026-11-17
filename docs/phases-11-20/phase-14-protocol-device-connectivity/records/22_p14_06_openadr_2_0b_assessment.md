# P14.06 — OpenADR 2.0b compatibility assessment (record-only) and the P14-E-2 registration

| Field | Value |
|---|---|
| Subtask | P14.06 (OpenADR 2.0b compatibility assessment — record-only depth; expected verdict DEFER) |
| Wave / agent | Phase 14 wave 1, Agent B |
| Status | RECORD-ONLY: the honest deliverable is the assessment; no 2.0b code, schema, or model exists or should at this depth (plan §1 deliverable-depth statement) |
| Sources | [S-10] (OpenADR Alliance — current 3.0 program surface); [S-11] (OpenADR 2.0b Profile Specification, via the gridlinktechnologies.com mirror recorded in the gap analysis — the standard text itself was NOT consulted; every 2.0b detail below is UNVERIFIED against it) |
| Files touched | This record only (plus the `openadr_2_0b` `not_modelled` entry inside `crates/adapters/protocol-openadr/src/scope.rs`, which this assessment justifies) |

## 1. What 2.0b is, and what would differ from the landed 3.0 design

UNVERIFIED secondary-source reading of [S-11]; each row names the
implementation-time check (the standard text, or an interop test with a
named counterparty's VEN) that must confirm it before any 2.0b work:

| Dimension | 3.0 (landed design, record 21) | 2.0b legacy profile (assessment — UNVERIFIED) |
|---|---|---|
| Payload encoding | REST/JSON (the surface record 21 designs as data) | XML payloads over SOAP services (the EiRegisterParty / EiEvent / EiReport service family) — transport is request/response SOAP, not REST |
| Transport/security | HTTPS with standard web TLS; the platform's Phase 13/18 identity surfaces apply | Historically deployed with X.509 mutual-TLS AND the OpenADR "openADR TLS" profile variants; several deployed 2.0b VENs cannot be pinned to one security posture — a per-program security review would be mandatory |
| VTN/VEN interop surface | Single JSON schema; VEN management is a platform surface (record 21 §5 item 5) | A separate XML schema generation with its own payload versioning and venID/marketContext conventions that do not map 1:1 onto 3.0 |
| Mapping into the lifecycle | Event → PROPOSED (record 21 §3) | The SAME rule would apply verbatim — a 2.0b event is an external control object and enters at PROPOSED; nothing in the legacy profile changes the never-an-issued-command rule |
| Testing/certification | OpenADR Alliance 3.0 certification path (P14.19 records it) | A separate 2.0b certification track with its own test harness generation |

## 2. Assessment verdict: **DEFER**

The 2.0b surface is assessed, not implemented, because implementing it
without a named counterparty would produce protocol code with nothing to
talk to — the same reasoning the phase plan applies to the EU surfaces
(plan §1 record-only-depth list, which names P14.06 explicitly). The landed
3.0 design loses nothing by the deferral: the load-bearing invariants
(external events enter at PROPOSED; scope declaration; common manifest;
unit conversion at the adapter) are protocol-shape-independent and are
already encoded once in the common port layer (P14.01), where a future 2.0b
bridge adapter would implement them rather than re-decide them.

What would change the verdict: a named DR program or market requiring 2.0b
VEN/VTN interop [S-11] — which is exactly the closure condition the
E-decision below records.

## 3. Decision register: P14-E-2 — OPEN (agents never close E-decisions, conventions §4)

| Field | Value |
|---|---|
| ID | P14-E-2 |
| Question | Does the platform serve the legacy OpenADR 2.0b XML/SOAP profile alongside 3.0? |
| Options | (a) 3.0-only; (b) a 2.0b bridge adapter on demand; (c) a full 2.0b VTN alongside 3.0 |
| DEFAULT (RECOMMENDATION) | **"3.0-first, 2.0b on program demand"** — effectively (a) now, (b) when a named program requires it. This record implements the assessment part of the default (P14.06 records the assessment either way, plan §4); no 2.0b code exists |
| What would close it | A named DR program or market requiring 2.0b VEN/VTN interop [S-11], with its security posture documented (the per-program review in §1 row 2) |
| Status | **OPEN** — the human owner closes it; this agent records the default only |

## 4. Honest boundaries

No 2.0b message was exchanged (no message of any kind was), no 2.0b schema
was written, and no interop was attempted. The 2.0b details in §1 are
secondary-source readings labelled UNVERIFIED; they justify the deferral and
the `not_modelled` entry — they are not a basis for implementation.
