// P5.06 — Dispatch workspace tests (Vitest). UNRUN — never executed in the
// Phase 5 loop (no install/build/test; decision record: 05_09_workspaces_b.md).
// The human executes these via `npm test` after the first install.
//
// Coverage per the task contract: rendering, data mapping (gate-evidence
// extraction, rejection view, approval parameters/resolution), state
// vocabulary (the 14 P0.11 states EXACTLY), and honesty labelling on mocked
// fetch (simulated issue/acknowledgement wording; acknowledgement NEVER
// physical delivery; the missing-transition-endpoint gap; statements verbatim).
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import { createApiClient, type ApiClient } from '../api/client';
import type {
  CommandTransitionDto,
  DataEnvelope,
  DispatchCommandDetailDto,
  DispatchCommandDto,
  FeasibilityCheckDto,
  PageEnvelope,
} from '../api/types';
import {
  COMMAND_STATE_DEFINITIONS,
  COMMAND_STATES,
  GATES,
} from '../components/stateVocabulary';
import {
  approvalParameters,
  ApprovalFlow,
  CommandListItem,
  gateEvidence,
  loadCommandDetail,
  loadCommandList,
  rejectionView,
  resolveApprovalAction,
  simulatedMeaning,
} from './Dispatch';

// --- Fixtures (Phase 4 wire shapes, from dto_phase4.rs) ----------------------

function commandFixture(overrides: Partial<DispatchCommandDto>): DispatchCommandDto {
  return {
    command_id: 'command-1',
    tenant_id: 'tenant-1',
    portfolio_id: 'portfolio-1',
    site_id: 'site-1',
    asset_id: 'asset-1',
    schedule_id: 'sched-chain-1',
    schedule_revision: 2,
    valid_until_millis: 1767312000000,
    state: 'PROPOSED',
    command_payload: { intervals: [{ p_export_mw: 0.4 }] },
    no_execute_endpoint_statement: 'NO-EXECUTE-STATEMENT-VERBATIM',
    simulation_only_statement: 'SIMULATION-ONLY-STATEMENT-VERBATIM',
    ...overrides,
  };
}

function transitionFixture(overrides: Partial<CommandTransitionDto>): CommandTransitionDto {
  return {
    seq: 1,
    from_state: 'PROPOSED',
    to_state: 'VALIDATED',
    actor: 'svc:validator',
    at_unix_millis: 1767225700000,
    ...overrides,
  };
}

function checkFixture(overrides: Partial<FeasibilityCheckDto>): FeasibilityCheckDto {
  return {
    check_id: 'check-1',
    tenant_id: 'tenant-1',
    command_id: 'command-1',
    portfolio_id: 'portfolio-1',
    site_id: 'site-1',
    asset_id: 'asset-1',
    schedule_id: 'sched-chain-1',
    schedule_revision: 2,
    model_id: 'model-1',
    model_version: 'grid-model-v1',
    model_content_hash: 'sha256:abc',
    outcome: 'PASSED',
    actor: 'svc:feasibility',
    correlation_id: 'corr-1',
    checked_at_unix_millis: 1767225800000,
    no_execute_endpoint_statement: 'NO-EXECUTE-STATEMENT-VERBATIM',
    ...overrides,
  };
}

const ACKNOWLEDGED_COMMAND = commandFixture({ command_id: 'command-ack', state: 'ACKNOWLEDGED' });
const ACK_JOURNAL: CommandTransitionDto[] = [
  transitionFixture({ seq: 1, from_state: 'PROPOSED', to_state: 'VALIDATED', gate: 'validation', evidence_reference: 'validation-evidence-1' }),
  transitionFixture({ seq: 2, from_state: 'VALIDATED', to_state: 'FEASIBLE', gate: 'feasibility', evidence_reference: 'feasibility-evidence-1' }),
  transitionFixture({ seq: 3, from_state: 'FEASIBLE', to_state: 'APPROVED', gate: 'authorization', evidence_reference: 'authorization-decision-1', actor: 'user:operator-1' }),
  transitionFixture({ seq: 4, from_state: 'APPROVED', to_state: 'ISSUED', evidence_reference: 'signature-ref-1', actor: 'svc:dispatcher' }),
  transitionFixture({ seq: 5, from_state: 'ISSUED', to_state: 'ACKNOWLEDGED', evidence_reference: 'acceptance-ref-1', actor: 'sim:adapter' }),
];

const REJECTED_COMMAND = commandFixture({ command_id: 'command-rej', state: 'REJECTED' });
const REJECT_JOURNAL: CommandTransitionDto[] = [
  transitionFixture({ seq: 1, from_state: 'VALIDATED', to_state: 'REJECTED', gate: 'feasibility', reason: 'transformer thermal limit violated at interval 7', evidence_reference: 'feasibility-evidence-rej-1' }),
];

// --- Vocabulary tests ---------------------------------------------------------

