// P5.04 — the Grid workspace (report §20.1 item 5), on the Phase 4
// feasibility-check records (P4.30 read surface, dto_phase4.rs) and the
// P4.03 model-health vocabulary.
//
// Elements per §20.1 item 5 / ledger P5.04:
// - topology/model version: version + content hash per check record
//   (§20.2: model version AND timestamp on EVERY outcome — the row renders
//   both even when the record omits them, as explicit "not present in the
//   record" markers),
// - feeder/substation boundaries: the frozen §15.2 candidate path
//   /api/v1/grid-models is NOT served by the control plane (no route
//   exists in apps/control-plane), so no boundary map can be drawn; the
//   site scoping of the check records is rendered as the only observed
//   boundary information, and the gap is stated,
// - voltage/thermal validation outcomes: per-record outcome (the persisted
//   PASSED | LIMIT_VIOLATED | FAIL_CLOSED vocabulary), the failing
//   constraint / fail-closed reason, and the margins summary verbatim
//   (§12.5 steps 5 and 7, P4.06/P4.07),
// - coverage: the record's coverage statement verbatim in a distinct
//   block; a PASSED record carrying one is qualified coverage-LIMITED —
//   uncovered parts are labelled uncovered, never shown as passing,
// - model health (P4.03): the lifecycle states (draft/active/retired) and
//   the completeness/staleness/coverage axes are NOT exposed by any
//   Phase 1–4 read API, so the panel is the honest gap; the coverage AXIS
//   is partially visible per record via the coverage statements.
//
// NO feasibility-guarantee language anywhere (verdict.rs; §12.5): a
// within-limits outcome is evidence against the declared model, not a
// field-safety, certification, or approval claim.
import type { JSX } from 'react';
import type { ApiClient } from '../api/client';
import { listFeasibilityChecks } from '../api/dispatch';
import {
  ErrorBlock,
  HonestEmpty,
  LoadingLine,
  RoleScopeLine,
  useWorkspaceData,
  type LoadState,
} from '../components/workspaceData';
import { FreshnessLine } from '../components/ValueType';
import { formatUnixMillis, NOT_PRESENT as FORMAT_NOT_PRESENT } from '../components/format';
import {
  COVERAGE_QUALIFIER,
  OUTCOME_NOTE_FAIL_CLOSED,
  OUTCOME_NOTE_LIMIT_VIOLATED,
  OUTCOME_NOTE_PASSED,
  countOutcomes,
  groupBySite,
  outcomeLabel,
  toGridCheckRow,
  type GridCheckRow,
} from './gridData';

export interface GridData {
  rows: GridCheckRow[];
  consultedCount: number;
}

export async function loadGrid(client: ApiClient): Promise<GridData> {
  const page = await listFeasibilityChecks(undefined, client);
  return { rows: page.data.map(toGridCheckRow), consultedCount: page.data.length };
}

export function GridWorkspace(): JSX.Element {
  const state = useWorkspaceData(loadGrid);
  return <GridView state={state} />;
}

export function GridView({ state }: { state: LoadState<GridData> }): JSX.Element {
  return (
    <section aria-label="Grid">
      <h2>Grid</h2>
      <RoleScopeLine />
      {state.status === 'loading' && <LoadingLine what="the grid feasibility records" />}
      {state.status === 'error' && <ErrorBlock what="The feasibility-check records" error={state.error} />}
      {state.status === 'ready' && <GridReady data={state.data} readAt={state.readAt} />}
    </section>
  );
}

