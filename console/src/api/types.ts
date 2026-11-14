// P5.01 — API-layer types: hand-written mirrors of the server DTOs.
//
// SHAPE-MIRROR NOTE (read before editing): these types mirror
// apps/control-plane/src/{dto.rs,dto_phase3.rs,dto_phase4.rs} field-for-
// field (snake_case on the wire — the mirrors use the WIRE names, not
// camelCase, so a payload mismatch fails visibly instead of silently).
// The real shapes are pinned by the apps/control-plane contract tests
// (tests/api*.rs) and the generated OpenAPI. The P5.01 step-4 drift test
// (generated client vs live OpenAPI) lands with the first install; until
// then these mirrors are the recorded stand-in — treat a server DTO edit
// as a REQUIRED edit here too.

/** Single-record envelope (dto.rs DataEnvelope). */
export interface DataEnvelope<T> {
  data: T;
}

/** List envelope (dto.rs ListEnvelope). */
export interface ListEnvelope<T> {
  data: T[];
}

/** Keyset page envelope (the recorded P2.12/P3.28/P4.30 convention). */
export interface PageEnvelope<T> {
  data: T[];
  next_cursor: string | null;
}

// --- Phase 1 registry (dto.rs) ----------------------------------------------

export interface TenantDto {
  id: string;
  name: string;
  description?: string;
  identity_provider_reference: string;
  policy_profile: string;
  created_at: string;
}

export interface PortfolioDto {
  id: string;
  tenant_id: string;
  name: string;
  description?: string;
  operator: string;
  market_region: string;
  aggregation_rules: string;
  created_at: string;
}

export interface SiteDto {
  id: string;
  portfolio_id: string;
  tenant_id: string;
  name: string;
  description?: string;
  location: string;
  timezone: string;
  point_of_common_coupling: string;
  grid_model_reference: string;
  edge_identity: string;
  created_at: string;
}

export interface AssetDto {
  id: string;
  site_id: string;
  tenant_id: string;
  name: string;
  description?: string;
  asset_type: string;
  vendor_model: string;
  /** Canonical MW (P0.07); P > 0 means export at the point of connection. */
  rated_power_mw: number;
  /** Canonical MWh (storage assets). */
  rated_energy_mwh?: number;
  phase: string;
  bus_reference: string;
  capability_profile: string;
  lifecycle_status: 'active' | 'deactivated';
  deactivation_reason?: string;
  deactivated_at?: string;
  created_at: string;
}

// --- Phase 2 telemetry (dto.rs) ----------------------------------------------

/** The frozen six-state quality vocabulary (§10.4; exactly one per value). */
export type TelemetryQuality =
  | 'GOOD'
  | 'SUSPECT'
  | 'BAD'
  | 'MISSING'
  | 'ESTIMATED'
  | 'STALE';

export interface TelemetryRecordDto {
  asset_id: string;
  quantity: string;
  value_num?: number;
  value_bool?: boolean;
  value_text?: string;
  unit: string;
  quality: TelemetryQuality;
  q_sign_convention?: string;
  event_time: string;
  ingest_time: string;
  source_id: string;
  source_sequence: number;
  /** Always true on the Phase 2 surface: simulator-derived telemetry. */
  simulated: boolean;
}

export interface QuantityStateDto {
  quantity: string;
  value_num?: number;
  value_bool?: boolean;
  value_text?: string;
  unit: string;
  quality: TelemetryQuality;
  q_sign_convention?: string;
  event_time: string;
  ingest_time: string;
  age_ms: number;
  simulated: boolean;
}

export interface CurrentStateDto {
  asset_id: string;
  generated_at: string;
  quantities: QuantityStateDto[];
  simulated: boolean;
}

// --- Phase 3 records (dto_phase3.rs) ------------------------------------------

export interface EnvelopeRecordDto {
  envelope_id: string;
  tenant_id: string;
  portfolio_id: string;
  envelope_version: string;
  horizon_start_millis: number;
  horizon_end_millis: number;
  interval_millis: number;
  interval_count: number;
  covered_site_ids: string[];
  envelope_payload: unknown;
  event_payload_gap_note: string;
}

export interface ScheduleRevisionDto {
  schedule_id: string;
  revision: number;
  tenant_id: string;
  portfolio_id: string;
  site_id: string;
  horizon_start_millis: number;
  horizon_end_millis: number;
  roll_trigger: string;
  /** The §12.6 state (CANDIDATE/…; the closed vocabulary lives server-side). */
  state: string;
  supersedes_revision?: number;
  supersedes_candidate_id?: string;
  superseded_by_revision?: number;
  superseded_by_candidate_id?: string;
  candidate_id: string;
  candidate_status: string;
  candidate_status_statement: string;
  validation_scope: string;
  validation_scope_statement: string;
  schedule_payload: unknown;
}

export interface OptimizationRunDto {
  candidate_id: string;
  tenant_id: string;
  portfolio_id: string;
  site_id: string;
  status: string;
  problem_class: string;
  /** NEVER conflate solver classes (AGENTS.md rule 4): the name/version are
   * recorded facts from the §12.6 record, not capability claims. */
  solver_name: string;
  solver_version: string;
  provider_id: string;
  provider_version: string;
  objective_value?: number;
  objective_units?: string;
  input_snapshot_id: string;
  grid_model_version: string;
  grid_model_site_id: string;
  grid_model_site_reference: string;
  schedule_proposed_key: string;
  candidate_status: string;
  candidate_status_statement: string;
  record: unknown;
}

// --- Phase 4 records (dto_phase4.rs) ------------------------------------------

export interface DispatchCommandDto {
  command_id: string;
  tenant_id: string;
  portfolio_id: string;
  site_id: string;
  asset_id: string;
  schedule_id: string;
  schedule_revision: number;
  valid_until_millis: number;
  supersedes_command_id?: string;
  superseded_by_command_id?: string;
  /** The §14.1 lifecycle state (PROPOSED/…; closed vocabulary server-side). */
  state: string;
  command_payload: unknown;
  no_execute_endpoint_statement: string;
  simulation_only_statement: string;
}

export interface CommandTransitionDto {
  seq: number;
  from_state: string;
  to_state: string;
  actor: string;
  at_unix_millis: number;
  gate?: string;
  reason?: string;
  evidence_reference?: string;
}

export interface DispatchCommandDetailDto {
  command: DispatchCommandDto;
  journal: CommandTransitionDto[];
}

export interface FeasibilityCheckDto {
  check_id: string;
  tenant_id: string;
  command_id?: string;
  portfolio_id: string;
  site_id: string;
  asset_id: string;
  schedule_id: string;
  schedule_revision: number;
  model_id?: string;
  model_version?: string;
  model_content_hash?: string;
  outcome: string;
  failing_constraint?: string;
  reason?: string;
  margins?: string;
  coverage?: string;
  actor: string;
  correlation_id: string;
  checked_at_unix_millis: number;
  no_execute_endpoint_statement: string;
}

export interface AuditEventDto {
  audit_id: string;
  tenant_id?: string;
  actor: string;
  action: string;
  object_type: string;
  object_id: string;
  before_ref?: string;
  after_ref?: string;
  decision_outcome?: string;
  decision_reason_reference?: string;
  evidence?: string;
  event_time_unix_millis: number;
  ingest_time_unix_millis: number;
  correlation_id: string;
  corrected_audit_id?: string;
  append_only_statement: string;
}
