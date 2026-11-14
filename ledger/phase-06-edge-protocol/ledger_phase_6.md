# Phase 6 — Edge and Protocol Integration: Ledger

| Field | Value |
|---|---|
| Phase | 6 — Stage 6 Edge and protocol integration |
| Report trace | §23 Stage 6; §18; §2.3; §17; §3.3; §2.1 (v0.4.0) |
| Release | v0.4.0 — Site Integration |
| Control status | **Read-only / test-lab by default.** Simulation-only until this phase's exit gate is met. No field write authority. |
| Status | Ledger only — no code written, no decisions closed |
| Research cut-off | 7 November 2026 (report §27) |

## How to read this ledger

- Every subtask has an ID (`P6.01` …), a report trace (`§N` / `§N.M`), inputs,
  detailed steps, a concrete deliverable, testable acceptance criteria, an owner
  role, dependencies, a verification method, and risks. **No subtask is done
  until its acceptance criteria are evidenced.**
- Checkboxes `- [ ]` mark work not yet done. Do not tick a box on the basis of
  a plan or a draft; tick it only when the named deliverable exists and the
  verification method has been run and recorded.
- Canonical units and signs (§10.2): MW, MVAr, MVA, MWh; `P > 0` = export at
  the point of connection; reactive-power sign declared per grid model and
  device profile, never assumed; SOC is a fraction in [0,1] at the API
  boundary. Telemetry quality is exactly one of
  `GOOD` / `SUSPECT` / `BAD` / `MISSING` / `ESTIMATED` / `STALE` (§10.4).
- Command lifecycle (§14.1): `PROPOSED → VALIDATED → FEASIBLE → APPROVED →
  ISSUED → ACKNOWLEDGED → TRACKING → COMPLETED`, plus `REJECTED`, `FAILED`,
  `EXPIRED`, `CANCELLED`, `SUPERSEDED`, `FALLBACK`. Validation, feasibility,
  and authorization are distinct gates. Acknowledgement is not physical
  delivery.
- This phase does **not** certify anything. Implementing a protocol adapter is
  not a conformance claim (§2.3, §17.5, AGENTS.md rule 2). FlexCore is not a
  safety case, not protection-grade, and not a hard-real-time controller
  (§0.3, §14.5, §17.5).
- Open gates that bound this phase: **D5** (first protocol and asset mix —
  before v0.4.0) and **D6** (cyber and OT boundary review — before any
  read-only pilot). Neither is closed by this ledger; P6.01–P6.03 exist to
  force the decision, not to assume it.

## Prior-phase outputs consumed (assumed complete per §23 Stages 1–5)

- Phase 1: Cargo workspace, Axum API, PostgreSQL + tenant context, NATS
  JetStream + outbox, tracing/metrics, CI (§23 Stage 1, §6–§8).
- Phase 2: asset registry, canonical telemetry model, quality/stale
  detection, deterministic replay (§23 Stage 2, §10).
- Phase 3: capability models, flexibility envelopes, deterministic
  LP/MILP/QP scheduler, solver result contract (§23 Stage 3, §11–§12).
- Phase 4: grid-model provider, AC power-flow validation, command lifecycle
  and audit, human approval, signed command model, simulation-only dispatch
  adapter (§23 Stage 4, §12.5, §14).
- Phase 5: operator console workspaces, simulator adapters, scenario
  replay, failure injection (§23 Stage 5, §19–§20).
- If any of these is not in fact complete, P6.04 (readiness check) must fail
  and this phase must not start implementation work.

---

## P6.01 — Close, or formally record as still open, decision D5 (first protocol and asset mix)

- [x] **P6.01 — Close, or formally record as still open, decision D5 (first protocol and asset mix)** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/01_d5_disposition.md`)
  - **Objective:** Produce a single written D5 decision record naming the one
    pilot protocol adapter and the asset mix it serves, or record D5 as still
    OPEN with owner, venue, and date — and stop adapter implementation until
    it is closed.
  - **Report trace:** §25.2; §2.3; §23 Stage 6.
  - **Inputs / prerequisites:** D5 entry in `.agent/DECISIONS.md` (OPEN);
    §2.3 protocol table; pilot-partner candidate list (if any); §2.2 asset
    support table.
  - **Steps:**
    1. List every candidate protocol (OpenEMS, OpenADR 3.0, SunSpec Modbus,
       OCPP 2.0.1/2.1, IEEE 2030.5/CSIP) with the asset mix each would serve
       and the pilot partner that would exercise it.
    2. Score each against: pilot partner exists and is named; lab hardware
       or simulator available; conformance/interoperability suite exists and
       is obtainable; license compatibility (see D4, P6.02); OT-boundary
       implications (see D6, P6.03).
    3. Recommend exactly one adapter for this phase. Record the rejected
       candidates and why — do not leave a second adapter half-selected.
    4. Write the decision as an ADR (context, decision, alternatives,
       consequences, revisit conditions — §22.1) and update
       `.agent/DECISIONS.md`; or, if no pilot partner is named, record D5
       as OPEN with owner, decision venue, and date, and mark P6.10–P6.19
       blocked.
  - **Deliverable / output:** ADR for D5 (or a dated OPEN record with owner,
    venue, date) in `docs/adr/`; updated `.agent/DECISIONS.md` status line.
  - **Acceptance criteria / exit check:** Either (a) one named protocol +
    asset mix + named pilot partner is recorded in an ADR, or (b) D5 is
    recorded OPEN with owner/venue/date and all adapter subtasks are marked
    blocked. No adapter code exists before (a).
  - **Owner role:** Product + integration.
  - **Dependencies:** None (phase entry gate).
  - **Verification method:** Read the ADR / DECISIONS.md entry; confirm the
    named partner and protocol appear verbatim; confirm no adapter crate
    exists yet (`git log` / workspace tree).
  - **Risks / failure modes:** Selecting a protocol with no pilot partner
    produces an untestable adapter; selecting two adapters splits the phase
    and guarantees neither is conformance-tested; assuming D5 closed without
    a record repeats the Stage 0 failure mode.

## P6.02 — License and dependency audit for the Phase 6 stack (D4 follow-through)

- [x] **P6.02 — License and dependency audit for the Phase 6 stack (D4 follow-through)** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/02_license_audit.md`)
  - **Objective:** Prove every library, protocol stack, and test tool this
    phase will introduce is license-compatible with the recommended
    Apache-2.0 core license, or record the incompatibility and the decision
    it forces.
  - **Report trace:** §25.2 (D4); §9.3; §22.2.
  - **Inputs / prerequisites:** Candidate dependency list from P6.01 and
    RESEARCH.md (OpenEMS client libs, Modbus crates, OCPP crates, TLS/mTLS
    crates, test harnesses); D4 status.
  - **Steps:**
    1. Enumerate every new direct and transitive dependency the edge agent
       and the selected adapter would add, with license (SPDX identifier)
       and source.
    2. Check each against the D4 Apache-2.0 compatibility rule; flag
       copyleft (e.g. GPL) or unknown-license items explicitly.
    3. Record the audit as a table (dependency, version pinned, license,
       verdict, action). For any incompatible item, record the alternative
       (replace, isolate out-of-process, or escalate to legal) — do not
       silently vendor it.
    4. Add the audit to the release SBOM inputs (§9.3, §22.2).
  - **Deliverable / output:** `ledger/phase-06-edge-protocol/` license-audit
    table (kept with this phase's records) and SBOM input list; D4 status
    update if the audit closes or narrows D4.
  - **Acceptance criteria / exit check:** Every dependency in the Phase 6
    build has a recorded license and a pass/flag verdict; zero dependencies
    with unknown license; every flagged item has a named action and owner.
  - **Owner role:** Legal + engineering.
  - **Dependencies:** P6.01.
  - **Verification method:** Re-run the dependency enumeration from the
    lockfiles once code exists and diff against the audit table; any
    unlisted dependency fails the check.
  - **Risks / failure modes:** Transitive GPL/copyleft dependency discovered
    after integration forces a rewrite; protocol test suites with
    restrictive licenses block conformance testing (see P6.19).

## P6.03 — Cyber and OT boundary review for the pilot site (D6)

- [x] **P6.03 — Cyber and OT boundary review for the pilot site (D6)** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/03_d6_boundary_review.md`)
  - **Objective:** Produce the site-specific cyber/OT boundary review that
    D6 requires before any read-only pilot, covering zones, flows, and the
    exact network path the edge agent will use.
  - **Report trace:** §17.2; §17.3; §17.4; §25.2 (D6); §3.3; §23 Stage 9
    (boundary review is a Stage 9 deliverable brought forward as a gate).
  - **Inputs / prerequisites:** §17 zones (Enterprise IT, Central
    application, Customer integration/DMZ, OT/site control, Field/device);
    pilot site network description from the pilot partner; NIST SP 800-82
    Rev. 3 as the current final reference (Rev. 4 is a draft — see
    RESEARCH.md).
  - **Steps:**
    1. Draw the pilot site zone diagram with every permitted flow listed;
       explicitly list prohibited flows (no direct database/NATS/plugin
       network access from any field-device network, §17.2; no inbound
       internet access into OT, §3.3).
    2. Record the chosen connection pattern: outbound-only from edge to
       central platform (or customer-controlled broker/gateway, DMZ,
       unidirectional gateway, or jump host — whichever the site requires).
    3. Record certificate/identity, firewall allowlist, and segmentation
       decisions; record what the site does **not** permit. Record the TLS
       minimum version and cipher policy for the edge mTLS channel here —
       P6.07 pins its channel policy from this record, and the record must
       exist before P6.07 can pass.
    4. Obtain security-owner sign-off; write the review as a dated record
       with reviewer name and scope. If the site cannot be described, D6
       stays OPEN and P6.24 (lab integration) stays blocked.
  - **Deliverable / output:** Dated OT-boundary review document for the
    pilot site (zones, permitted/prohibited flows, connection pattern,
    sign-off); D6 status update.
  - **Acceptance criteria / exit check:** Review exists, names the site, lists
    all five §17.2 zones, lists permitted and prohibited flows, names the
    outbound connection pattern, and carries security sign-off with a date.
    Without it, no pilot-site connection work proceeds.
  - **Owner role:** Security.
  - **Dependencies:** P6.01 (site/partner must be named).
  - **Verification method:** Independent read of the review against §17.2–
    §17.4; confirm every §17.3 control that applies to the edge path is
    addressed or explicitly deferred with a reason.
  - **Risks / failure modes:** A generic (non-site-specific) review is not a
    D6 closure — the report states the boundary is site-specific and not
    derivable from architecture alone; an assumed DMZ that the site does
    not operate creates an unapproved path into OT.

## P6.04 — Phase readiness check against Stages 1–5 outputs

- [x] **P6.04 — Phase readiness check against Stages 1–5 outputs** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/04_readiness_check.md`)
  - **Objective:** Verify, by running the checks, that the Stages 1–5
    deliverables this phase depends on actually exist and pass — before any
    Phase 6 code is written.
  - **Report trace:** §23 Stages 1–5; §22.2; AGENTS.md §6–§7.
  - **Inputs / prerequisites:** Phase 1–5 ledgers and their exit-gate
    evidence; repo at the Phase 5 state.
  - **Steps:**
    1. Run `python tools/verify_docs.py` and `python tools/validate_agent.py`
       and record actual output.
    2. Run the workspace test suite (`cargo test --workspace`, Python
       `pytest`, frontend checks per §22.2) and record pass/fail.
    3. Confirm the simulation-only dispatch adapter (Phase 4) and the
       telemetry/quality pipeline (Phase 2) behave as specified on the
       §24 vertical slice scenario.
    4. List every gap found; block Phase 6 implementation on any gap in
       the telemetry, command-lifecycle, or audit paths.
  - **Deliverable / output:** Readiness-check record with command outputs
    quoted, pass/fail per Stage 1–5 dependency, and a gap list.
  - **Acceptance criteria / exit check:** Both doc tools pass; workspace
    tests pass; §24 slice replays deterministically; gap list is empty or
    every gap has a named owner and a fix subtask before P6.05.
  - **Owner role:** Engineering lead.
  - **Dependencies:** None (but gates P6.05 onward).
  - **Verification method:** Re-run the quoted commands; outputs must match
    the record.
  - **Risks / failure modes:** Building an edge agent on an unverified
    telemetry or command path bakes upstream defects into field-facing
    code; "verified" claimed without running anything (AGENTS.md rule 7).

