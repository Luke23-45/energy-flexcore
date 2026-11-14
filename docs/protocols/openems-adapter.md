# OpenEMS Adapter Guide

| Field | Value |
|---|---|
| Scope | The FlexCore `openems-rest` adapter (`crates/edge-agent/src/adapters/openems.rs`): the read-only integration between the FlexCore edge agent and an OpenEMS Edge site EMS |
| OpenEMS version pin | **2026.11.0** — recorded in `ledger/phase-06-edge-protocol/RESEARCH.md` §1 (checked 2026-11-07; published 31 October 2026, Zenodo record). The lab run must pin **the version the lab Edge actually runs** and re-diff the mapping — OpenEMS monthly releases move component IDs and channels (2026.11.0 removed Energy Scheduler v1). The pin below is the recorded research pin, NOT a lab-verified one |
| API-verification status | **UNVERIFIED.** No REST endpoint path, JSON field name, or response shape in this guide has been executed against a real Edge. Every HTTP/JSON element is an explicit assumption, labelled in code (`// UNVERIFIED-ASSUMPTION`) and listed in `ledger/phase-06-edge-protocol/10_11_openems.md` §2 |
| Control status | Read-only. The adapter has **no write surface** — structural, not policy (§3 below). FlexCore is a supervisory orchestration layer; it never injects into the OpenEMS controller cycle (§18.1, AGENTS.md rule 3) |
| Conformance status | Bounded claim only — see §6. Outside the §6 boundary block, no claim of that kind (certification, conformance, compliance of any protocol) is permitted anywhere in FlexCore-authored text |

## 1. What is integrated