describe('the 14-state lifecycle vocabulary (P0.11, EXACT spellings)', () => {
  it('COMMAND_STATES is exactly the frozen 14 states in frozen order', () => {
    expect([...COMMAND_STATES]).toEqual([
      'PROPOSED',
      'VALIDATED',
      'FEASIBLE',
      'APPROVED',
      'ISSUED',
      'ACKNOWLEDGED',
      'TRACKING',
      'COMPLETED',
      'REJECTED',
      'FAILED',
      'EXPIRED',
      'CANCELLED',
      'SUPERSEDED',
      'FALLBACK',
    ]);
  });

  it('every state has a distinct definition and marker (states readable without a tooltip)', () => {
    expect(COMMAND_STATE_DEFINITIONS).toHaveLength(14);
    const meanings = new Set(COMMAND_STATE_DEFINITIONS.map((d) => d.meaning));
    expect(meanings.size).toBe(14);
  });

  it('the three gates are the distinct §14.1 vocabulary, verbatim', () => {
    expect([...GATES]).toEqual(['validation', 'feasibility', 'authorization']);
  });
});

describe('simulated labelling (P5.13 wording rules)', () => {
  it('ISSUED means issued to the SIMULATOR (never "issued to device")', () => {
    const meaning = simulatedMeaning('ISSUED');
    expect(meaning).toContain('SIMULATOR');
    expect(meaning).not.toContain('issued to device');
    expect(meaning).not.toContain('delivered');
    expect(meaning).not.toContain('executed on');
  });

  it('ACKNOWLEDGED is application-layer acceptance and NEVER physical delivery', () => {
    const meaning = simulatedMeaning('ACKNOWLEDGED');
    expect(meaning).toContain('APPLICATION-LAYER ACCEPTANCE');
    expect(meaning).toContain('NOT physical delivery');
  });

  it('TRACKING/COMPLETED/FAILED/FALLBACK carry the simulated qualification', () => {
    for (const state of ['TRACKING', 'COMPLETED', 'FAILED', 'FALLBACK']) {
      expect(simulatedMeaning(state)).toContain('SIMULATED');
    }
  });
});

// --- Data-mapping tests --------------------------------------------------------

describe('gateEvidence (three DISTINCT evidences, never merged)', () => {
  it('extracts one distinct entry per gate with its own evidence reference', () => {
    const evidence = gateEvidence(ACK_JOURNAL);
    expect(evidence).toHaveLength(3);
    const gates = evidence.map((e) => e.gate);
    expect(gates).toEqual(['validation', 'feasibility', 'authorization']);
    expect(evidence.find((e) => e.gate === 'validation')?.evidenceReference).toBe('validation-evidence-1');
    expect(evidence.find((e) => e.gate === 'feasibility')?.evidenceReference).toBe('feasibility-evidence-1');
    expect(evidence.find((e) => e.gate === 'authorization')?.evidenceReference).toBe('authorization-decision-1');
    // The authorization actor is distinct from the service actors.
    expect(evidence.find((e) => e.gate === 'authorization')?.actor).toBe('user:operator-1');
  });

  it('non-gate transitions are not presented as gate evidence', () => {
    const evidence = gateEvidence(ACK_JOURNAL.filter((entry) => entry.seq >= 4));
    expect(evidence).toHaveLength(0);
  });
});

describe('rejectionView (failing constraint + evidence, §20.2)', () => {
  it('names the rejecting gate, the failing check, and the evidence reference', () => {
    const view = rejectionView('REJECTED', REJECT_JOURNAL, []);
    expect(view.gate).toBe('feasibility');
    expect(view.failingCheck).toBe('transformer thermal limit violated at interval 7');
    expect(view.evidenceReference).toBe('feasibility-evidence-rej-1');
  });

  it('links the failing feasibility-check record with its constraint', () => {
    const check = checkFixture({
      outcome: 'LIMIT_VIOLATED',
      failing_constraint: 'transformer T1 loading 103% > 100% at interval 7',
      margins: 'T1: +3% over limit',
    });
    const view = rejectionView('REJECTED', REJECT_JOURNAL, [check]);
    expect(view.matchingCheck).not.toBeNull();
    expect(view.matchingCheck?.failing_constraint).toContain('transformer T1');
  });

  it('returns an empty view for non-rejected commands', () => {
    const view = rejectionView('COMPLETED', ACK_JOURNAL, []);
    expect(view.gate).toBeNull();
    expect(view.matchingCheck).toBeNull();
  });
});