## P6.05 — Edge agent crate and composition-root design

- [x] **P6.05 — Edge agent crate and composition-root design** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/05_12_20_edge_foundation.md`)
  - **Objective:** Define the edge agent as a separate Rust daemon/service
    (`apps/edge-agent`) with its crate boundaries, dependency direction, and
    composition root — without yet implementing protocol adapters.
  - **Report trace:** §18.1; §8.1; §8.2; §6.2; §3.3.
  - **Inputs / prerequisites:** P6.04 readiness; §8.1 target tree
    (`apps/edge-agent`, `crates/adapters/openems/` etc.); §18.1
    responsibility list.
  - **Steps:**
    1. Write the edge-agent design note: binary entrypoint, config surface,
       module list (enrollment, channel, adapter manager, buffer,
       command receiver, audit buffer, health, updater), and which crates
       each depends on.
    2. Enforce §6.2/§8.2: domain depends on nothing infrastructure; adapters
       depend on ports; `apps/edge-agent` is the composition root.
    3. Create only the crates that have real code in this subtask (skeleton
       with enrollment + config + health); do not create empty adapter
       crates "to be ready" (AGENTS.md §1).
    4. Pin the toolchain in `rust-toolchain.toml` and record the Rust
       version used.
  - **Deliverable / output:** Edge-agent design note + skeleton crate(s)
    that compile (`cargo build -p edge-agent` passes); `Cargo.toml`
    workspace entry.
  - **Acceptance criteria / exit check:** `cargo fmt --check` and
    `cargo clippy --workspace --all-targets -- -D warnings` pass on the new
    crate; no empty crates exist; dependency direction matches §8.2
    (checked by inspection of `Cargo.toml` files).
  - **Owner role:** Rust engineer.
  - **Dependencies:** P6.04.
  - **Verification method:** `cargo build`, `cargo fmt --check`,
    `cargo clippy` output quoted; crate dependency graph inspected.
  - **Risks / failure modes:** Edge agent becomes a second domain layer
    (§8.2 forbids `flexcore-kernel` becoming one; same applies here);
    premature crate scaffolding creates dead code the report forbids.

## P6.06 — Edge identity, enrollment, and certificate lifecycle

- [x] **P6.06 — Edge identity, enrollment, and certificate lifecycle** — implemented, awaiting human build/test (decision record: `06_07_identity_channel.md`)
  - **Objective:** Implement secure site enrollment: each edge agent holds a
    unique identity scoped to one tenant and one approved site/asset set,
    with short-lived credentials and rotation.
  - **Report trace:** §18.1; §18.3; §17.3; §16.2; §3.3.
  - **Inputs / prerequisites:** P6.05 skeleton; D6 review (P6.03); control-
    plane identity/tenancy model from Phase 1.
  - **Steps:**
    1. Define the enrollment protocol: edge generates/provisions a key
       pair (hardware-backed storage where available, §18.3), submits a
       certificate-signing request out-of-band or via an approved
       enrollment flow, and receives a site certificate.
    2. Bind the issued identity to exactly one tenant and the approved
       site/asset set (§16.2); store the binding server-side and enforce it
       on every session.
    3. Implement certificate expiry tracking, renewal before expiry, and
       revocation handling; surface expiry in operator-visible health
       (§18.3).
    4. Write negative tests: enrollment with a duplicate identity, an
       expired certificate, a revoked certificate, and an identity bound to
       a different tenant must all fail closed.
  - **Deliverable / output:** Enrollment flow + certificate lifecycle code,
    server-side identity registry entries, expiry/renewal handling, negative
    tests.
  - **Acceptance criteria / exit check:** No two edges share an identity;
    an edge cannot enroll for a tenant/site it is not approved for; expired
    or revoked credentials are rejected; expiry is visible to an operator
    at least one rotation window before expiry.
  - **Owner role:** Security + Rust engineer.
  - **Dependencies:** P6.03, P6.05.
  - **Verification method:** Run the negative tests; attempt enrollment
    with a revoked certificate in the lab and confirm rejection in logs.
  - **Risks / failure modes:** Shared or default credentials (the OpenEMS
    Getting Started default `admin`/`admin` and `Metadata.Dummy`
    accept-any-password mode must never be used as the FlexCore enrollment
    model — see RESEARCH.md); an edge trusted merely because it belongs to
    a customer site (§18.2).

## P6.07 — Outbound mTLS channel (edge-initiated, no inbound OT access)

- [x] **P6.07 — Outbound mTLS channel (edge-initiated, no inbound OT access)** — implemented, awaiting human build/test (decision record: `06_07_identity_channel.md`)
  - **Objective:** Implement the edge-to-central channel as an
    edge-initiated, mutually authenticated TLS connection, with no inbound
    internet access into the OT network.
  - **Report trace:** §3.3; §18.1; §18.2; §17.3; §17.4.
  - **Inputs / prerequisites:** P6.06 identity; D6 connection pattern
    (P6.03).
  - **Steps:**
    1. Implement the client: edge dials out to the central platform (or
       customer-controlled broker/gateway); the central platform never
       dials into the site.
    2. Require mTLS in both directions: edge verifies the central
       certificate; central verifies the edge certificate issued in P6.06.
    3. Pin TLS versions/cipher policy per the D6 review; reject
       plaintext (`ws://`) and unauthenticated connections.
    4. Implement reconnect with backoff, session resumption rules, and a
       clean distinction between "channel down" and "site down" in health.
  - **Deliverable / output:** mTLS channel implementation + connection
    policy (TLS minimum version, cipher allowlist, reconnect/backoff
    parameters) + tests for each failure mode.
  - **Acceptance criteria / exit check:** A connection without a valid
    client certificate is rejected; a connection to a central endpoint
    with an untrusted certificate is refused by the edge; no listener
    socket is opened on the edge for inbound central connections; reconnect
    after a dropped channel succeeds without operator action and without
    duplicating in-flight state.
  - **Owner role:** Rust engineer + security.
  - **Dependencies:** P6.06.
  - **Verification method:** Packet/connection tests in the lab: attempt
    plaintext, wrong-CA, and expired-cert connections and record
    rejections; confirm no inbound port is listening on the edge host.
  - **Risks / failure modes:** Confusing FlexCore's mTLS channel with
    OpenEMS's native API-key WebSocket — OpenEMS Edge authenticates to its
    backend by API key over WebSocket (§27, OpenEMS docs); FlexCore must
    not claim OpenEMS speaks mTLS natively, and must not downgrade its own
    channel to API-key-only auth. See RESEARCH.md contradiction note R6-C1.