| Aspect | Recorded state |
|---|---|
| Protocol / product | OpenEMS Edge (Java, OSGi component model; Natures abstract hardware; Channels are single pieces of component state; EdgeConfig is the JSON definition of the Edge's configuration) — per `RESEARCH.md` §1, sources: OpenEMS Edge architecture docs and the 2026.11.0 Zenodo record |
| Transport | Minimal read-only HTTP/1.1 GET client (std-only, no new dependencies); connect timeout 5 s, read timeout 10 s (composition constants, recorded as DEFAULT in `10_11_openems.md` D-10.6) |
| Channel used | The FlexCore adapter's own REST reads. **This is NOT the OpenEMS Edge↔Backend channel** — OpenEMS authenticates to its backend by API key over WebSocket, and the Backend.Edge.App ↔ Edge.Manager link is trusted-network without peer authentication (RESEARCH.md R6-C1). FlexCore must not claim OpenEMS speaks mTLS natively, and must not downgrade its own uplink to API-key-only auth |
| Adapter registry | id `openems-rest`, status `Implemented` (via `AdapterRegistry::with_openems_available()`); the default allowlist table still carries the legacy PLANNED entry pending a one-line recorded follow-up (`10_11_openems.md` §6) |

## 2. REST API usage (ALL UNVERIFIED — carried over verbatim from the decision record)

The following endpoints and shapes are **assumptions to verify at the lab Edge**, not documented facts (`10_11_openems.md` §2; the OpenEMS REST API was never executed when the adapter was written):

| # | Assumption | Verify at the lab Edge by |
|---|---|---|
| A1 | EdgeConfig is fetched at `GET /rest/edge/config` | Diff the real endpoint path (OpenEMS REST API docs; the Edge may instead expose JSON-RPC over WebSocket) |
| A2 | The EdgeConfig JSON carries `components` as an object keyed by component id; each component carries `factoryId`, `alias`, and a `channels` object keyed by channel id | Dump the real EdgeConfig and diff the shape (including whether `components` is an object or an array) |
| A3 | Each channel entry carries an optional `unit` string (e.g. `"W"`, `"var"`, `"%"`) and human text fields | Diff the real channel metadata keys (`unit` vs `unitName` vs none) |
| A4 | A channel value is fetched at `GET /rest/channel/{component}/{channel}` | Diff the real per-channel read path |
| A5 | The channel-value response is an object with a numeric `value` and a numeric `timestamp` (Unix epoch milliseconds) | Capture a real response; confirm the timestamp unit and field names. A missing timestamp is a named `ReadFailed` — `event_time` is never fabricated |
| A6 | REST channel values carry no quality code; each mapping-profile entry therefore DECLARES the code attached to a fresh reading | Confirm whether OpenEMS exposes any channel validity/quality information; if it does, extend the adapter's declared-quality mapping explicitly |
| A7 | Component/channel ids are REST-path-safe (`[A-Za-z0-9._-]`); anything else is recorded unmapped | Check real id character sets |
| A8 | The Edge's clock-synchronisation status is UNKNOWN | Record the Edge's NTP/synchronisation state in the lab run |

Sign conventions are **declarations, not verifications**: the mapping profile declares `ess0/ActivePower` export-positive, `meter0/ActivePower` import-positive, and reactive power injection-positive — each labelled UNVERIFIED-ASSUMPTION inside the profile's own `assumptions` array, announced at startup, and correctable by a profile edit when the lab shows the real sign. No unit or sign is ever inferred from observed values (§10.2).

## 3. The read-only guarantee (structural)

| Layer | Guarantee |
|---|---|
| Trait level | `OpenemsRestTransport` declares exactly ONE method, `get`. No caller of the adapter surface can express a state-changing request against the Edge |
| Client level | The transport formats exactly one request shape — `GET <path> HTTP/1.1` with `Connection: close` — and nothing else |
| Module level | A lexical scan test over the adapter module refuses writer method definitions, writer request-builder shapes, and verb string literals (deliberately broad — it also catches commented-out writers) |
| Runtime witness | The test double counts GETs and carries a `write_attempts` counter the adapter surface cannot increment — asserted zero |
| No controller injection | The adapter enables/configures NO OpenEMS Controller and touches NO Scheduler (§4 below) |

## 4. Execution-model honesty

OpenEMS Edge runs an input–process–output (IPO) cycle: a Cycle component sets the process image, then executes Controllers in Scheduler order (`RESEARCH.md` §1, OpenEMS Edge architecture). FlexCore is a supervisory orchestration layer: through this adapter it **observes channels produced by that cycle and never injects into it** (§18.1; AGENTS.md rule 3 — no inverter inner-loop, sub-cycle, or protection-grade control). Standing up the lab Edge with simulated devices (the OpenEMS Getting Started shape) is OpenEMS's own simulation running its own controllers — FlexCore only reads the resulting channels. Note that the Getting Started lab defaults (`admin`/`admin`, `Metadata.Dummy` accept-any-password, `ws://`) are labelled testing-only by OpenEMS itself and must never become a deployment model (RESEARCH.md R6-C2).

## 5. Mapping profile (`openems-mapping-profile.v1.json`)

The channel mapping lives in a **versioned configuration file** (`crates/edge-agent/config/openems-mapping-profile.v1.json`), not hardcoded in the adapter (P6.10 step 4; `10_11_openems.md` §5):

| Rule | Behaviour |
|---|---|
| Envelope check | `profile` identity string + integer `version` = 1; a different version is a named refusal, never an interpreted guess; a migration is a reviewed change |
| Channel classification | Every discovered channel lands in exactly one register class: MAPPED / EXPLICITLY_UNMAPPED (with reason) / UNMAPPED (with reason) / AMBIGUOUS (with reason). Ambiguity causes include duplicate profile entries, conflicting mapped/unmapped declarations, unknown quantity/unit spellings, an active-power entry without a declared native sign, a unit/quantity family mismatch, and a source-declared unit conflict. None is resolved by guessing |
| Quality | Profile entries declare the code attached to a fresh reading; an entry without the declaration yields readings the anti-corruption layer refuses (`QualityNotDeclared`). Readings older than the profile's `stale_after_millis` are stamped STALE by the adapter and map to canonical `STALE` through the frozen quality table (§10.4 — degraded states are never upgraded to GOOD) |
| Reactive convention | One site-level `reactive_source_convention` declaration with optional per-entry override; the convention travels on every reactive record, never applied as a silent flip (§10.2) |
| Source sequence | Edge-assigned, monotonically increasing per point, incremented per successful read |

## 6. Conformance suite and the claim boundary

The FlexCore-side suite is `flexcore-edge-adapter-conformance-suite-v1` (P6.19; CI job `edge-agent-suite` in `.github/workflows/ci.yml` — **CI execution UNRUN**, the first human run is the verification point). Per `19_23_22_conformance_security_performance.md` §1: **no official protocol conformance suite applies to the OpenEMS REST channel** — the suite is 100 % FlexCore-side, and the absence of an OpenEMS REST-channel certification program is recorded as UNVERIFIED-ABSENCE (what the checked sources support), not a verified negative.

The claim boundary, verbatim from `19_23_22_conformance_security_performance.md` §3:

> Passing this suite entitles FlexCore to claim **exactly** this:
>
> *"passes the FlexCore edge adapter conformance suite version 1 against
> the recorded OpenEMS REST API shape."*
>
> It does **NOT** entitle any claim of OpenADR Alliance, Open Charge
> Alliance (OCA), SunSpec, or IEEE 2030.5 certification or conformance.
> Those require the respective programs' own processes, their own test
> tools, and (for several) verification at their appointed test houses
> (report §2.3: implementing an adapter is not a conformance claim).
> FlexCore is not certified, and no marketing text, README, or release note
> may claim certification, conformance, or compliance of any protocol.

That block-quoted sentence is the only place in FlexCore-authored text where the words certified/certification may appear in a protocol claim; everywhere else they are forbidden (`19_23_22` §3 grep list).

## 7. Known limitations

| Limitation | Status |
|---|---|
| UNVERIFIED API shape | Every endpoint/field above (§2) is an assumption until the P6.11 lab run confirms it; a shape mismatch is a fixture/profile edit and a suite re-run, never a reinterpretation (`19_23_22` D-2) |
| No TLS on this hop | Only `http://` composes; a `https://` base URL is a named refusal. A rustls-backed transport for the REST hop is the recorded open item. The FlexCore edge↔central uplink is unaffected (mTLS, P6.07) |
| No write path | Structural (§3) — and the `EmsAdapter` port has no dispatch/write method at all; the edge command path is disabled by default behind the P6.14 gate |
| Lab evidence absent | No lab Edge has been parsed, no capture reviewed, no reconciliation run — the P6.11 lab-run record (`10_11_openems.md` §7) is a TEMPLATE with EMPTY cells |
| Version drift | OpenEMS releases monthly; component IDs and channels move. Re-check the channel list on every release bump; re-pin at the lab run |
| OpenEMS license | Not recorded by any in-repo source — an open D4 item (`02_license_audit.md` §4). Integration uses the documented REST API over the network (an external-system boundary); no OpenEMS code is vendored, and no client library is added without the license-audit note first |