describe('approvalParameters + resolveApprovalAction (no invented transition)', () => {
  it('assembles actor from claims, scope from the command, and expiry from valid_until', () => {
    const parameters = approvalParameters(ACKNOWLEDGED_COMMAND, {
      tenantId: 'tenant-1',
      subject: 'operator-1',
      role: 'operator',
    });
    expect(parameters.actor).toContain('operator-1');
    expect(parameters.scope.assetId).toBe('asset-1');
    expect(parameters.scope.scheduleRevision).toBe(2);
    expect(parameters.expiryUtc).toBe(new Date(1767312000000).toISOString());
  });

  it('states "limits not displayed" instead of inventing limit figures', () => {
    const parameters = approvalParameters(ACKNOWLEDGED_COMMAND, null);
    expect(parameters.limits).toContain('does not invent limit figures');
  });

  it('uses payload limits only when the payload actually carries them', () => {
    const withLimits = commandFixture({
      command_payload: { intervals: [], limits: { max_p_export_mw: 1.0 } },
    });
    const parameters = approvalParameters(withLimits, null);
    expect(parameters.limits).toContain('max_p_export_mw');
  });

  it('resolves to the MISSING-ENDPOINT GAP for a FEASIBLE command — never a performed approval', () => {
    const resolution = resolveApprovalAction(commandFixture({ state: 'FEASIBLE' }));
    expect(resolution.kind).toBe('gap');
    if (resolution.kind === 'gap') {
      expect(resolution.detail).toContain('MISSING-ENDPOINT GAP');
      expect(resolution.detail).toContain('READ-ONLY');
      expect(resolution.detail).toContain('does not invent one');
    }
  });

  it('a non-FEASIBLE command is noted as not approvable in this state', () => {
    const resolution = resolveApprovalAction(commandFixture({ state: 'APPROVED' }));
    expect(resolution.kind).toBe('not-approvable-state');
  });
});

// --- Rendering tests -------------------------------------------------------------

describe('rendering (fixture-driven)', () => {
  it('the command list item renders the exact state spelling via the badge', () => {
    const html = renderToString(<CommandListItem command={ACKNOWLEDGED_COMMAND} />);
    expect(html).toContain('ACKNOWLEDGED');
    expect(html).toContain('accepted by simulator (simulated)');
    expect(html).toContain('command-ack');
  });

  it('the approval flow renders parameters and, once confirmed, the gap resolution', () => {
    const feasible = commandFixture({ state: 'FEASIBLE' });
    const unconfirmed = renderToString(
      <ApprovalFlow command={feasible} claims={null} confirmed={false} onConfirm={() => undefined} />,
    );
    expect(unconfirmed).toContain('confirm-high-impact-checkbox');
    expect(unconfirmed).toContain('I confirm this high-impact action');
    const confirmed = renderToString(
      <ApprovalFlow command={feasible} claims={null} confirmed={true} onConfirm={() => undefined} />,
    );
    expect(confirmed).toContain('approval-resolution-gap');
    expect(confirmed).toContain('MISSING-ENDPOINT GAP');
    expect(confirmed).toContain('NO approval was performed from the console');
  });
});

// --- Mocked-fetch tests -----------------------------------------------------------

describe('data loading over a mocked fetch', () => {
  it('loadCommandList hits the read-only list endpoint with the state filter', async () => {
    const paths: string[] = [];
    const client: ApiClient = createApiClient({
      fetchImpl: async (input: RequestInfo | URL) => {
        paths.push(String(input));
        return new Response(
          JSON.stringify({ data: [commandFixture({})], next_cursor: null } satisfies PageEnvelope<DispatchCommandDto>),
          { status: 200 },
        );
      },
    });
    const list = await loadCommandList(client, 'ISSUED');
    expect(paths[0]).toBe('/api/v1/dispatch-commands?state=ISSUED&limit=200');
    expect(list.commands).toHaveLength(1);
    expect(list.truncated).toBe(false);
  });

  it('loadCommandDetail reads the detail + the command-scoped feasibility checks', async () => {
    const paths: string[] = [];
    const client: ApiClient = createApiClient({
      fetchImpl: async (input: RequestInfo | URL) => {
        paths.push(String(input));
        const url = String(input);
        if (url.startsWith('/api/v1/dispatch-commands/command-ack')) {
          return new Response(
            JSON.stringify({
              data: { command: ACKNOWLEDGED_COMMAND, journal: ACK_JOURNAL },
            } satisfies DataEnvelope<DispatchCommandDetailDto>),
            { status: 200 },
          );
        }
        return new Response(
          JSON.stringify({ data: [checkFixture({})], next_cursor: null } satisfies PageEnvelope<FeasibilityCheckDto>),
          { status: 200 },
        );
      },
    });
    const detail = await loadCommandDetail(client, 'command-ack');
    expect(paths.some((p) => p === '/api/v1/dispatch-commands/command-ack')).toBe(true);
    expect(paths.some((p) => p.startsWith('/api/v1/feasibility-checks?command_id=command-ack'))).toBe(true);
    expect(detail.detail.command.state).toBe('ACKNOWLEDGED');
    expect(detail.checks).toHaveLength(1);
  });

  it('an API error surfaces as a rejected promise with the server code (no invented data)', async () => {
    const client: ApiClient = createApiClient({
      fetchImpl: async () =>
        new Response(JSON.stringify({ error: { code: 'not_found', message: 'no such command' } }), { status: 404 }),
    });
    await expect(loadCommandDetail(client, 'missing')).rejects.toMatchObject({
      status: 404,
      code: 'not_found',
    });
  });
});