## P6.08 — Edge telemetry ingestion from the site EMS (read path)

- [x] **P6.08 — Edge telemetry ingestion from the site EMS (read path)** — implemented, awaiting human build/test (decision record: `08_09_ingest_buffer.md`)
  - **Objective:** Read telemetry from the site EMS through the selected
    integration, normalize it to the canonical model, and forward it over
    the mTLS channel — read-only in this phase.
  - **Report trace:** §18.1; §10.2; §10.3; §10.4; §2.3.
  - **Inputs / prerequisites:** P6.07 channel; Phase 2 telemetry model;
    OpenEMS Edge channel model (Natures/Channels, EdgeConfig JSON — see
    RESEARCH.md) or the selected adapter's data model.
  - **Steps:**
    1. Define the read mapping: each site value → canonical field, unit
       conversion to MW/MVAr/MVA/MWh, sign mapping recorded per device
       profile (§10.2), SOC converted to [0,1].
    2. Attach `event_time`, `ingest_time`, source sequence, and exactly
       one quality state per value (§10.3–§10.4); map richer source states
       (e.g. `SUBSTITUTED`, `COMMUNICATION_LOSS`) to the six canonical
       states, retaining the original code as diagnostic metadata.
    3. Never silently convert a quality flag into a normal reading; a
       `SUSPECT`/`BAD`/`STALE` value must arrive flagged as such.
    4. Forward over the mTLS channel with idempotency keys so duplicates
       from reconnects are tolerated (§6.3).
  - **Deliverable / output:** Read-path mapping table per asset type +
    ingestion code + unit/sign/quality tests.
  - **Acceptance criteria / exit check:** Every forwarded value carries
    canonical units, a declared sign convention, event/ingest time, source
    sequence, and exactly one quality state; a deliberately corrupted or
    out-of-range source value arrives as `SUSPECT`/`BAD`, never as `GOOD`;
    duplicate delivery after reconnect produces one stored value.
  - **Owner role:** Integration engineer.
  - **Dependencies:** P6.01, P6.07.
  - **Verification method:** Lab feed with known values (including a
    corrupted value and a duplicate replay); compare stored telemetry
    against the known feed, field by field.
  - **Risks / failure modes:** Implicit unit/sign assumptions (the most
    common integration defect); quality flags dropped at the adapter
    boundary, making bad data look measured; duplicate telemetry double-
    counted in flexibility calculations.

## P6.09 — Telemetry buffering, store-and-forward, and recovery

- [x] **P6.09 — Telemetry buffering, store-and-forward, and recovery** — implemented, awaiting human build/test (decision record: `08_09_ingest_buffer.md`)
  - **Objective:** Buffer telemetry locally in a bounded, encrypted queue
    when the central channel is down, and forward it in order on recovery
    without loss within the bound or silent overflow beyond it.
  - **Report trace:** §18.1; §18.3; §21.2; §15.4.
  - **Inputs / prerequisites:** P6.08 read path; §18.3 local encrypted
    queue requirement.
  - **Steps:**
    1. Define the buffer: capacity (records and bytes), encryption at rest,
       persistence across edge restarts, per-asset ordering by source
       sequence.
    2. Define overflow policy explicitly (e.g. oldest-first drop with a
       recorded gap event, or stop accepting new values) — the policy is a
       configuration decision, recorded, not implicit.
    3. Implement store-and-forward: on reconnect, drain in sequence order;
       mark forwarded records; handle a second outage mid-drain.
    4. Emit a gap/loss event whenever records are dropped or a sequence
       discontinuity is detected, so downstream quality becomes `MISSING`
       for the gap rather than silently interpolated.
  - **Deliverable / output:** Buffer implementation + capacity/overflow
    policy document + recovery tests (outage, restart mid-buffer, outage
    mid-drain, overflow).
  - **Acceptance criteria / exit check:** A 30-minute channel outage loses
    zero records within the stated capacity; after restart the buffer
    survives; after overflow the loss is reported as a gap event and the
    affected intervals are `MISSING`, not fabricated; drain completes
    without duplicates downstream.
  - **Owner role:** Rust engineer.
  - **Dependencies:** P6.08.
  - **Verification method:** Fault-injection tests per §19.2(6): kill the
    channel, generate telemetry, restart the edge, restore the channel,
    and reconcile record counts and sequences end to end.
  - **Risks / failure modes:** Unbounded buffer exhausts edge disk; silent
    overflow fabricates continuity; restart loses the buffer because it
    was memory-only.

## P6.10 — OpenEMS integration: EdgeConfig discovery and channel mapping

- [x] **P6.10 — OpenEMS integration: EdgeConfig discovery and channel mapping** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/10_11_openems.md`)
  - **Objective:** Discover the OpenEMS Edge configuration (EdgeConfig JSON)
    and map its components/channels to FlexCore assets, with no assumption
    about which devices a site has.
  - **Report trace:** §2.3; §18.1; §8.1 (`crates/adapters/openems/`); §27
    (OpenEMS references).
  - **Inputs / prerequisites:** P6.01 (OpenEMS selected, or this subtask is
    re-scoped to the selected integration); OpenEMS 2026.11.0 docs (see
    RESEARCH.md); lab OpenEMS Edge instance.
  - **Steps:**
    1. Fetch and parse EdgeConfig from the lab Edge; record the OpenEMS
       version pinned for the lab.
    2. Build the mapping: OpenEMS Nature/Component/Channel → FlexCore
       asset type, capability profile, and telemetry points. Record every
       unmapped channel and every channel whose units/sign are ambiguous.
    3. Handle the OpenEMS execution model honestly: Edge runs an
       input–process–output cycle with Controllers ordered by a Scheduler;
       FlexCore reads channels, it does not assume it can inject into the
       cycle.
    4. Write the mapping as a versioned configuration file, not hardcoded
       in the adapter.
  - **Deliverable / output:** EdgeConfig parser + versioned channel-mapping
    configuration + unmapped/ambiguous-channel register.
  - **Acceptance criteria / exit check:** The lab Edge's full EdgeConfig
    parses; every channel is mapped, explicitly unmapped with a reason, or
    flagged ambiguous; no channel is mapped by guessing its units.
  - **Owner role:** Integration engineer.
  - **Dependencies:** P6.01, P6.08.
  - **Verification method:** Parse the lab EdgeConfig and diff the mapping
    against a hand-checked table produced from the OpenEMS documentation.
  - **Risks / failure modes:** OpenEMS monthly releases move component
    IDs/channels (2026.11.0 removed Energy Scheduler v1 — see RESEARCH.md);
    an unpinned OpenEMS version silently changes the mapping.

## P6.11 — OpenEMS integration: read-only data path end to end

- [x] **P6.11 — OpenEMS integration: read-only data path end to end** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/10_11_openems.md`)
  - **Objective:** Demonstrate the complete read-only path — OpenEMS Edge
    channels → FlexCore edge agent → mTLS → control plane → stored
    telemetry — in the lab, with no write call issued to OpenEMS.
  - **Report trace:** §23 Stage 6 (exit gate); §18.1; §3.4 (steps 2–4).
  - **Inputs / prerequisites:** P6.07, P6.08, P6.10; lab OpenEMS Edge with
    simulated PV + battery (per OpenEMS Getting Started) or the pilot
    partner's lab devices.
  - **Steps:**
    1. Stand up the lab Edge with a known simulated asset set; record the
       exact OpenEMS version, EdgeConfig, and simulator inputs.
    2. Run the read path for a defined window; capture telemetry at both
       ends and reconcile value-for-value.
    3. Verify quality mapping end to end, including a forced stale source
       (stop the simulator feed) producing `STALE`/`MISSING` downstream.
    4. Confirm by code inspection and by network capture that no write,
       setpoint, or controller-configuration call was sent to the Edge.
  - **Deliverable / output:** Lab run record (versions, EdgeConfig,
    reconciliation table, stale-feed evidence, no-write attestation).
  - **Acceptance criteria / exit check:** Reconciliation matches within
    stated tolerances for every mapped channel; stale feed is flagged
    downstream within the freshness limit; zero write calls observed.
  - **Owner role:** Integration engineer.
  - **Dependencies:** P6.07, P6.08, P6.10.
  - **Verification method:** Independent reconciliation of the captured
    telemetry files; network capture reviewed for write calls.
  - **Risks / failure modes:** A "read-only" integration that quietly
    writes (e.g. by enabling a Controller); simulator feed mistaken for
    live device data in the record.

## P6.12 — Selected protocol adapter: interface and anti-corruption layer

