// P5.22 — the scenario-run record client (SHAPE-MIRROR of the
// flexcore-sim-runner run record: apps/sim-runner/src/scenario_run.rs
// `ScenarioRunRecord` + apps/sim-runner/src/fault_injection.rs
// `FaultJournal`/`FaultJournalEntry`; drift note: these TypeScript types
// mirror the Rust serde field-for-field in the WIRE names, so a payload
// mismatch fails visibly instead of silently).
//
// Honest contract carried by this module (rendered by the workspace,
// never paraphrased):
// - The run-record SERVING API does not exist yet in this phase: the
//   ScenarioRunRecord is produced by the sim-runner CLI as a JSON
//   artefact, and no control-plane route serves it (the gap is recorded
//   in the decision record 21_24_faults_visibility.md and owned by the
//   run-history surface, P5.08/P5.30). The path below is the DISPLAY
//   CONTRACT: a 404/error renders the honest error state, and no run is
//   ever fabricated by the console.
// - Every run record this workspace displays is SIMULATED (the entire 0.x
//   line, §21.4) and labels itself with the scenario id + version it came
//   from.
// - The fault journal is the §19.2 layer 6 visibility contract: fault id,
//   injection point, observed effect, platform response + evidence ref —
//   a fault whose effect the run did not observe carries
//   `not_observed_in_this_run`, never "passed".
import { api, type ApiClient } from './client';

/** The run-record read path (the display contract; no server route yet). */
export const SCENARIO_RUNS_PATH = '/api/v1/scenarios/runs';

/** The closed platform-response vocabulary of the fault journal. */
export const FAULT_PLATFORM_RESPONSES = [
  'reject',
  'hold',
  'fallback',
  'failed',
  'per_asset',
  'not_observed_in_this_run',
  'cross_referenced_to_phase_4_suite',
] as const;
export type FaultPlatformResponse = (typeof FAULT_PLATFORM_RESPONSES)[number];

/** One fault-journal entry (fault_injection.rs FaultJournalEntry, wire names). */
export interface FaultJournalEntryDto {
  fault_id: string;
  /** The closed fault-kind vocabulary (ScenarioFaultKind::name). */
  kind: string;
  injection_point: string;
  at_unix_millis: number;
  observed_effect: string;
  platform_response: string;
  evidence_reference: string;
  pre_registered_reference: string;
}

/** The fault journal envelope (fault_injection.rs FaultJournal, wire names). */
export interface FaultJournalDto {
  entries: FaultJournalEntryDto[];
  observed_count: number;
  cross_referenced_count: number;
  /** The verbatim statement (rendered, never paraphrased). */
  statement: string;
}

/** One dispatch-command outcome (scenario_run.rs CommandRunSummary, wire names). */
export interface CommandRunSummaryDto {
  command_id: string;
  asset_key: string;
  final_state: string;
  issue_outcome: string;
  acceptance_reference?: string | null;
  tracking_verdict?: string | null;
}

/** One grid feasibility check outcome (scenario_run.rs GridCheckSummary). */
export interface GridCheckSummaryDto {
  attempt_id: string;
  backend: string;
  provider_id_version: string;
  verdict: string;
  reason?: string | null;
}

/**
 * The run-record summary the workspace displays (the display-relevant
 * fields of scenario_run.rs ScenarioRunRecord; additive fields are
 * tolerated by the mirror and render only what exists).
 */
export interface ScenarioRunRecordSummaryDto {
  scenario_id: string;
  scenario_version: string;
  master_seed: number;
  code_version: string;
  simulator_source_id: string;
  records_ingested: number;
  commands: CommandRunSummaryDto[];
  grid_checks: GridCheckSummaryDto[];
  /** Present when the run's scenario configured any fault (P5.21). */
  fault_journal?: FaultJournalDto;
  statements?: string[];
}

/** The read response (a JSON array of run records, newest last). */
export interface ScenarioRunsEnvelope {
  data: ScenarioRunRecordSummaryDto[];
}

/**
 * Lists the tenant's scenario-run records (GET the display-contract path).
 * In this phase the server has no such route: the read fails honestly and
 * the workspace renders the error/gap state — the console never invents
 * runs (see the module header).
 */
export function listScenarioRuns(client: ApiClient = api): Promise<ScenarioRunsEnvelope> {
  return client.get(SCENARIO_RUNS_PATH);
}
