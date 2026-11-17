// P5.25 — the wired re-sync contract tests (FRV1-R3-006 resolution (a)).
// WRITTEN-UNRUN — the human gate runs `npm test` after the first install.
//
// Two layers, mirroring the recorded router.ts pattern (pure core tested
// directly, wiring pinned structurally):
// 1. BEHAVIOURAL — `runWorkspaceLoad` (the framework-free load
//    choreography): a successful read acknowledges the pending re-sync; a
//    FAILED read never does (an errored state is not a re-sync — the gap
//    stays flagged until a real re-read lands).
// 2. STRUCTURAL — the hook/provider wiring exists (the shell provides the
//    resync channel from useLiveEvents; the data hooks key on the token).
//    The hook effect choreography itself needs a DOM + effects, which the
//    node test environment does not have (the honest limitation recorded
//    in a11yPrinciples.test.tsx applies here too).
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  runWorkspaceLoad,
  type LoadState,
  type WorkspaceResync,
} from './workspaceData';
import type { ApiClient } from '../api/client';

function recordingResync(token = 3): { resync: WorkspaceResync; acknowledges: number[] } {
  const acknowledges: number[] = [];
  return {
    resync: {
      token,
      acknowledge: () => {
        acknowledges.push(token);
      },
    },
    acknowledges,
  };
}

describe('runWorkspaceLoad (the re-sync choreography, P5.25)', () => {
  it('settles loading → ready and ACKNOWLEDGES the re-sync on a successful read', async () => {
    const { resync, acknowledges } = recordingResync();
    const settled: LoadState<string>[] = [];
    await runWorkspaceLoad<string>(
      () => Promise.resolve('the-data'),
      resync,
      (state) => settled.push(state),
    );
    expect(settled).toHaveLength(2);
    expect(settled[0]).toEqual({ status: 'loading' });
    const ready = settled[1];
    expect(ready?.status).toBe('ready');
    if (ready?.status === 'ready') {
      expect(ready.data).toBe('the-data');
      expect(ready.readAt).toBeTruthy();
    }
    expect(acknowledges).toEqual([3]);
  });

  it('settles loading → error and NEVER acknowledges a failed read (a failed read is not a re-sync)', async () => {
    const { resync, acknowledges } = recordingResync();
    const settled: LoadState<string>[] = [];
    await runWorkspaceLoad<string>(
      () => Promise.reject(new Error('the read failed')),
      resync,
      (state) => settled.push(state),
    );
    expect(settled).toHaveLength(2);
    expect(settled[0]).toEqual({ status: 'loading' });
    expect(settled[1]).toEqual({ status: 'error', error: 'unreadable response: the read failed' });
    expect(acknowledges).toEqual([]);
  });

  it('works with no resync channel (standalone render / tests): loads, no acknowledgement', async () => {
    const settled: LoadState<number>[] = [];
    await runWorkspaceLoad<number>(
      (_client: ApiClient) => Promise.resolve(7),
      null,
      (state) => settled.push(state),
    );
    expect(settled[1]).toEqual({ status: 'ready', data: 7, readAt: expect.any(String) });
  });
});

describe('the re-sync wiring exists end to end (structural pins)', () => {
  const read = (path: string): string =>
    readFileSync(new URL(path, import.meta.url), 'utf8');

  it('the shell provides the workspace resync channel from the live session', () => {
    const app = read('../App.tsx');
    expect(app).toContain('WorkspaceResyncContext.Provider');
    expect(app).toContain('token: live.resyncToken');
    expect(app).toContain('acknowledge: live.acknowledgeResync');
  });

  it('useLiveEvents exposes resyncToken and bumps it on BOTH gap paths (notice + paused-stale)', () => {
    const hook = read('../live/useLiveEvents.ts');
    expect(hook).toContain('resyncToken: number');
    // Every gap-flagging site pairs setNeedsResync(true) with a token bump.
    const bumps = hook.split('setResyncToken((token) => token + 1)').length - 1;
    const flags = hook.split('setNeedsResync(true)').length - 1;
    expect(bumps).toBe(flags);
    expect(bumps).toBeGreaterThan(0);
    expect(hook).toContain('resyncToken,');
  });

  it('the mount-loaded hand-rolled workspaces key their read on the resync token', () => {
    expect(read('../workspaces/Schedules.tsx')).toContain('resync?.token');
    expect(read('../workspaces/Dispatch.tsx')).toContain('resync?.token');
  });

  it('the workspace data hook keys the effect on the resync token', () => {
    expect(read('./workspaceData.tsx')).toContain('[resync?.token]');
  });
});