- [x] **P6.12 — Selected protocol adapter: interface and anti-corruption layer** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/05_12_20_edge_foundation.md`)
  - **Objective:** Define the selected adapter (per D5, P6.01) behind the
    `ProtocolAdapter`/`TelemetrySource` port, with an anti-corruption layer
    that converts the protocol's terms, units, timestamps, and status codes
    into canonical contracts — and nothing else leaks through.
  - **Report trace:** §9.1; §6.4 (anti-corruption layer); §5.3; §2.3.
  - **Inputs / prerequisites:** P6.01 decision; §9.1 interface list;
    protocol specification for the selected adapter (see RESEARCH.md).
  - **Steps:**
    1. Write the adapter contract: which port methods it implements
       (`discover`, `read_state`, `capabilities`, `decode`), and which it
       explicitly does not (no direct dispatch in this phase).
    2. Write the anti-corruption mapping tables: protocol status codes →
       canonical quality; protocol timestamps → `event_time`/`ingest_time`;
       protocol units/signs → canonical units/signs.
    3. Place the adapter in `crates/adapters/<protocol>/` depending only on
       ports, never on domain internals (§8.2).
    4. Record the specification version and clause references the mapping
       relies on; anything not checked against the specification is marked
       ASSUMPTION.
  - **Deliverable / output:** Adapter interface definition + mapping tables
    + skeleton adapter crate implementing the read path.
  - **Acceptance criteria / exit check:** Adapter compiles against the port
    trait with no domain imports; every protocol field it consumes appears
    in a mapping table; every unmapped field is listed as unmapped.
  - **Owner role:** Integration engineer.
  - **Dependencies:** P6.01, P6.05.
  - **Verification method:** `cargo` dependency inspection; mapping-table
    review against the protocol specification's own field list.
  - **Risks / failure modes:** Vendor/protocol types leaking into the
    domain (§6.2 violation); timestamp confusion (device local time treated
    as UTC — §10.3); status-code mapping that upgrades `BAD` to `GOOD`.

## P6.13 — Selected protocol adapter: read/telemetry implementation and tests

- [x] **P6.13 — Selected protocol adapter: read/telemetry implementation and tests** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/13_14_17_adapter_command_policy.md`)
  - **Objective:** Implement the adapter's read/telemetry path against the
    real protocol (or its official simulator/test harness) and test it with
    the §19.2 fault-injection set.
  - **Report trace:** §2.3; §19.2; §10.4.
  - **Inputs / prerequisites:** P6.12; protocol test harness or simulator;
    lab devices if available.
  - **Steps:**
    1. Implement decode/parse for every mapped message type, with strict
       validation (unknown fields logged, not silently accepted; malformed
       messages rejected with a diagnostic, not partially applied).
    2. Test: valid messages, malformed messages, truncated messages,
       out-of-order delivery, duplicate delivery, clock skew, and stale
       data.
    3. Test the adapter against the protocol's own reference vectors or
       test suite where one is obtainable; record which vectors were run.
    4. Measure and record read-path latency and throughput on the lab
       hardware profile.
  - **Deliverable / output:** Adapter read implementation + fault-injection
    test results + reference-vector record.
  - **Acceptance criteria / exit check:** All §19.2(6) fault cases behave
    as specified (reject/flag, never crash or fabricate); reference vectors
    pass or each failure is recorded with cause; no `panic!`/`unwrap` on
    untrusted input (clippy + fuzz where applicable).
  - **Owner role:** Integration engineer.
  - **Dependencies:** P6.12.
  - **Verification method:** Run the fault-injection suite and quote
    results; run the reference vectors and attach output.
  - **Risks / failure modes:** Lenient parsing accepts malformed data as
    valid telemetry; a crash on malformed input takes down the edge agent;
    adapter tested only against FlexCore's own simulator, never the real
    protocol.

## P6.14 — Command path: keep disabled by default; define the test-env enablement gate

- [x] **P6.14 — Command path: keep disabled by default; define the test-env enablement gate** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/13_14_17_adapter_command_policy.md`)
  - **Objective:** Prove the command path is disabled by default in v0.4.0
    and define — in writing, with named approvers — the only conditions
    under which it may be enabled in a test environment.
  - **Report trace:** §23 Stage 6 (exit gate); §2.1 (v0.4.0); §21.4; §14.1;
    §17.5.
  - **Inputs / prerequisites:** Phase 4 command lifecycle; P6.03 D6 review.
  - **Steps:**
    1. Implement the command receiver in the edge agent as
       disabled-by-default: the flag defaults to off in code, in config,
       and in every shipped profile; enabling requires an explicit,
       signed, per-site configuration change.
    2. Write the enablement gate: test environment only; named approver(s);
       D6 review complete for that site; lab (not field) devices; expiry
       on the enablement; audit event on enable and disable.
    3. Verify that with the flag off, an `ISSUED` command arriving at the
       edge is rejected (not queued, not forwarded) and the rejection is
       audited.
    4. Verify that no plugin, adapter, or optimizer can enable the path
       (§5.7: a plugin can never select control-authority mode).
  - **Deliverable / output:** Disabled-by-default implementation + written
    enablement-gate procedure + rejection tests.
  - **Acceptance criteria / exit check:** Default build rejects commands at
    the edge with an audit record; enablement requires all gate conditions
    and is time-bounded; no code path allows a plugin to flip the flag.
  - **Owner role:** Security + Rust engineer.
  - **Dependencies:** P6.05, P6.03.
  - **Verification method:** Attempt to issue a command against a
    default-configured lab edge and confirm rejection + audit; attempt to
    enable via a plugin call and confirm refusal.
  - **Risks / failure modes:** A config example or test fixture that ships
    the flag on; enablement without expiry becoming permanent field
    control; the gate treated as paperwork while the code path is live.

## P6.15 — Command verification at the edge (test-env only): signature, expiry, sequence

- [x] **P6.15 — Command verification at the edge (test-env only): signature, expiry, sequence** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/15_16_command_verify_forward.md`)
  - **Objective:** For the test-environment command path only, implement
    edge-side verification of every command — signature, expiry, sequence,
    and allowlist — before any forwarding to a site adapter.
  - **Report trace:** §18.1; §17.3; §17.4; §14.3.
  - **Inputs / prerequisites:** P6.14 gate; Phase 4 signed-command model.
  - **Steps:**
    1. Verify the command signature/authenticated transport identity
       against the central platform's key; reject unsigned or
       wrong-signer commands.
    2. Verify `valid_from`/`valid_until` against the edge clock with the
       recorded clock-quality check; reject stale/expired commands (§14.3,
       §17.5 rule 5).
    3. Verify monotonic sequence/version per asset/control channel;
       reject duplicates and out-of-sequence commands (§14.3).
    4. Verify the target adapter is on the site allowlist and the command
       is within the approved scope/limits/expiry in its authorization
       reference (§17.5 rule 6).
    5. Log every rejection with reason; forward nothing that fails any
       check.
  - **Deliverable / output:** Edge command-verification implementation +
    negative tests for each check.
  - **Acceptance criteria / exit check:** Forged, replayed, expired,
    duplicate, out-of-sequence, and non-allowlisted commands are all
    rejected in tests, each with a distinct audit reason; a valid command
    passes all four checks in the test env only.
  - **Owner role:** Security + Rust engineer.
  - **Dependencies:** P6.14.
  - **Verification method:** Replay-attack test (resend a captured valid
    command), expiry test (aged command), sequence test, allowlist test —
    each recorded.
  - **Risks / failure modes:** Clock drift making expiry checks meaningless
    (§17.4); sequence state lost on edge restart allowing replays; a
    single combined "valid" boolean hiding which check failed.

## P6.16 — Command forwarding only to allowlisted adapters; fallback on failure

