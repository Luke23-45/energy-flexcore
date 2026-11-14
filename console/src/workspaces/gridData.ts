// P5.04 — Grid workspace data mapping (pure functions, no I/O).
//
// Source contract: the persisted §12.5 feasibility-check records
// (dto_phase4.rs FeasibilityCheckDto — the P4.30 read surface). The
// recorded verdict vocabulary on THIS resource is the persisted three:
// `PASSED` | `LIMIT_VIOLATED` | `FAIL_CLOSED` (feasibility_check.rs
// FEASIBILITY_CHECK_OUTCOMES) — NOT the internal six-way routing of
// verdict.rs (REJECTED/CONSTRAINED/…), which the gate renders before
// persistence; the console must not re-derive routing outcomes the record
// does not carry.
//
// Honesty rules implemented here (each tested in Grid.test.tsx):
// - model version + content hash + completion timestamp travel with EVERY
//   outcome row (§20.2: model version and timestamp on every grid
//   feasibility outcome); a record without them renders the explicit
//   "not present in the record" marker — never a plausible version.
// - A PASSED record that carries a coverage statement is rendered as
//   coverage-LIMITED: the coverage text travels on the record (§12.5 step
//   6), and parts the model does not cover are labelled uncovered — never
//   shown as passing (the ledger risk row). The console does not parse
//   semantics out of the coverage string; it displays it verbatim in a
//   distinct block and qualifies the outcome label whenever one is
//   present.
// - Margins (§12.5 step 7) are displayed verbatim in their own block; no
//   margin structure is invented beyond what the record carries.
// - FAIL_CLOSED renders the reason plus the fail-closed rule (§12.5 final
//   paragraph, §17.5 invariant 9): feasibility is NOT inferred.
// - NO feasibility-guarantee language: a PASSED row is an evidence-backed
//   outcome against the DECLARED model within stated tolerances — never a
//   field-safety, certification, or approval claim (verdict.rs).
import type { FeasibilityCheckDto } from '../api/types';

export const OUTCOME_NOTE_PASSED =
  'PASSED means an evidence-backed outcome against the DECLARED model and ' +
  'limits within stated tolerances (§12.5) — NOT a field-safety, ' +
  'certification, or approval claim (crates/flexcore-grid-api/src/' +
  'verdict.rs; authorization is a distinct gate, §14.1).';

export const OUTCOME_NOTE_FAIL_CLOSED =
  'FAIL_CLOSED: feasibility is NOT inferred (§12.5 final paragraph; §17.5 ' +
  'invariant 9). Each trigger maps to a recorded REJECT or HOLD per the ' +
  'P4.08 policy table; no trigger yields a pass (fail_closed.rs).';

export const OUTCOME_NOTE_LIMIT_VIOLATED =
  'LIMIT_VIOLATED: the candidate exceeded a declared network limit; the ' +
  'failing constraint is named on the record (§12.5 step 5, P4.06).';

export const COVERAGE_QUALIFIER =
  'This record carries a coverage statement (§12.5 step 6): the parts of ' +
  'the network the model does not cover are UNCOVERED — they are NOT ' +
  'validated by this outcome and are never shown as passing. The coverage ' +
  'text is the record\'s own statement, displayed verbatim below.';

/** The qualified display label for a persisted outcome. */
export function outcomeLabel(check: { outcome: string; coverage?: string | null }): string {
  const base = check.outcome;
  if (check.coverage !== undefined && check.coverage !== null && check.coverage.length > 0 && check.outcome === 'PASSED') {
    return `${base} — coverage-limited (uncovered parts are NOT validated)`;
  }
  return base;
}

export interface GridCheckRow {
  check_id: string;
  site_id: string;
  schedule_id: string;
  schedule_revision: number;
  command_id: string | null;
  outcome: string;
  model_id: string | null;
  model_version: string | null;
  model_content_hash: string | null;
  failing_constraint: string | null;
  reason: string | null;
  margins: string | null;
  coverage: string | null;
  actor: string;
  correlation_id: string;
  checked_at_unix_millis: number;
}

/** Maps a record onto the display row (nulls stay null — never defaulted). */
export function toGridCheckRow(check: FeasibilityCheckDto): GridCheckRow {
  return {
    check_id: check.check_id,
    site_id: check.site_id,
    schedule_id: check.schedule_id,
    schedule_revision: check.schedule_revision,
    command_id: check.command_id ?? null,
    outcome: check.outcome,
    model_id: check.model_id ?? null,
    model_version: check.model_version ?? null,
    model_content_hash: check.model_content_hash ?? null,
    failing_constraint: check.failing_constraint ?? null,
    reason: check.reason ?? null,
    margins: check.margins ?? null,
    coverage: check.coverage ?? null,
    actor: check.actor,
    correlation_id: check.correlation_id,
    checked_at_unix_millis: check.checked_at_unix_millis,
  };
}

export interface SiteCheckGroup {
  site_id: string;
  rows: GridCheckRow[];
}

/** Groups the rows by site (the closest the record surface comes to
 * feeder/substation scoping — see the boundaries note in Grid.tsx). */
export function groupBySite(rows: GridCheckRow[]): SiteCheckGroup[] {
  const order: string[] = [];
  const groups = new Map<string, GridCheckRow[]>();
  for (const row of rows) {
    const existing = groups.get(row.site_id);
    if (existing) {
      existing.push(row);
    } else {
      groups.set(row.site_id, [row]);
      order.push(row.site_id);
    }
  }
  return order.map((site_id) => ({ site_id, rows: groups.get(site_id) as GridCheckRow[] }));
}

export interface OutcomeCounts {
  PASSED: number;
  LIMIT_VIOLATED: number;
  FAIL_CLOSED: number;
  other: number;
}

/** Counts per persisted outcome; unknown codes never masquerade as a pass. */
export function countOutcomes(rows: GridCheckRow[]): OutcomeCounts {
  const counts: OutcomeCounts = { PASSED: 0, LIMIT_VIOLATED: 0, FAIL_CLOSED: 0, other: 0 };
  for (const row of rows) {
    if (row.outcome === 'PASSED' || row.outcome === 'LIMIT_VIOLATED' || row.outcome === 'FAIL_CLOSED') {
      counts[row.outcome] += 1;
    } else {
      counts.other += 1;
    }
  }
  return counts;
}
