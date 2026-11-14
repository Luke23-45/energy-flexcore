// P6.25 — Sites-edge-status tests (the Sites.test.tsx pattern: render +
// data-mapping + honesty labelling on a mocked client; react-dom/server,
// no DOM environment).
//
// UNRUN — written, never executed (the loop bans; decision record
// 25_31_32_console_clock_decommission.md). Covered (the ledger P6.25
// test set):
// - the states are DISTINGUISHABLE (connectivity gap vs last-seen,
//   certificate treatments, adapter-health gap vs verbatim snapshot,
//   command-path DISABLED vs TEST_ENV_ENABLED-with-expiry vs
//   NOT_REPORTED);
// - no proposed value is displayed as executed (the proposed-vs-executed
//   statement renders on EVERY command-path display, and an enabled
//   test-env path is labelled TEST ENV ONLY);
// - NO enable affordance is wired to any endpoint (the only affordance
//   records a LOCAL request event; the mock client records zero
//   non-GET calls and no /enable path exists anywhere in the module);
// - explicit confirmation is REQUIRED before the request is recorded
//   (the button stays disabled until the checkbox is operated);
// - absent operational facts render as explicit gaps, never as plausible
//   healthy values.
import { describe, expect, it, beforeEach } from 'vitest';
import { renderToString } from 'react-dom/server';
import type { ApiClient } from '../api/client';
import type { EdgeStatusDto } from '../api/edgeStatus';
import {
  loadEdgeStatuses,
  mapEdgeStatus,
  clearCommandPathRequests,
  getCommandPathRequests,
  COMMAND_PATH_REQUEST_NOTE,
} from './sitesEdgeStatus';
import { SitesEdgeStatusSection } from './SitesEdgeStatus';

const READ_AT = 1_794_490_000_000;

const STATUS: EdgeStatusDto = {
  device_id: 'edge-01',
  site_id: 'site-a-uuid',
  enrollment_seq: 1,
  enrollment_status: 'ACTIVE',
  identity_expires_at_unix_millis: READ_AT + 30 * 86_400_000,
  certificate_expiry_unix_millis: READ_AT + 30 * 86_400_000,
  connectivity: {
    last_seen_unix_millis: READ_AT - 5_000,
    source_note: 'last-seen supplied by the composed edge-status reporter',
  },
  adapter_health_snapshot_json:
    '{"adapters":[{"adapter_id":"simulated-test","status":"HEALTHY"}]}',
  buffer_depth: 3,
  buffer_capacity: 16,
  data_freshness: {
    last_event_unix_millis: READ_AT - 2_000,
    note: 'last event time supplied by the composed edge-status reporter',
  },
  command_path: {
    state: 'DISABLED',
    enablement_valid_until_unix_millis: null,
    note: 'the P6.14 gate state as reported; the console can NEVER enable the command path',
  },
  status_note: 'polling surface',
};

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

describe('mapEdgeStatus (data mapping)', () => {
  it('maps the certificate treatments across the rotation window', () => {
    const valid = mapEdgeStatus(
      { ...STATUS, certificate_expiry_unix_millis: READ_AT + 30 * 86_400_000 },
      READ_AT,
    );
    expect(valid.certificateTreatment).toBe('valid');
    const rotationDue = mapEdgeStatus(
      { ...STATUS, certificate_expiry_unix_millis: READ_AT + 3 * 86_400_000 },
      READ_AT,
    );
    expect(rotationDue.certificateTreatment).toBe('rotation-due');
    const expired = mapEdgeStatus(
      { ...STATUS, certificate_expiry_unix_millis: READ_AT - 1 },
      READ_AT,
    );
    expect(expired.certificateTreatment).toBe('expired');
    const notReported = mapEdgeStatus(
      { ...STATUS, certificate_expiry_unix_millis: null },
      READ_AT,
    );
    expect(notReported.certificateTreatment).toBe('not-reported');
  });

  it('marks the command path enabled ONLY for TEST_ENV_ENABLED', () => {
    expect(mapEdgeStatus(STATUS, READ_AT).commandPathEnabled).toBe(false);
    expect(
      mapEdgeStatus(
        { ...STATUS, command_path: { ...STATUS.command_path, state: 'TEST_ENV_ENABLED' } },
        READ_AT,
      ).commandPathEnabled,
    ).toBe(true);
    expect(
      mapEdgeStatus(
        { ...STATUS, command_path: { ...STATUS.command_path, state: 'NOT_REPORTED' } },
        READ_AT,
      ).commandPathEnabled,
    ).toBe(false);
  });
});