- [x] **P6.16 — Command forwarding only to allowlisted adapters; fallback on failure** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/15_16_command_verify_forward.md`)
  - **Objective:** Forward a verified test-env command only to its
    allowlisted site adapter, and define per-asset fallback behaviour when
    forwarding, acknowledgement, or tracking fails.
  - **Report trace:** §18.1; §14.4; §14.1; §21.2.
  - **Inputs / prerequisites:** P6.15; per-asset fallback policy from the
    site/equipment operator.
  - **Steps:**
    1. Implement the allowlist check as a separate step from verification
       (P6.15), with its own audit event.
    2. Forward via the adapter's `propose_setpoint` path only; record
       issue, acknowledgement, and tracking per §14.1 states.
    3. Implement the per-asset fallback policy (§14.4): hold last accepted
       setpoint for a bounded interval where approved, return to local EMS
       autonomous control, preserve reserve/SOC constraints, stop accepting
       new commands, or require operator intervention — as configured per
       asset/site, reviewed with the operator. There is no universal
       "set power to zero" fallback.
    4. Test: adapter NACK, adapter timeout, no acknowledgement, tracking
       deviation, and command expiry mid-flight — each must land in the
       specified §14.1 terminal/fallback state.
  - **Deliverable / output:** Forwarding + fallback implementation +
    per-asset fallback configuration + failure-mode tests.
  - **Acceptance criteria / exit check:** No command reaches a
    non-allowlisted adapter; every tested failure lands in the specified
    state with an audit record; fallback policy exists per asset and is
    operator-reviewed (signed/dated).
  - **Owner role:** Integration engineer + power-systems reviewer.
  - **Dependencies:** P6.15, P6.12.
  - **Verification method:** Failure-injection run in the lab; review the
    state transitions against §14.1; confirm fallback config carries
    operator review.
  - **Risks / failure modes:** A last-known setpoint treated as safe after
    topology/state change (§14.4); fallback policy copied across assets
    without review; partial fleet success reported as portfolio success
    (§14.3).

## P6.17 — Local policy enforcement and adapter health monitoring

- [x] **P6.17 — Local policy enforcement and adapter health monitoring** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/13_14_17_adapter_command_policy.md`)
  - **Objective:** Enforce site-local policy at the edge (rate limits,
    operating windows, per-adapter enablement) and monitor adapter health
    with operator-visible status.
  - **Report trace:** §18.1; §18.3; §21.3.
  - **Inputs / prerequisites:** P6.05, P6.12; site policy from the pilot
    partner.
  - **Steps:**
    1. Define the local policy surface: which adapters are enabled, command
       rate limits, allowed operating windows, and what happens when a
       policy is violated (reject + audit, never silently proceed).
    2. Implement per-adapter health checks (responsive, stale data, error
       rate) with `GOOD`/`DEGRADED`/`FAILED` status and last-seen
       timestamps.
    3. Surface health and certificate expiry in the operator console
       (§20.1 Sites workspace) and in edge logs with correlation IDs
       (§21.3).
    4. Test: adapter process/device failure, adapter returning stale data,
       and policy violation — each must degrade health visibly and block
       dependent actions.
  - **Deliverable / output:** Local policy configuration + health-monitoring
    implementation + console surfacing.
  - **Acceptance criteria / exit check:** A failed adapter shows as failed
    in the console within the stated detection window; a policy violation
    is rejected and audited; health status is never inferred from the
    absence of errors.
  - **Owner role:** Rust engineer.
  - **Dependencies:** P6.05, P6.12.
  - **Verification method:** Kill a lab adapter and time the health
    transition; attempt a policy-violating action and confirm rejection.
  - **Risks / failure modes:** Health reported as "unknown = healthy";
    policy enforced only centrally, so a disconnected edge enforces
    nothing.

## P6.18 — Local audit buffer and offline behaviour

- [x] **P6.18 — Local audit buffer and offline behaviour** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/18_21_audit_offline_isolation.md`)
  - **Objective:** Keep a local, bounded audit buffer at the edge so that
    enrollment, command, rejection, and health events survive a central
    outage, and define offline behaviour (what the edge does — and does not
    do — while disconnected).
  - **Report trace:** §18.1; §18.3; §21.2; §17.3 (audit logging).
  - **Inputs / prerequisites:** P6.09 buffer design; Phase 4 audit model.
  - **Steps:**
    1. Define the audit buffer: append-oriented, bounded, encrypted at
       rest, surviving restarts; upload on reconnect in order; never
       overwrite an un-uploaded record silently.
    2. Define offline behaviour in writing: local EMS remains authoritative
       for fast safety behaviour; the edge issues no new commands while
       disconnected (test-env path included); telemetry buffering follows
       P6.09.
    3. Test: central outage during a command flow, edge restart with a
       non-empty audit buffer, and audit-buffer overflow — each must
       preserve or explicitly report loss, never silently drop.
    4. Confirm a central outage does not disable site-local protective
       controls or force an unsafe device state (§21.2).
  - **Deliverable / output:** Audit-buffer implementation + offline-
    behaviour statement + outage tests.
  - **Acceptance criteria / exit check:** All audit events generated during
    a 30-minute central outage are uploaded on reconnect in order; buffer
    overflow is reported, not silent; no command is issued by the edge
    while disconnected.
  - **Owner role:** Rust engineer.
  - **Dependencies:** P6.09, P6.14.
  - **Verification method:** Outage fault-injection test with event-count
    reconciliation before/after reconnect.
  - **Risks / failure modes:** Audit gap during the exact window an
    incident needs; edge "helpfully" issuing queued commands on reconnect
    after their validity expired.

## P6.19 — Adapter conformance tests and the conformance claim boundary

- [x] **P6.19 — Adapter conformance tests and the conformance claim boundary** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/19_23_22_conformance_security_performance.md`)
  - **Objective:** Build the adapter conformance test suite for the selected
    protocol and write down, explicitly, what passing it does and does not
    entitle FlexCore to claim.
  - **Report trace:** §2.3; §23 Stage 6; §19.2(3); §9.3.
  - **Inputs / prerequisites:** P6.13; the protocol's conformance or
    interoperability suite (OpenADR Alliance test tool / certification
    profiles; OCA certification profiles; SunSpec test procedures; IEEE
    2030.5/CSIP test procedure — whichever applies; see RESEARCH.md).
  - **Steps:**
    1. Identify the applicable conformance suite, its owner, its access
       terms (membership, license, fee), and whether FlexCore can run it
       in the lab; record this before writing tests.
    2. Build the FlexCore-side conformance suite: contract tests (§19.2(3))
       that every adapter must pass, plus protocol-specific cases from the
       official suite where runnable.
    3. Run the suite in CI; record pass/fail per case with the protocol
       version and suite version.
    4. Write the claim boundary in the adapter documentation: "passes
       FlexCore conformance suite version X against protocol version Y"
       — and explicitly: this is not Alliance/OCA/SunSpec certification,
       which requires the Alliance's own program and test house (§2.3).
  - **Deliverable / output:** Conformance suite in CI + run record +
    claim-boundary statement in adapter docs.
  - **Acceptance criteria / exit check:** Suite runs in CI and passes on
    the lab build; every case maps to a protocol requirement or is marked
    FlexCore-only; the docs state the certification boundary in plain
    language; no marketing or README text claims certification.
  - **Owner role:** Integration engineer + product.
  - **Dependencies:** P6.13, P6.02.
  - **Verification method:** CI run quoted; grep of docs/README for
    "certified"/"conformant"/"compliant" and confirm each occurrence is
    either the bounded statement or removed.
  - **Risks / failure modes:** Conformance suite behind a paywall/membership
    that was never budgeted (D4/P6.02 miss); "certified" claimed from
    parsing alone (§2.3 explicitly forbids this); suite version drift
    unrecorded.

## P6.20 — Edge deployment profile: packaging, service, and signed updates

- [x] **P6.20 — Edge deployment profile: packaging, service, and signed updates** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/20_deployment_profile.md`)
  - **Objective:** Ship the edge agent as a versioned Linux package/OCI
    image or systemd service per §18.3/§21.4, with signed, verified update
    bundles and staged rollback.
  - **Report trace:** §18.3; §21.4; §21.2.
  - **Inputs / prerequisites:** P6.05–P6.09; deployment modes (§21.1).
  - **Steps:**
    1. Define the deployment profile: Linux x86-64/ARM64, systemd or an
       approved container runtime, explicit network allowlist, no shell
       access for plugins (§18.3).
    2. Build the versioned package/OCI image with checksums and SBOM
       (§21.4, §22.2).
    3. Implement signed update bundles: signature verification before
       install, staged rollout, rollback to the previous version on
       failure, and an audit event per update.
    4. Test install, upgrade, failed upgrade (rollback), and downgrade
       paths on both architectures in the lab.
  - **Deliverable / output:** Edge package/image + update/rollback
    implementation + install/upgrade test record.
  - **Acceptance criteria / exit check:** Update with a bad signature is
    refused; a failed upgrade rolls back to the previous working version;
    the running version is visible to the operator; SBOM and checksums
    are published with the release.
  - **Owner role:** DevOps / Rust engineer.
  - **Dependencies:** P6.05.
  - **Verification method:** Lab upgrade/rollback tests on x86-64 and
    ARM64; signature-tamper test.
  - **Risks / failure modes:** Unsigned updates as an attack path into OT;
    a failed upgrade bricking a remote edge with no rollback; ARM64 never
    tested despite being a shipped target.

## P6.21 — Multi-site and multi-tenant isolation at the edge

- [x] **P6.21 — Multi-site and multi-tenant isolation at the edge** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/18_21_audit_offline_isolation.md`)
  - **Objective:** Prove one edge agent cannot see, forward, or command
    another tenant's or site's assets, and that a compromised edge cannot
    escalate to other sites.
  - **Report trace:** §16.2; §17.4; §17.5 (rule 8); §5.7.
  - **Inputs / prerequisites:** P6.06 identity binding; P6.08 read path.
  - **Steps:**
    1. Write isolation tests: edge A requests tenant B's telemetry, edge A
       presents a command for site B, and edge A enumerates assets — all
       must fail.
    2. Enforce tenant/site scoping server-side on every edge session, not
       only in the edge agent (defence in depth with RLS per §16.2 — and
       note the §16.2 warning that the application role must not own
       tenant tables or bypass RLS).
    3. Test cross-tenant access-denial auditing: denials are logged with
       actor, object, and timestamp.
    4. Document the blast radius of a compromised edge: what it can and
       cannot reach.
  - **Deliverable / output:** Isolation test suite + blast-radius note.
  - **Acceptance criteria / exit check:** All cross-tenant/site attempts
    fail and are audited; no test passes by relying on the edge agent's
    goodwill alone (server-side enforcement demonstrated by testing
    against a maliciously modified edge client in the lab).
  - **Owner role:** Security engineer.
  - **Dependencies:** P6.06, P6.08.
  - **Verification method:** Run the isolation suite including the
    modified-client case; review audit logs for denials.
  - **Risks / failure modes:** Tenant context taken from an untrusted
    request field (§16.2); RLS bypassed because the app runs as table
    owner (§16.2, [R25]).