function GridReady({ data, readAt }: { data: GridData; readAt: string }): JSX.Element {
  const counts = countOutcomes(data.rows);
  const groups = groupBySite(data.rows);
  return (
    <>
      <h3>Topology / model version</h3>
      {data.rows.length === 0 ? (
        <HonestEmpty
          what="Topology / model version"
          reason="No feasibility-check record was returned by the tenant-scoped read, and no Phase 1–4 read API serves the versioned network models themselves (the /api/v1/grid-models §15.2 candidate path is not implemented by the control plane). No model version is invented."
          testId="model-version-gap"
        />
      ) : (
        <p>
          Model identity is shown per outcome below (the record surface binds
          every check to its model version and content hash, P4.03/P4.12: a
          model change invalidates prior evidence).
        </p>
      )}

      <h3>Feeder / substation boundaries</h3>
      <HonestEmpty
        what="Feeder / substation boundaries"
        reason="The /api/v1/grid-models resource is a frozen §15.2 candidate path the control plane does not serve yet, so no topology or boundary map can be rendered. The site scoping of the feasibility-check records below is the only observed boundary information — it is a record grouping, NEVER a drawn topology or a validated network picture."
        testId="boundaries-gap"
      />

      <h3>Voltage / thermal validation outcomes</h3>
      {data.rows.length === 0 ? (
        <HonestEmpty
          what="Voltage / thermal validation outcomes"
          reason="No feasibility-check record exists in the tenant-scoped read. No outcome, margin, or coverage figure is fabricated."
          testId="outcomes-empty"
        />
      ) : (
        <>
          <p>
            {counts.PASSED} PASSED, {counts.LIMIT_VIOLATED} LIMIT_VIOLATED,{' '}
            {counts.FAIL_CLOSED} FAIL_CLOSED{counts.other > 0 ? `, ${counts.other} outside the persisted vocabulary (rendered as-is, never as a pass)` : ''}{' '}
            — {data.consultedCount} record(s) consulted (first returned page).
          </p>
          <p>{OUTCOME_NOTE_PASSED}</p>
          {groups.map((group) => (
            <SiteOutcomeGroup key={group.site_id} group={group} readAt={readAt} />
          ))}
        </>
      )}

      <h3>Model health (P4.03)</h3>
      <HonestEmpty
        what="Model health (completeness, staleness, coverage; lifecycle draft/active/retired)"
        reason="No Phase 1–4 read API exposes model-health records (the P4.03 health check runs inside the feasibility gate; its lifecycle states and findings are not served). Shown as unavailable rather than inferred. The COVERAGE axis is partially visible per outcome below: every record carrying a coverage statement surfaces it verbatim."
        testId="model-health-gap"
      />
    </>
  );
}

function SiteOutcomeGroup({
  group,
  readAt,
}: {
  group: { site_id: string; rows: GridCheckRow[] };
  readAt: string;
}): JSX.Element {
  return (
    <article data-testid={`grid-site-${group.site_id}`}>
      <h4>
        Site scope: <code>{group.site_id}</code> ({group.rows.length} record(s))
      </h4>
      {group.rows.map((row) => (
        <CheckRow key={row.check_id} row={row} readAt={readAt} />
      ))}
    </article>
  );
}

function CheckRow({ row, readAt }: { row: GridCheckRow; readAt: string }): JSX.Element {
  return (
    <div className="grid-check" data-testid={`grid-check-${row.check_id}`}>
      <p>
        <strong>{outcomeLabel(row)}</strong> — check <code>{row.check_id}</code>,
        schedule <code>{row.schedule_id}</code> revision {row.schedule_revision}
        {row.command_id ? (
          <>
            , command <code>{row.command_id}</code>
          </>
        ) : null}
        , actor <code>{row.actor}</code>, correlation id <code>{row.correlation_id}</code>.
      </p>
      <p data-testid="model-binding">
        Model: <code>{row.model_id ?? 'not present in the record'}</code>; version:{' '}
        <code>{row.model_version ?? FORMAT_NOT_PRESENT}</code>; content hash:{' '}
        <code>{row.model_content_hash ?? FORMAT_NOT_PRESENT}</code>; completed at{' '}
        {formatUnixMillis(row.checked_at_unix_millis)}. (§20.2: model version
        and timestamp travel with EVERY outcome; a model change invalidates
        prior evidence — P4.12.)
      </p>
      {row.outcome === 'PASSED' && <p>{OUTCOME_NOTE_PASSED}</p>}
      {row.outcome === 'LIMIT_VIOLATED' && (
        <>
          <p>{OUTCOME_NOTE_LIMIT_VIOLATED}</p>
          <p data-testid="failing-constraint">
            Failing constraint: <code>{row.failing_constraint ?? FORMAT_NOT_PRESENT}</code>
          </p>
        </>
      )}
      {row.outcome === 'FAIL_CLOSED' && (
        <>
          <p>{OUTCOME_NOTE_FAIL_CLOSED}</p>
          <p data-testid="fail-closed-reason">
            Fail-closed reason: <code>{row.reason ?? FORMAT_NOT_PRESENT}</code>
          </p>
        </>
      )}
      {row.margins !== null && (
        <p data-testid="margins-block">
          Margins / diagnostics (§12.5 step 7, verbatim):{' '}
          <code>{row.margins}</code>
        </p>
      )}
      {row.coverage !== null && (
        <div role="note" data-testid="coverage-block">
          <p>{COVERAGE_QUALIFIER}</p>
          <p>
            Coverage statement (§12.5 step 6, verbatim):{' '}
            <code>{row.coverage}</code>
          </p>
        </div>
      )}
      <p>
        <FreshnessLine readAt={readAt} />
      </p>
    </div>
  );
}
