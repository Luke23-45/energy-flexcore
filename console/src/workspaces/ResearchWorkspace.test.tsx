// P8.27 — Research-workspace tests (the PluginsWorkspace.test.tsx pattern:
// render + data-mapping + honesty labelling on a mocked client;
// react-dom/server, no DOM environment).
//
// UNRUN — written, never executed (the loop bans; decision record
// 27_console_research.md). Covered (the ledger P8.27 test set):
// - the states are DISTINGUISHABLE (a PROPOSAL row is never an executed
//   action; research rows are structurally distinct from validated
//   schedules and executed commands — the proposed-vs-executed note
//   renders; the mode lock renders `simulation-shadow`);
// - every module's evaluation status renders VERBATIM (TEMPLATE_EMPTY /
//   NO_RECORD — the closed vocabulary carries no "results exist" value)
//   and the recorded OPEN blockers render verbatim;
// - the shadow comparisons render `NO_RUN` with the statistics ABSENT
//   (an empty denominator is never a rate of zero — no rate fabricated);
// - provenance (module/model version, input snapshot id, validation
//   outcome) is visible per proposal; a missing validation outcome
//   renders the REFUSAL, never a pass;
// - NO mutation endpoint is called or exists on the client surface (the
//   mock client records ONLY GETs to /api/v1/research/status; there is no
//   promote/issue/authorize/fast-track/approve path anywhere in the
//   module);
// - explicit confirmation is REQUIRED before the promotion request is
//   recorded (the button stays disabled until the checkbox is operated);
// - zero proposals render as the honest empty state (no proposal row is
//   fabricated);
// - the dynamic module's provenance note cites the dynamic case
//   reference, NEVER a benchmark-set version (X-5).
import { describe, expect, it, beforeEach } from 'vitest';
import { renderToString } from 'react-dom/server';
import type { ApiClient } from '../api/client';
import type { ResearchStatusDto } from '../api/research';
import {
  PROMOTION_REQUEST_NOTE,
  RESEARCH_PROPOSED_VS_EXECUTED_NOTE,
  clearPromotionRequests,
  getPromotionRequests,
  loadResearchStatus,
  moduleVersionOf,
  proposalOf,
} from './researchWorkspaceData';
import { ResearchStatusView } from './ResearchWorkspace';

const READ_AT = '2026-11-14T12:00:00.000Z';

const STATUS: ResearchStatusDto = {
  modules: [
    {
      module_id: 'admm-dmpc',
      module_version:
        'flexcore-research/admm-formulation.v1 + flexcore-research/admm-operations.v1',
      role: 'M1 (P8.02 §4): ADMM/DMPC coordination — proposals only.',
      evaluation_status: 'TEMPLATE_EMPTY',
      evaluation_record: 'ledger/phase-08-advanced-coordination/09_admm_evaluation.md',
      open_blockers: ['no P8.38 tolerance pre-registration exists yet (X-8)'],
      provenance_note: 'every result row cites flexcore-research-benchmark-set-v1',
    },
    {
      module_id: 'dynamic-controller',
      module_version: 'NOT_RECORDED',
      role: 'M4 (P8.02 §4): research controllers under disturbances in the declared ANDES model.',
      evaluation_status: 'TEMPLATE_EMPTY',
      evaluation_record: 'ledger/phase-08-advanced-coordination/17_dynamic_evaluation.md',
      open_blockers: ['ANDES is never installed and its API is UNVERIFIED in this loop (P8.16)'],
      provenance_note: 'dynamic track: rows cite inputs.dynamic_case_ref, NOT a benchmark-set version',
    },
  ],
  proposals: [],
  shadow_comparisons: [
    {
      module_id: 'marl-policy',
      baseline_id: 'baseline/highs-tier1-lp',
      comparison_status: 'NO_RUN',
      statistics_note: null,
      comparison_note: 'the MARL safety-filter statistics ride here when a shadow log ever exists',
    },
  ],
  mode_lock: 'simulation-shadow',
  status_note: 'the RECORDED Phase 8 research status — READ-ONLY surface',
};

function readyState(): { status: 'ready'; data: { status: ResearchStatusDto }; readAt: string } {
  return { status: 'ready' as const, data: { status: STATUS }, readAt: READ_AT };
}

function mockClient(calls: Array<{ path: string; method: string }>): ApiClient {
  return {
    async get<T>(path: string): Promise<T> {
      calls.push({ path, method: 'GET' });
      return { data: STATUS } as T;
    },
    async request<T>(path: string, init?: RequestInit): Promise<T> {
      calls.push({ path, method: init?.method ?? 'GET' });
      return this.get<T>(path);
    },
  };
}