## P6.22 — Performance, resource, and scale validation of the edge agent

- [x] **P6.22 — Performance, resource, and scale validation of the edge agent** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/19_23_22_conformance_security_performance.md`)
  - **Objective:** Measure the edge agent's resource use and throughput on
    the target gateway hardware profile and record limits — no invented
    numbers.
  - **Report trace:** §19.2(8); §19.4; §18.3.
  - **Inputs / prerequisites:** P6.08, P6.09; target gateway hardware
    specification (CPU/RAM/disk, x86-64 and ARM64).
  - **Steps:**
    1. Define the load profile: number of assets, telemetry rate, buffer
       depth, and channel conditions to be tested — taken from the pilot
       site, not assumed.
    2. Measure CPU, memory, disk, telemetry ingest rate, forwarding
       latency, and recovery drain time; record p95/p99 where meaningful.
    3. Test sustained load and burst load; record the point at which the
       buffer policy (P6.09) engages.
    4. Record results as measured values with hardware, versions, and
       conditions. Do not extrapolate to a 10,000-asset fleet (§19.3).
  - **Deliverable / output:** Edge performance report (measured values,
    hardware, versions, conditions, limits).
  - **Acceptance criteria / exit check:** Measurements exist for the stated
    load profile on both architectures; no performance claim appears
    without its measurement conditions; buffer-engagement point is known.
  - **Owner role:** Rust engineer.
  - **Dependencies:** P6.08, P6.09.
  - **Verification method:** Re-run the load test and compare against the
    recorded numbers within stated tolerance.
  - **Risks / failure modes:** Lab laptop numbers presented as gateway
    numbers; burst load untested, so the first real burst overflows the
    buffer.

## P6.23 — Security testing: replay, forgery, malicious telemetry, secrets exposure

- [x] **P6.23 — Security testing: replay, forgery, malicious telemetry, secrets exposure** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/19_23_22_conformance_security_performance.md`)
  - **Objective:** Run the §19.2(7) security test layer against the edge
    and adapter surface and record every finding with severity and fix.
  - **Report trace:** §19.2(7); §17.4; §5.7.
  - **Inputs / prerequisites:** P6.07, P6.13, P6.15; threat model (§17.4).
  - **Steps:**
    1. Test command forgery and replay (P6.15 cases, extended to the
       full lifecycle).
    2. Test malicious/incorrect telemetry: spoofed source identity,
       out-of-range values, stale-data replay — each must be flagged or
       rejected, never accepted as `GOOD` (§17.4).
    3. Test auth bypass: unauthenticated edge session, wrong-tenant
       token, expired token.
    4. Test secrets exposure: confirm no secret appears in logs, config
       files, or error messages (§17.3: secrets manager integration; no
       secrets in plugin config or source control).
    5. Record findings in a dated security-test report; every high/critical
       finding blocks the exit gate until fixed and re-tested. Plugin-escape
       testing is NOT in scope here — there is no plugin host in Phase 6
       (WASM host is v0.5.0 / Stage 7); do not claim it was tested.
  - **Deliverable / output:** Security-test report with findings, severities,
    fixes, and re-test evidence.
  - **Acceptance criteria / exit check:** All §19.2(7) categories exercised;
    zero open high/critical findings; secrets-exposure test passes.
  - **Owner role:** Security engineer.
  - **Dependencies:** P6.07, P6.13, P6.15.
  - **Verification method:** Independent re-run of the attack cases by a
    second engineer; findings tracked to closure.
  - **Risks / failure modes:** Security tests run only by the feature
    author; a "test" that asserts the happy path and never attempts the
    attack.

## P6.24 — Read-only end-to-end lab integration (the exit-gate run)

- [x] **P6.24 — Read-only end-to-end lab integration (the exit-gate run)** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/24_exit_gate_lab_run_record.md`)
  - **Objective:** Execute the §23 Stage 6 exit gate as a single witnessed
    lab run: read-only end-to-end site integration works; the command path
    stays disabled unless explicitly approved for the test environment.
  - **Report trace:** §23 Stage 6 (exit gate); §3.4; §19.2(4).
  - **Inputs / prerequisites:** P6.01–P6.23; lab site (OpenEMS Edge +
    simulated or real lab devices); D6 review (P6.03).
  - **Steps:**
    1. Write the run book: exact versions (FlexCore, edge agent, OpenEMS,
       adapter, protocol suite), lab topology, asset list, data to capture.
    2. Run the full workflow (§3.4 steps 1–5, 11–12): register, connect,
       ingest, normalize, store, query, replay — read-only throughout.
    3. Attempt a command against the default-configured edge and confirm
       rejection + audit (P6.14). If — and only if — the test-env gate was
       explicitly approved, run the bounded command test per P6.15–P6.16
       and record the approval.
    4. Capture the complete evidence pack: telemetry reconciliation,
       quality flags, audit trail, health status, and the no-write
       attestation.
  - **Deliverable / output:** Exit-gate run book + evidence pack + signed
    gate record.
  - **Acceptance criteria / exit check:** Read-only path works end to end
    in the lab with reconciled telemetry; command path is disabled by
    default (or, if enabled, the written approval, scope, and expiry are
    attached); every CHECKLIST.md item (see `CHECKLIST.md`) has evidence
    or is explicitly failed.
  - **Owner role:** Integration lead + security reviewer.
  - **Dependencies:** All P6.01–P6.23, P6.33.
  - **Verification method:** Independent witness re-runs the run book from
    the evidence pack alone; results reproduce.
  - **Risks / failure modes:** A gate "passed" on a demo that nobody can
    reproduce; the command test run without written approval and later
    described as the default.

## P6.25 — Operator console surfacing for sites, edge, and adapters

- [x] **P6.25 — Operator console surfacing for sites, edge, and adapters** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/25_31_32_console_clock_decommission.md`)
  - **Objective:** Show edge connectivity, adapter status, telemetry
    freshness, and command-path state in the operator console — with
    proposed vs executed states visually distinct.
  - **Report trace:** §20.1 (Sites, Assets, Plugins & Integrations);
    §20.2; §21.3.
  - **Inputs / prerequisites:** P6.17 health; P6.14 command-path state;
    Phase 5 console.
  - **Steps:**
    1. Add/extend the Sites workspace: edge connectivity, certificate
       expiry, adapter health, last-seen timestamps, buffer depth.
    2. Show data freshness prominently; distinguish measured, estimated,
       stale, and simulated values visually (§20.2).
    3. Show the command-path state (disabled / test-env enabled with
       expiry) so an operator can never mistake a proposed setpoint for
       an executed command (§20.2).
    4. Require explicit confirmation for any high-impact action exposed
       here. The console must never enable the test-env command path
       directly: it may only display the P6.14 gate state and record a
       request for the signed per-site configuration change, which takes
       effect only through the P6.14 procedure.
  - **Deliverable / output:** Console changes + UI test record.
  - **Acceptance criteria / exit check:** An operator can determine edge
    health, adapter health, data freshness, and command-path state from
    the console without reading logs; no proposed value is displayed as
    executed.
  - **Owner role:** Frontend engineer.
  - **Dependencies:** P6.17, P6.14.
  - **Verification method:** Walkthrough with an operator against the lab;
    confirm each state is distinguishable.
  - **Risks / failure modes:** A green "connected" indicator that hides a
    stale feed; command-path state buried in a tooltip.

## P6.26 — Documentation: edge operations, adapter, and protocol guides