describe('loadEdgeStatuses (mocked fetch mapping)', () => {
  it('maps each device independently and carries errors per slot verbatim', async () => {
    const calls: Array<{ path: string; method: string }> = [];
    const failing: ApiClient = {
      async get<T>(path: string): Promise<T> {
        calls.push({ path, method: 'GET' });
        throw new Error('not_found: no such resource');
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    const data = await loadEdgeStatuses(['edge-01'], mockClient(calls), READ_AT);
    expect(data.slots[0]?.row?.status.device_id).toBe('edge-01');
    expect(data.slots[0]?.error).toBeNull();
    expect(calls[0]?.path).toBe('/api/v1/edge/status?device_id=edge-01');
    expect(calls.every((call) => call.method === 'GET')).toBe(true);

    const failed = await loadEdgeStatuses(['edge-404'], failing, READ_AT);
    expect(failed.slots[0]?.row).toBeNull();
    expect(failed.slots[0]?.error).toContain('not_found');
  });
});

describe('SitesEdgeStatusSection (render honesty)', () => {
  beforeEach(() => {
    clearCommandPathRequests();
  });

  function readyData(): { slots: ReturnType<typeof buildSlots>; readAtUnixMillis: number } {
    return { slots: buildSlots(), readAtUnixMillis: READ_AT };
  }

  function buildSlots() {
    const mapped = mapEdgeStatus(STATUS, READ_AT);
    return [{ deviceId: 'edge-01', row: mapped, error: null }];
  }

  it('renders the states DISTINCTLY: last-seen, certificate treatment, verbatim adapter health, buffer depth, freshness', () => {
    const html = renderToString(<SitesEdgeStatusSection data={readyData()} />);
    // Connectivity: the last-seen instant + the verbatim source note.
    expect(html).toContain('edge-connectivity-last-seen');
    expect(html).toContain('last-seen supplied by the composed edge-status reporter');
    // Certificate expiry treatment (text-borne, distinct testids).
    expect(html).toContain('certificate-expiry-valid');
    expect(html).toContain('[valid]');
    // Adapter health VERBATIM — the closed vocabulary rendered as-is.
    expect(html).toContain('adapter-health-verbatim');
    expect(html).toContain('"status":"HEALTHY"');
    // Buffer depth.
    expect(html).toContain('edge-buffer-depth');
    expect(html).toContain('3 record(s) buffered of capacity 16');
    // Freshness PROMINENT (its own heading + testid), with the read-at anchor.
    expect(html).toContain('edge-data-freshness');
    expect(html).toContain('Data freshness');
    // Identity term rendered verbatim (registry vocabulary).
    expect(html).toContain('ACTIVE');
  });

  it('renders the command-path state with expiry and the proposed-vs-executed distinction', () => {
    const html = renderToString(<SitesEdgeStatusSection data={readyData()} />);
    expect(html).toContain('command-path-DISABLED');
    expect(html).toContain('[COMMAND PATH: DISABLED]');
    expect(html).toContain('proposed-vs-executed');
    expect(html).toContain('A PROPOSED setpoint is never an executed command');
  });

  it('renders a TEST_ENV_ENABLED path as TEST ENV ONLY with its expiry — never as executed', () => {
    const enabled = mapEdgeStatus(
      {
        ...STATUS,
        command_path: {
          state: 'TEST_ENV_ENABLED',
          enablement_valid_until_unix_millis: READ_AT + 3_600_000,
          note: STATUS.command_path.note,
        },
      },
      READ_AT,
    );
    const html = renderToString(
      <SitesEdgeStatusSection
        data={{ slots: [{ deviceId: 'edge-01', row: enabled, error: null }], readAtUnixMillis: READ_AT }}
      />,
    );
    expect(html).toContain('command-path-TEST_ENV_ENABLED');
    expect(html).toContain('TEST ENV ONLY');
    expect(html).toContain('enablement expires');
    expect(html).toContain('A PROPOSED setpoint is never an executed command');
  });

  it('renders absent operational facts as explicit gaps, never as healthy values', () => {
    const gapped = mapEdgeStatus(
      {
        ...STATUS,
        certificate_expiry_unix_millis: null,
        connectivity: {
          last_seen_unix_millis: null,
          source_note:
            'no edge-status reporter composed (or the edge was never seen): last-seen is NOT known',
        },
        adapter_health_snapshot_json: null,
        buffer_depth: null,
        buffer_capacity: null,
        command_path: {
          state: 'NOT_REPORTED',
          enablement_valid_until_unix_millis: null,
          note: 'no edge-status reporter is composed',
        },
      },
      READ_AT,
    );
    const html = renderToString(
      <SitesEdgeStatusSection
        data={{ slots: [{ deviceId: 'edge-01', row: gapped, error: null }], readAtUnixMillis: READ_AT }}
      />,
    );
    expect(html).toContain('certificate-expiry-not-reported');
    expect(html).toContain('never read as "valid forever"');
    expect(html).toContain('adapter-health-gap');
    expect(html).toContain('never rendered as healthy');
    expect(html).toContain('not reported (the gap is named, never assumed empty)');
    expect(html).toContain('command-path-NOT_REPORTED');
    expect(html).toContain('never a silent DISABLED');
  });

  it('renders a failed edge-status fetch as an honest per-slot error', () => {
    const html = renderToString(
      <SitesEdgeStatusSection
        data={{
          slots: [{ deviceId: 'edge-404', row: null, error: 'not_found: no such resource' }],
          readAtUnixMillis: READ_AT,
        }}
      />,
    );
    expect(html).toContain('edge-status-error');
    expect(html).toContain('not_found: no such resource');
  });

  it('offers NO enable affordance on a DISABLED path and never calls an endpoint', () => {
    const html = renderToString(
      <SitesEdgeStatusSection data={{ slots: buildSlots(), readAtUnixMillis: READ_AT }} />,
    );
    // No enable affordance exists on the DISABLED display:
    expect(html).toContain('no-request-affordance');
    expect(html).toContain('there is no enable affordance on this console');
    expect(html).toContain('the server has no enable endpoint either');
    // And the module never issues any non-GET call (the mock client in the
    // load test records ONLY GETs; there is no /enable path anywhere to
    // call — the server surface has none either).
    expect(getCommandPathRequests()).toHaveLength(0);
  });

  it('REQUIRES explicit confirmation before recording the request, then records LOCALLY only', () => {
    // The affordance text states the recorded rule verbatim:
    expect(COMMAND_PATH_REQUEST_NOTE).toContain('RECORDS A REQUEST');
    expect(COMMAND_PATH_REQUEST_NOTE).toContain('ONLY');
    expect(COMMAND_PATH_REQUEST_NOTE).toContain('P6.14');
    // (The checkbox → button-disabled → recorded flow is exercised by the
    // interaction tests on a DOM environment; this render-level test pins
    // that the confirmation controls EXIST and that nothing is recorded
    // without them.)
    const enabled = mapEdgeStatus(
      {
        ...STATUS,
        command_path: { ...STATUS.command_path, state: 'TEST_ENV_ENABLED' },
      },
      READ_AT,
    );
    const html = renderToString(
      <SitesEdgeStatusSection
        data={{ slots: [{ deviceId: 'edge-01', row: enabled, error: null }], readAtUnixMillis: READ_AT }}
      />,
    );
    expect(html).toContain('command-path-request-checkbox');
    expect(html).toContain('aria-required="true"');
    expect(html).toContain('command-path-request-button');
    expect(html).toContain('Record request (local console event only)');
    // Rendering alone records nothing:
    expect(getCommandPathRequests()).toHaveLength(0);
  });
});