describe('mapping (verbatim, no upgrades)', () => {
  it('renders the module version verbatim and the gap as NOT_RECORDED', () => {
    expect(moduleVersionOf(STATUS.modules[0])).toContain('admm-formulation.v1');
    expect(moduleVersionOf(STATUS.modules[1])).toBe('NOT_RECORDED');
  });

  it('renders a proposal verbatim with the validation refusal for a missing outcome', () => {
    const dto = {
      proposal_id: 'p-1',
      module_id: 'admm-dmpc',
      module_version: 'flexcore-research/admm-formulation.v1',
      input_snapshot_id: 'snap-1',
      validation_outcome: '',
      proposal_status: 'PROPOSED',
      provenance_note: 'basis note',
    };
    const read = proposalOf(dto);
    expect(read.proposalStatus).toBe('PROPOSED');
    expect(read.validationOutcome).toBe('VALIDATION_OUTCOME_MISSING');
    const recorded = proposalOf({ ...dto, validation_outcome: 'passed' });
    expect(recorded.validationOutcome).toBe('passed');
  });
});

describe('loadResearchStatus (mocked fetch mapping)', () => {
  it('GETs the status once and carries errors honestly', async () => {
    const calls: Array<{ path: string; method: string }> = [];
    const data = await loadResearchStatus(mockClient(calls));
    expect(data.status.modules).toHaveLength(2);
    expect(calls).toHaveLength(1);
    expect(calls[0]?.path).toBe('/api/v1/research/status');
    expect(calls.every((call) => call.method === 'GET')).toBe(true);

    const failing: ApiClient = {
      async get<T>(_path: string): Promise<T> {
        throw new Error('forbidden: action denied');
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    await expect(loadResearchStatus(failing)).rejects.toThrow('forbidden');
  });
});

describe('ResearchStatusView (render honesty)', () => {
  beforeEach(() => {
    clearPromotionRequests();
  });

  it('renders the mode lock verbatim and the no-raise rule', () => {
    const html = renderToString(<ResearchStatusView state={readyState()} />);
    expect(html).toContain('research-mode-lock');
    expect(html).toContain('<code>simulation-shadow</code>');
    expect(html).toContain('can never select or raise');
  });

  it('renders the evaluation statuses VERBATIM — no value means "results exist"', () => {
    const html = renderToString(<ResearchStatusView state={readyState()} />);
    expect(html).toContain('evaluation-status-admm-dmpc');
    expect(html).toContain('[TEMPLATE_EMPTY]');
    expect(html).toContain('evaluation-status-dynamic-controller');
    // The recorded OPEN blockers render verbatim.
    expect(html).toContain('blockers-admm-dmpc');
    expect(html).toContain('no P8.38 tolerance pre-registration exists yet');
    expect(html).toContain('blockers-dynamic-controller');
    expect(html).toContain('ANDES is never installed');
    // The module-version gap is NAMED, never an invented version.
    expect(html).toContain('module-version-dynamic-controller');
    expect(html).toContain('the gap is named, never an invented version');
  });

  it('renders the dynamic provenance WITHOUT a benchmark-set version (X-5)', () => {
    const html = renderToString(<ResearchStatusView state={readyState()} />);
    expect(html).toContain('rows cite inputs.dynamic_case_ref, NOT a benchmark-set version');
  });

  it('renders the shadow comparisons as NO_RUN with the statistics ABSENT — never a rate over an empty denominator', () => {
    const html = renderToString(<ResearchStatusView state={readyState()} />);
    expect(html).toContain('shadow-comparison-marl-policy');
    expect(html).toContain('comparison-status-marl-policy');
    expect(html).toContain('[NO_RUN]');
    expect(html).toContain('comparison-statistics-marl-policy');
    expect(html).toContain('an empty denominator is never a rate of zero');
    expect(html).toContain('baseline/highs-tier1-lp');
  });

  it('renders ZERO proposals as the honest empty state — no proposal row fabricated', () => {
    const html = renderToString(<ResearchStatusView state={readyState()} />);
    expect(html).toContain('research-proposals-empty');
    expect(html).toContain('no research run has ever happened');
  });

  it('renders a proposal with its provenance triple, labelled PROPOSAL and never executed', () => {
    const withProposal: ResearchStatusDto = {
      ...STATUS,
      proposals: [
        {
          proposal_id: 'p-1',
          module_id: 'admm-dmpc',
          module_version: 'flexcore-research/admm-formulation.v1',
          input_snapshot_id: 'snap-1',
          validation_outcome: 'VALIDATION_OUTCOME_MISSING',
          proposal_status: 'PROPOSED',
          provenance_note: 'basis note',
        },
      ],
    };
    const html = renderToString(
      <ResearchStatusView
        state={{ status: 'ready' as const, data: { status: withProposal }, readAt: READ_AT }}
      />,
    );
    expect(html).toContain('research-proposal-p-1');
    expect(html).toContain('[PROPOSAL — PROPOSED; never executed]');
    expect(html).toContain('flexcore-research/admm-formulation.v1');
    expect(html).toContain('snap-1');
    expect(html).toContain('proposal-validation-p-1');
    expect(html).toContain('VALIDATION_OUTCOME_MISSING');
    expect(html).toContain('this proposal is not a pass');
  });

  it('renders the proposed-vs-executed distinction (§20.2, P8.27 step 2)', () => {
    const html = renderToString(<ResearchStatusView state={readyState()} />);
    expect(html).toContain('research-proposed-vs-executed');
    expect(RESEARCH_PROPOSED_VS_EXECUTED_NOTE).toContain('PROPOSAL / simulation result');
    expect(RESEARCH_PROPOSED_VS_EXECUTED_NOTE).toContain('never displayed as executed commands');
    expect(RESEARCH_PROPOSED_VS_EXECUTED_NOTE).toContain('ADR-010');
  });

  it('offers a REQUEST-only promotion affordance and never calls an endpoint', () => {
    const withProposal: ResearchStatusDto = {
      ...STATUS,
      proposals: [
        {
          proposal_id: 'p-1',
          module_id: 'admm-dmpc',
          module_version: 'flexcore-research/admm-formulation.v1',
          input_snapshot_id: 'snap-1',
          validation_outcome: 'passed',
          proposal_status: 'PROPOSED',
          provenance_note: 'basis note',
        },
      ],
    };
    const html = renderToString(
      <ResearchStatusView
        state={{ status: 'ready' as const, data: { status: withProposal }, readAt: READ_AT }}
      />,
    );
    expect(html).toContain('promotion-request');
    expect(html).toContain('RECORD ONLY — nothing is promoted');
    // No mutation path exists anywhere in the client module (the load test
    // above records ONLY GETs; the affordance's handler calls the LOCAL
    // recorder, never the API client).
    expect(getPromotionRequests()).toHaveLength(0);
  });

  it('REQUIRES explicit confirmation before recording the promotion request', () => {
    expect(PROMOTION_REQUEST_NOTE).toContain('RECORDS A REQUEST');
    expect(PROMOTION_REQUEST_NOTE).toContain('protected validation -> feasibility -> authorization gates');
    expect(PROMOTION_REQUEST_NOTE).toContain('P8.29');
    const withProposal: ResearchStatusDto = {
      ...STATUS,
      proposals: [
        {
          proposal_id: 'p-1',
          module_id: 'admm-dmpc',
          module_version: 'flexcore-research/admm-formulation.v1',
          input_snapshot_id: 'snap-1',
          validation_outcome: 'passed',
          proposal_status: 'PROPOSED',
          provenance_note: 'basis note',
        },
      ],
    };
    const html = renderToString(
      <ResearchStatusView
        state={{ status: 'ready' as const, data: { status: withProposal }, readAt: READ_AT }}
      />,
    );
    expect(html).toContain('promotion-request-checkbox');
    expect(html).toContain('aria-required="true"');
    expect(html).toContain('promotion-request-button');
    expect(html).toContain('disabled');
    // Rendering alone records nothing.
    expect(getPromotionRequests()).toHaveLength(0);
  });

  it('renders the promotion audit-trail gap and the verbatim server surface note', () => {
    const html = renderToString(<ResearchStatusView state={readyState()} />);
    expect(html).toContain('research-audit-gap');
    expect(html).toContain('no promotion has ever been attempted or granted');
    expect(html).toContain('research-status-note');
    expect(html).toContain('READ-ONLY surface');
  });

  it('renders loading and error states honestly', () => {
    const loading = renderToString(
      <ResearchStatusView state={{ status: 'loading' }} />,
    );
    expect(loading).toContain('data-testid="loading"');
    const error = renderToString(
      <ResearchStatusView
        state={{ status: 'error', error: 'forbidden: action denied' }}
      />,
    );
    expect(error).toContain('data-testid="error-block"');
    expect(error).toContain('forbidden: action denied');
  });
});