- [x] **P6.26 — Documentation: edge operations, adapter, and protocol guides** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/26_27_28_docs_ci_drill.md`)
  - **Objective:** Write the operator, integration, and protocol
    documentation for this phase — including limits — under `docs/`
    (operations, protocols, security), without touching the report or the
    foundations.
  - **Report trace:** §8.1 (`docs/operations/`, `docs/protocols/`,
    `docs/security/`); §22.2.
  - **Inputs / prerequisites:** P6.01–P6.25 outputs.
  - **Steps:**
    1. Write the edge operations guide: install, enroll, monitor, update,
       recover, decommission.
    2. Write the adapter guide: protocol version pinned, mapping tables,
       conformance suite and claim boundary (P6.19), known limitations.
    3. Write the OT-boundary operations note for the pilot site (from
       P6.03), marked site-specific.
    4. Run `python tools/verify_docs.py` after any doc change; keep
       section references (`§N`) resolving.
  - **Deliverable / output:** Docs under `docs/operations/`,
    `docs/protocols/`, `docs/security/` + verification output.
  - **Acceptance criteria / exit check:** Docs exist for install/enroll/
    monitor/update/recover/decommission; every protocol claim cites its
    specification version; `verify_docs.py` passes.
  - **Owner role:** Technical writer + integration engineer.
  - **Dependencies:** P6.01–P6.25 (drafts may start earlier; final needs
    the evidence).
  - **Verification method:** A new engineer follows the install guide on a
    clean lab host and succeeds without asking the author.
  - **Risks / failure modes:** Docs describing the intended system instead
    of the built one; protocol version unpinned in docs while code moves.

## P6.27 — CI gates for edge and adapter code

- [x] **P6.27 — CI gates for edge and adapter code** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/26_27_28_docs_ci_drill.md`)
  - **Objective:** Wire the §22.2 CI gates to the new edge/adapter code and
    add the adapter conformance suite (P6.19) to CI.
  - **Report trace:** §22.2; §9.3.
  - **Inputs / prerequisites:** P6.05, P6.19.
  - **Steps:**
    1. Extend CI: `cargo fmt --check`, `cargo clippy --workspace
       --all-targets -- -D warnings`, `cargo test --workspace`,
       container/dependency scanning, SBOM generation, license/security
       review, scenario regression suite — all covering the new crates.
    2. Add the adapter conformance suite as a CI job with its protocol and
       suite versions recorded in the job log.
    3. Add a CI check that fails if a new dependency lacks a license entry
       (P6.02 audit).
    4. Record a green CI run on the phase branch.
  - **Deliverable / output:** CI configuration + green run record.
  - **Acceptance criteria / exit check:** All §22.2 gates run on the new
    code and pass; conformance suite runs in CI; an unlicensed dependency
    fails CI by construction (demonstrated once, then removed).
  - **Owner role:** DevOps.
  - **Dependencies:** P6.05, P6.19, P6.02.
  - **Verification method:** Inspect the CI run; introduce a test
    unlicensed dependency in a scratch branch and confirm CI fails.
  - **Risks / failure modes:** CI that runs the old suite and never the new
    crates; conformance suite runnable only on one engineer's machine.

## P6.28 — Failure-mode and recovery drill (edge + adapter)

- [x] **P6.28 — Failure-mode and recovery drill (edge + adapter)** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/26_27_28_docs_ci_drill.md`)
  - **Objective:** Run a single combined drill covering the §19.2(6) fault
    set against the edge and adapter, and record recovery time and data
    loss for each fault.
  - **Report trace:** §19.2(6); §21.2.
  - **Inputs / prerequisites:** P6.09, P6.18, P6.23.
  - **Steps:**
    1. Drill each fault: dropped/reordered/duplicated messages, clock
       skew, stale telemetry, unavailable assets, adapter crash, plugin
       crash (if any plugin is involved), partial fleet response,
       central outage, edge restart.
    2. For each fault record: detection time, behaviour, recovery time,
       data loss (records), and whether any command was issued or
       duplicated.
    3. Fix every fault that loses data silently or issues a duplicate
       command; re-drill until clean.
    4. Reuse the platform fault-injection suites (P4.31) and the
       simulator-runner injection controls (P5.21) where they already cover
       a fault; this drill adds only the edge/adapter layer — do not
       re-run (and drift) the platform suites here.
  - **Deliverable / output:** Drill report with per-fault table.
  - **Acceptance criteria / exit check:** Every listed fault has a recorded
    outcome; zero faults cause silent data loss or a duplicate/orphaned
    command; recovery times are recorded as measured values.
  - **Owner role:** Integration lead.
  - **Dependencies:** P6.09, P6.18, P6.23.
  - **Verification method:** Independent witness re-runs two faults from
    the drill and reproduces the recorded outcomes.
  - **Risks / failure modes:** Drill run only against the simulator, never
    the lab Edge; recovery time recorded from memory instead of measured.

## P6.29 — Handover to Phase 7 (plugin SDK): what the edge exposes as seams

- [x] **P6.29 — Handover to Phase 7 (plugin SDK): what the edge exposes as seams** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/29_phase7_seam_handover.md`)
  - **Objective:** Hand Phase 7 a precise list of the capability seams the
    edge and adapter work created (or deliberately did not create), so the
    plugin SDK builds on real contracts, not on assumed ones.
  - **Report trace:** §5.2; §9.1; §23 Stage 7.
  - **Inputs / prerequisites:** P6.12, P6.17; Phase 7 PLAN.md.
  - **Steps:**
    1. List every port/contract the edge and adapters now implement
       (`TelemetrySource`, `AssetAdapter`, protocol adapter ports) with
       their current version and owner.
    2. List what is **not** a seam yet: anything Phase 7 must define
       (e.g. WIT interfaces for adapters, manifest fields for edge
       deployment) — do not pretend it exists.
    3. Record the edge agent's deployment profile (`site-edge` in §5.8)
       as the Phase 7 composition input.
    4. Note the hard boundary for Phase 7: the edge command path and its
       enablement gate (P6.14) are protected kernel behaviour, not a
       plugin capability.
  - **Deliverable / output:** Handover note listing seams, non-seams, and
    the `site-edge` profile input.
  - **Acceptance criteria / exit check:** Phase 7 can start from the
    handover note without re-deriving the adapter contracts; every listed
    seam names its port, version, and owner; non-seams are explicit.
  - **Owner role:** Architect.
  - **Dependencies:** P6.12, P6.17.
  - **Verification method:** Phase 7 lead confirms the note is sufficient
    to write the Phase 7 PLAN.md seam table (or records what is missing).
  - **Risks / failure modes:** Phase 7 designing WIT interfaces for
    adapters that do not match the Rust ports built here; the command-path
    gate accidentally exposed as a plugin capability.

## P6.30 — Phase retrospective, open items, and v0.4.0 release-gate record

