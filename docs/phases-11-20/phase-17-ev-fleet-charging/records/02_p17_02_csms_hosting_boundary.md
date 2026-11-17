# P17.02 — CSMS hosting boundary: both shapes designed, the edge-agent-hosted depot DEFAULT implemented — and P17-E-2 registered OPEN

| Field | Value |
|---|---|
| Subtask | P17.02 (CSMS hosting boundary decision + design: edge-agent-hosted vs control-plane-hosted; DEFAULT edge-agent-hosted for depot sites — offline tolerance, store-and-forward buffer reuse; both shapes designed, the default implemented per conventions §4) |
| Wave / agent | Phase 17 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the two hosting shapes are declared data (`src/hosting.rs`), both designed; the DEFAULT is implemented as the depot placement constructor; no process runs anywhere — the transport seam (record 01 §7) is UNRUN in BOTH shapes |
| Sources | [S-121] (OCA OCPP protocols — the server role), [S-126][S-127] (security posture per hosting shape), [S-133] (Argonne EVSE security report — the depot attack-surface note); phase-17 context §3 constraint 2 |
| Files touched | `crates/adapters/protocol-ocpp/src/hosting.rs` (NEW); `crates/adapters/protocol-ocpp/src/csms.rs` (the security requirements the shapes bind; from record 01); `tests/adapter_surface.rs` (the hosting tests; from record 01's batch) |

## 1. The two shapes, both designed

| Shape | Design | Recorded cost |
|---|---|---|
| **Edge-agent-hosted** (P17-E-2 DEFAULT for depot sites) | The CSMS endpoint runs inside the edge agent at the depot. Backhaul loss does not sever the charge-point↔CSMS link. The edge agent's store-and-forward buffer (P6.09) and mTLS uplink are REUSED, not rebuilt; telemetry and relayed lifecycle output travel edge-local, records go up the existing uplink. The store-and-forward flag is STRUCTURAL: the shape is refused without buffer reuse (`HostingError::EdgeShapeRequiresStoreAndForward`) — a depot CSMS without offline tolerance would contradict the shape's own recorded rationale. | The edge deployment carries the depot attack surface; certificate custody and security-event handling land at the edge (Profile 3 onboarding, record 03, is required in either shape) |
| **Control-plane-hosted** | The CSMS endpoint runs in the control plane; charge points connect directly to the platform. Fits sites with reliable backhaul and no edge agent deployment. | A backhaul outage SEVERS the CSMS link; the domain link-loss rule (record 04 — connection loss is NOT asset unavailability) governs the recorded state, and no command can be relayed while the link is down. The shape REQUIRES a recorded reason (`HostingError::ControlPlaneShapeRequiresReason`) — the cost is named, not hidden. |

Both shapes bind the SAME fail-closed security requirements
(`CsmsSecurityRequirements`: Profile-3-onward onboarding, non-empty
trust-anchor labels) and the SAME adapter scope — the hosting boundary
changes WHERE the server runs, never WHAT it is permitted to do.

## 2. P17-E-2 — CSMS hosting boundary (registered OPEN — agents never close E-decisions, conventions §4)

- **Question:** does the CSMS connection live in the control plane or in
  the edge agent?
- **Options:** (a) edge-agent-hosted for depot sites (offline tolerance;
  store-and-forward buffer reuse), control-plane-hosted elsewhere;
  (b) control-plane-hosted only; (c) fully per-site configurable.
- **DEFAULT (RECOMMENDATION):** (a) — depot sites must tolerate backhaul
  loss, and the edge agent's offline buffer and mTLS uplink already exist.
- **Implemented per conventions §4:** `CsmsPlacement::depot_default(site)`
  constructs the (a) shape for a named depot site; option (c)'s
  per-site choice is expressible by constructing the placement per site —
  but no per-site registry is built in this batch (a composition-root
  concern once sites exist).
- **What closes it:** the human owner naming the first depot site and its
  connectivity profile. **P17-E-2 stays OPEN.**

## 3. Honest boundaries

1. The shapes are DECLARED DATA (`CsmsHostingShape`, `CsmsPlacement`), not
   deployment artifacts: no process placement, no transport, no site named,
   no hardware claim.
2. The capability summary is honest per shape: edge-agent-hosted reports
   backhaul-outage tolerance; control-plane-hosted reports that a backhaul
   outage severs the link.
3. The DEFAULT statement travels with every placement
   (`CsmsHostingShape::DEFAULT_STATEMENT`) and names the E-decision as
   OPEN with the human closer.
4. No site, fleet, or customer claim of any kind.
