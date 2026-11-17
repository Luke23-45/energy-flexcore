# P14.23 — Operator documentation per adapter (written docs, never exercised)

| Field | Value |
|---|---|
| Subtask | P14.23 — deployment, security, and operations documentation per protocol adapter (enrollment, credentials, budgets, failure modes) — WRITTEN DOCS, no runbook claimed exercised |
| Wave / agent | Phase 14 wave 3, Agent B (P14.19, P14.22–P14.24) |
| Status | **WRITTEN DOCUMENTATION ONLY.** Every document states the written state of its adapter (WRITTEN-UNRUN, or DESIGN-RECORD-ONLY for the two design-status protocols); no document describes observed behavior; no runbook was exercised, no deployment was performed, no adapter was operated |
| Conformance boundary | Every document carries the zero-conformance-claim boundary (conventions §7.3) and points to the human-gated certification evidence plan (record 101) |
| Sources | The six adapter records (02+01+03+04, 21+22, 23, 24, 43, 44), the two EU design records (41, 42), the common port layer record 01, and the existing docs-tree conventions (`docs/protocols/openems-adapter.md` — the shape this tree follows) |
| Files touched | `docs/protocols/ieee-2030.5-adapter.md`, `docs/protocols/openadr-adapter.md`, `docs/protocols/sunspec-modbus-adapter.md`, `docs/protocols/dnp3-adapter.md`, `docs/protocols/mqtt-sparkplug-adapter.md`, `docs/protocols/cim-cgmes-import.md`, `docs/protocols/iec-60870-5-104-design-status.md`, `docs/protocols/iec-61850-7-420-design-status.md`, this record. No existing doc was edited — `openems-adapter.md` is untouched |

## 1. Where the docs live and why

The existing docs tree already carries protocol-facing operator guidance
in `docs/protocols/` (`openems-adapter.md` — the P6 adapter guide), so
the phase-14 operator docs extend THAT tree rather than starting a
parallel `docs/operations/` fork (the existing `docs/operations/` tree
holds the deployment/edge/pilot runbooks — a different document class,
untouched). Each new document follows the `openems-adapter.md` shape:
a field table (scope, owner record, implementation status, conformance
status, control status) followed by numbered sections covering what the
adapter is, deployment configuration, security, operations, and the
honest boundary.

## 2. The document inventory (one per adapter, eight documents)

| Document | Adapter / surface | Covers | Status stated in the doc |
|---|---|---|---|
| `docs/protocols/ieee-2030.5-adapter.md` | `protocol-20305` (AggregatorServer) | CSIP resource tree, declared `AddressPlan`/`ServerLimits`/DNS-SD, operator-mediated enrollment, mTLS/trust-anchors/SFDI-LFDI (record 03), caps/paging/failure modes, the transport seam | WRITTEN-UNRUN |
| `docs/protocols/openadr-adapter.md` | `protocol-openadr` (VTN) | Endpoint design data, operator-mediated VEN acceptance, delivery modes, event → PROPOSED mapping, unit handling, 2.0b deferral (P14-E-2) | WRITTEN-UNRUN |
| `docs/protocols/sunspec-modbus-adapter.md` | `protocol-modbus` (client) | TCP/RTU declared transport (no defaults), per-model register maps (lab-verified before registration), polling budgets, reactive-sign convention, read-only structure | WRITTEN-UNRUN |
| `docs/protocols/dnp3-adapter.md` | `protocol-dnp3` (outstation) | Outstation role, utility-authoritative point list, declared scaling, control-arrival rule, secure-transport posture with mandatory open questions, master-role deferral | WRITTEN-UNRUN |
| `docs/protocols/mqtt-sparkplug-adapter.md` | `protocol-mqtt` (uplink) | Topic namespace, BIRTH/DEATH trust rules and refusal names, per-device credential bindings (no secret field), TLS-required broker config, publish-only structure, the NATS note [S-40] | WRITTEN-UNRUN |
| `docs/protocols/cim-cgmes-import.md` | `protocol-cim` (import) | RDF seam, the four-status classification, the blocking completeness gate, DER-group registry, untrusted-document posture | WRITTEN-UNRUN |
| `docs/protocols/iec-60870-5-104-design-status.md` | IEC 104 surface | DESIGN-STATUS: the designed client surface, the no-native-encryption compensating-controls design, the NOT-exists inventory, the P14-E-3 start condition | **DESIGN RECORD ONLY — no adapter exists** |
| `docs/protocols/iec-61850-7-420-design-status.md` | IEC 61850-7-420 surface | DESIGN-STATUS: MMS-client-only design, SCL/ICD ingestion design, the GOOSE/SV out-of-scope boundary, the NOT-exists inventory, the P14-E-3 start condition | **DESIGN RECORD ONLY — no adapter exists** |

## 3. The common template sections (what "per adapter" means here)

Every adapter document (not the design-status pair) covers the same five
operator concerns, each stated as DESIGN, never as observed behavior:

| Section | Content | Honesty rule applied |
|---|---|---|
| What the adapter is (and is not) | role, surface, direction, authority — including the structural absences (no write path / publish-only / no listener) | structural absence is stated as structure, not policy |
| Deployment | every configuration input the deployment declares | no defaults in production; every UNVERIFIED convention labeled |
| Security | enrollment, credentials, secrets posture | secret REFERENCES only (`secret.read` never granted); no security claim |
| Operations | budgets, cadence, observability vocabulary, failure modes | refusal names travel verbatim from the owner records; the P11 taxonomy is consumed, never redefined |
| Honest boundary (+ certification pointer) | what does not exist; the zero-claim boundary; the pointer to the evidence plan | conventions §7.3; records 101 |

## 4. What the docs deliberately do NOT do

1. **No runbook is claimed exercised.** These are written guides; the
   existing `docs/operations/` runbooks (deployment, edge, pilot
   handover) are separate documents and none was run or updated.
2. **No deployment values are invented.** Where a record ships no
   production number (the 2030.5 collection caps, poll budgets, QoS),
   the doc says the in-code values are registered recommendations /
   test values pending the X-8 pre-registration discipline — it does
   not transcribe new numbers.
3. **No credential material, endpoint, host, or port is fabricated** —
   every connection value is described as DECLARED-per-deployment.
4. **No design-status doc blurs into an adapter guide.** The 104/61850
   documents lead with "no adapter exists" and carry a NOT-exists
   inventory table.

## 5. Honest boundaries

No adapter was deployed, operated, enrolled, connected, or exercised;
no operator performed any procedure in these documents; every statement
about adapter behavior is a statement about WRITTEN-UNRUN code traced to
its owner record. Every external/protocol detail carries the owner
record's UNVERIFIED labels. No conformance, certification, security, or
compliance claim appears anywhere in the tree (conventions §7.3).

## 6. Handoff notes

1. **The orchestrator's ledger tick** for P14.23 cites this record
   (`records/103_p14_23_operator_documentation.md`).
2. **The close audit (P14.24, record 104)** includes these eight
   documents in the pipe-count, [S-n], and honesty sweeps.
3. **Future adapter batches** (e.g. post-P14-E-3 104/61850 crates):
   update the corresponding doc's status table (design-status → adapter
   guide) in the same batch as the crate — never silently.