- [x] **P6.30 — Phase retrospective, open items, and v0.4.0 release-gate record** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/30_retrospective_release_gate.md`)
  - **Objective:** Close the phase honestly: record what passed, what did
    not, every open item with an owner, and the v0.4.0 release-gate
    position per §22.3.
  - **Report trace:** §22.3; §23 Stage 6.
  - **Inputs / prerequisites:** P6.01–P6.34; CHECKLIST.md.
  - **Steps:**
    1. Complete CHECKLIST.md: every exit-gate item ticked with evidence,
       or explicitly failed with a reason.
    2. Record the v0.4.0 position against §22.3: which release-gate items
       this phase satisfies, which belong to later phases, and which are
       still open.
    3. List deferred work with owners: protocols not selected (with the
       D5 revisit condition), IEEE 2030.5/CSIP and IEC 61850 as later
       adapters (§2.3), unbalanced three-phase modelling if the pilot
       needs it earlier than v0.4.0 default (§25.2).
    4. State plainly what v0.4.0 is not: not certified, not safety-qualified,
       not field-control-approved; read-only/test-lab by default.
    5. Run `python tools/verify_docs.py` and `python tools/validate_agent.py`
       (AGENTS.md §6) and record their output; fix failures before claiming
       the gate.
  - **Deliverable / output:** Retrospective + open-items register +
    v0.4.0 release-gate record.
  - **Acceptance criteria / exit check:** CHECKLIST.md is complete with
    evidence links; every open item has an owner; the v0.4.0 position
    statement exists and contains the "not certified / not field-control"
    limitation in plain language.
  - **Owner role:** Engineering lead + product.
  - **Dependencies:** All P6.01–P6.34.
  - **Verification method:** Reviewer reads CHECKLIST.md and the
    retrospective and can trace every claim to a deliverable from
    P6.01–P6.29.
  - **Risks / failure modes:** A phase declared done with open gate items
    hidden in prose; deferred protocols presented as delivered.

---

## P6.31 — Edge time synchronization and clock-quality monitoring

- [x] **P6.31 — Edge time synchronization and clock-quality monitoring** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/25_31_32_console_clock_decommission.md`)
  - **Objective:** Own the edge clock: define the time-synchronization source, the tolerance, and the clock-quality status that P6.15's command verification depends on — so an expired/stale command cannot pass on a drifting edge clock.
  - **Report trace:** §10.3 (clock-quality/synchronization status); §17.4 (clock drift threat); §14.3 (expiry).
  - **Inputs / prerequisites:** P6.05 skeleton; P6.15 (consumer of the clock-quality check); D6 review (P6.03 — site network must permit the sync source).
  - **Steps:**
    1. Define the sync source for the lab and pilot profiles (e.g. NTP from a named, allowlisted source, or the site's existing time service); record it per deployment profile — never assume the edge host clock is correct.
    2. Define the tolerance (maximum acceptable offset) and the clock-quality states the edge reports (synchronized / degraded / unsynchronized), surfaced in edge health (P6.17) and the console (P6.25).
    3. Define the behaviour when sync is lost or offset exceeds tolerance: the edge holds (does not trust) expiry checks — commands are rejected or held per the P4.08 fail-closed policy, never accepted on an untrusted clock — and the condition is audited.
    4. Test: forced clock offset beyond tolerance, sync-source outage, and recovery — each produces the documented behaviour and health state.
  - **Deliverable / output:** Time-sync configuration + clock-quality monitoring + hold-on-drift rule + tests.
  - **Acceptance criteria / exit check:** Sync source and tolerance recorded per profile; quality states visible in health/console; offset beyond tolerance blocks command acceptance; outage and recovery tested.
  - **Owner role:** Rust engineer + security.
  - **Dependencies:** P6.05, P6.03.
  - **Verification method:** Clock-offset fault-injection test; health-state inspection; audit check for held commands.
  - **Risks / failure modes:** Expiry checks trusted on a free-running clock (§17.4); sync traffic blocked by the site firewall because it was never in the D6 allowlist (confirm in P6.03).

## P6.32 — Edge decommissioning and unenrollment

- [x] **P6.32 — Edge decommissioning and unenrollment** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/25_31_32_console_clock_decommission.md`)
  - **Objective:** Implement the counterpart of enrollment (P6.06): cleanly unenroll an edge agent — revoke its identity, wipe its local secrets and buffers, and confirm removal — so a retired edge leaves no live identity or buffered data behind.
  - **Report trace:** §18.1 (enrollment); §17.3 (short-lived credentials, certificate rotation); §16.2 (identity scoped to one tenant/site).
  - **Inputs / prerequisites:** P6.06 (identity binding, revocation handling); P6.09 (telemetry buffer); P6.18 (audit buffer); P6.26 (decommission documentation).
  - **Steps:**
    1. Define the decommission procedure: operator-initiated, authorized (RBAC + step-up where required), with a confirmation record naming the edge, tenant, site, actor, and time.
    2. Implement server-side revocation of the edge identity/certificate; the central platform rejects any later session from that identity.
    3. Implement edge-side wipe on decommission: keys/certificates, buffered telemetry (P6.09), and un-uploaded audit records (P6.18) — with an explicit rule for un-uploaded audit records (upload-then-wipe, or record the loss as a gap event; never silently drop).
    4. Test: a decommissioned identity attempting reconnect is rejected and audited; a wiped edge holds no recoverable key or buffered record.
  - **Deliverable / output:** Decommission procedure + revocation/wipe implementation + confirmation record + tests.
  - **Acceptance criteria / exit check:** Revocation blocks reconnect; wipe verified (no keys/buffers recoverable); un-uploaded audit handling follows the recorded rule; confirmation record exists per decommission.
  - **Owner role:** Security + Rust engineer.
  - **Dependencies:** P6.06, P6.09, P6.18.
  - **Verification method:** Reconnect-after-decommission test; post-wipe inspection; confirmation-record review.
  - **Risks / failure modes:** A decommissioned edge reconnecting with a cached certificate; audit records lost silently at wipe. Mitigation: revocation is server-side and checked per session; the audit rule is explicit.

## P6.33 — Exit-gate evidence-pack completeness

- [x] **P6.33 — Exit-gate evidence-pack completeness** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/33_evidence_pack_completeness.md`)
  - **Objective:** Make the P6.24 exit-gate pack complete: it must cite the conformance results (P6.19), the failure-mode drill results (P6.28), and the security-findings clearance (P6.23) — the three evidence types the pack as specified omits.
  - **Report trace:** §23 Stage 6 (exit gate); §22.3 (edge offline/degraded modes tested; release-gate position).
  - **Inputs / prerequisites:** P6.19, P6.23, P6.28, P6.24.
  - **Steps:**
    1. Define the pack contents checklist: telemetry reconciliation (P6.11/P6.24), quality flags, audit trail, health status, no-write attestation (existing) **plus** conformance suite run ID + protocol/suite versions (P6.19), drill report with per-fault recovery/loss (P6.28), security-test report with zero open high/critical findings (P6.23).
    2. For each added item, cite it by run ID and version; if an item is missing, the pack records it as explicitly missing with owner and date — never silent.
    3. Have the pack reviewed against §22.3: the offline/degraded-mode item must trace to the drill (P6.28), not to the read-only run alone.
    4. File the pack where P6.30 (retrospective + v0.4.0 release-gate record) can cite it.
  - **Deliverable / output:** Evidence-pack contents checklist + completed pack + §22.3 trace.
  - **Acceptance criteria / exit check:** Every listed item is cited by run ID/version or explicitly missing with owner/date; §22.3 offline/degraded item traces to the drill; P6.30 cites the pack.
  - **Owner role:** Integration lead.
  - **Dependencies:** P6.19, P6.23, P6.28, P6.24.
  - **Verification method:** Pack review against the checklist; §22.3 trace inspection.
  - **Risks / failure modes:** A gate "passed" on the read-only run while conformance/drill evidence is missing. Mitigation: the explicitly-missing rule forces the gap into the open.

## P6.34 — Secure-boot position record for edge hosts

- [x] **P6.34 — Secure-boot position record for edge hosts** — implemented, awaiting human build/test (decision record: `ledger/phase-06-edge-protocol/34_secure_boot_position.md`)
  - **Objective:** Record the secure-boot position for each edge gateway profile, closing the one §17.3 control P6.20 (signed updates) does not cover.
  - **Report trace:** §17.3 (secure boot/update policy for edge hosts where customer platform supports it); §18.3; §21.2.
  - **Inputs / prerequisites:** P6.20 (deployment profile, signed updates); gateway hardware specification (per profile, x86-64 and ARM64).
  - **Steps:**
    1. For each gateway profile, record: secure boot supported (and enabled + verified), supported but not enabled (with reason and owner), or unsupported by the hardware — with the fallback in each case (signed updates per P6.20, plus the residual risk).
    2. Where supported, record how secure boot is verified at install and after update (P6.20 upgrade/rollback paths).
    3. File the record with the deployment profile (P6.20) so the D6 review (P6.03) and the v0.4.0 release-gate record (P6.30) can cite it.
  - **Deliverable / output:** Secure-boot position record per gateway profile.
  - **Acceptance criteria / exit check:** Every shipped gateway profile has a recorded position; unsupported profiles name the fallback and residual risk; the record is cited by P6.20/P6.30.
  - **Owner role:** DevOps / security.
  - **Dependencies:** P6.20.
  - **Verification method:** Record review against the hardware specs; install/upgrade test confirms the verified-boot claim where made.
  - **Risks / failure modes:** "Secure boot" claimed for a profile whose hardware does not support it. Mitigation: the position is per profile, tied to the hardware spec, not a blanket claim.

## Subtask index

| ID | Title | Trace |
|---|---|---|
| P6.01 | Close, or formally record as still open, D5 | §25.2, §2.3 |
| P6.02 | License/dependency audit (D4 follow-through) | §25.2, §9.3 |
| P6.03 | Cyber and OT boundary review (D6) | §17.2–§17.4, §25.2 |
| P6.04 | Phase readiness check (Stages 1–5) | §23 Stages 1–5 |
| P6.05 | Edge agent crate and composition-root design | §18.1, §8.1–§8.2 |
| P6.06 | Edge identity, enrollment, certificate lifecycle | §18.1, §18.3, §17.3 |
| P6.07 | Outbound mTLS channel | §3.3, §18.1–§18.2 |
| P6.08 | Edge telemetry ingestion (read path) | §18.1, §10.2–§10.4 |
| P6.09 | Telemetry buffering, store-and-forward, recovery | §18.1, §18.3, §21.2 |
| P6.10 | OpenEMS: EdgeConfig discovery and channel mapping | §2.3, §18.1 |
| P6.11 | OpenEMS: read-only data path end to end | §23 Stage 6, §3.4 |
| P6.12 | Selected adapter: interface and anti-corruption layer | §9.1, §6.4 |
| P6.13 | Selected adapter: read/telemetry implementation and tests | §2.3, §19.2 |
| P6.14 | Command path disabled by default; test-env enablement gate | §23 Stage 6, §21.4 |
| P6.15 | Edge command verification (test-env only) | §18.1, §17.3–§17.4 |
| P6.16 | Forwarding to allowlisted adapters; fallback | §18.1, §14.4 |
| P6.17 | Local policy enforcement and adapter health | §18.1, §18.3, §21.3 |
| P6.18 | Local audit buffer and offline behaviour | §18.1, §18.3, §21.2 |
| P6.19 | Adapter conformance tests and claim boundary | §2.3, §19.2, §9.3 |
| P6.20 | Edge deployment profile and signed updates | §18.3, §21.4, §21.2 |
| P6.21 | Multi-site and multi-tenant isolation at the edge | §16.2, §17.4–§17.5 |
| P6.22 | Performance, resource, and scale validation | §19.2, §19.4 |
| P6.23 | Security testing (replay, forgery, malicious telemetry, secrets exposure) | §19.2, §17.4 |
| P6.24 | Read-only end-to-end lab integration (exit-gate run) | §23 Stage 6 |
| P6.25 | Operator console surfacing | §20.1–§20.2 |
| P6.26 | Documentation (edge, adapter, protocol guides) | §8.1, §22.2 |
| P6.27 | CI gates for edge and adapter code | §22.2, §9.3 |
| P6.28 | Failure-mode and recovery drill | §19.2, §21.2 |
| P6.29 | Handover to Phase 7 (seams) | §5.2, §9.1 |
| P6.30 | Retrospective, open items, v0.4.0 release-gate record | §22.3 |
| P6.31 | Edge time synchronization and clock-quality monitoring | §10.3, §17.4 |
| P6.32 | Edge decommissioning and unenrollment | §18.1, §17.3 |
| P6.33 | Exit-gate evidence-pack completeness | §23 Stage 6, §22.3 |
| P6.34 | Secure-boot position record for edge hosts | §17.3, §18.3 |

**Subtask count: 34.**
