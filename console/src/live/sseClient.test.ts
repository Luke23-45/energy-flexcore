// P5.25 — live-stream client tests: the SSE frame parser, the backoff
// schedule, the connection lifecycle (status transitions, reconnect with
// backoff, close correctness), and the at-least-once / resync honesty.
// UNRUN — human executes via `npm test` after install (decision record:
// 01_25_scaffold_live.md; the Phase 5 loop bans installs/builds/test runs).
import { describe, expect, it } from 'vitest';
import {
  connectLiveStream,
  nextBackoffMs,
  parseSseFrame,
  splitSseFrames,
  type LiveStreamOptions,
} from './sseClient';
import type { LiveEventEnvelope } from './types';

function envelopeJson(overrides: Partial<LiveEventEnvelope> = {}): string {
  return JSON.stringify({
    event_type: 'TELEMETRY_RECEIVED',
    event_name: 'TelemetryReceived',
    subject: 'flexcore.v1.telemetry.telemetry_received',
    correlation_id: 'c-1',
    tenant_id: 't-1',
    event_time: '2026-11-11T00:00:00Z',
    delivered_at: '2026-11-11T00:00:01Z',
    delivery: 'at-least-once',
    ordering_statement: 'events may be duplicated or reordered',
    ...overrides,
  } satisfies LiveEventEnvelope);
}

/** Builds a fetch that streams the given chunks once, then ends the body. */
function streamingFetch(chunks: string[], onCall?: () => void): typeof fetch {
  let calls = 0;
  return (async () => {
    calls += 1;
    onCall?.();
    if (calls > 1) {
      // Second connection: ends immediately (the test observes the retry).
      return new Response(null, { status: 500 });
    }
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        const encoder = new TextEncoder();
        for (const chunk of chunks) {
          controller.enqueue(encoder.encode(chunk));
        }
        controller.close();
      },
    });
    return new Response(stream, {
      status: 200,
      headers: { 'content-type': 'text/event-stream' },
    });
  }) as unknown as typeof fetch;
}




describe('parseSseFrame', () => {
  it('parses event + data fields', () => {
    expect(
      parseSseFrame('event: TELEMETRY_RECEIVED\ndata: {"a":1}'),
    ).toEqual({ event: 'TELEMETRY_RECEIVED', data: '{"a":1}' });
  });

  it('joins multi-line data with newlines and defaults event to message', () => {
    expect(parseSseFrame('data: line1\ndata: line2')).toEqual({
      event: 'message',
      data: 'line1\nline2',
    });
  });

  it('returns null for heartbeat comments and empty frames', () => {
    expect(parseSseFrame(': heartbeat')).toBeNull();
    expect(parseSseFrame('event: X')).toBeNull();
  });

  it('strips exactly one leading space from the field value (SSE spec)', () => {
    expect(parseSseFrame('data:  spaced')).toEqual({ event: 'message', data: ' spaced' });
  });
});

describe('splitSseFrames', () => {
  it('splits complete frames and keeps the trailing partial', () => {
    const { complete, rest } = splitSseFrames('a\n\nb\n\nc');
    expect(complete).toEqual(['a', 'b']);
    expect(rest).toBe('c');
  });

  it('handles CRLF frame delimiters', () => {
    const { complete } = splitSseFrames('data: x\r\n\r\ndata: y\r\n\r\n');
    expect(complete).toEqual(['data: x', 'data: y']);
  });
});

describe('nextBackoffMs (capped exponential with jitter)', () => {
  it('grows exponentially and caps at 30s', () => {
    expect(nextBackoffMs(1)).toBeGreaterThanOrEqual(800); // 1s +/- 20%
    expect(nextBackoffMs(1)).toBeLessThanOrEqual(1200);
    expect(nextBackoffMs(2)).toBeLessThanOrEqual(2400);
    expect(nextBackoffMs(10)).toBeLessThanOrEqual(36_000); // 30s cap + jitter
    expect(nextBackoffMs(10)).toBeGreaterThanOrEqual(24_000);
  });

  it('never returns below 250ms', () => {
    for (let attempt = 1; attempt <= 20; attempt += 1) {
      expect(nextBackoffMs(attempt)).toBeGreaterThanOrEqual(250);
    }
  });
});

describe('connectLiveStream (connection lifecycle)', () => {
  function options(overrides: Partial<LiveStreamOptions>): LiveStreamOptions {
    return {
      url: '/api/v1/live',
      tokenProvider: () => 'dev tenant=t subject=s role=viewer',
      fetchImpl: async () => new Response(null, { status: 401 }),
      onEnvelope: () => undefined,
      onResyncNotice: () => undefined,
      onStatusChange: () => undefined,
      ...overrides,
    };
  }

  it('delivers parsed envelopes and the open status', async () => {
    const envelopes: string[] = [];
    const statuses: string[] = [];
    let settled: (() => void) | undefined;
    const done = new Promise<void>((resolve) => {
      settled = resolve;
    });
    const handle = connectLiveStream(
      options({
        fetchImpl: streamingFetch([`event: TELEMETRY_RECEIVED\ndata: ${envelopeJson()}\n\n`]),
        onEnvelope: (e) => {
          envelopes.push(e.event_type);
          settled?.();
        },
        onStatusChange: (s) => statuses.push(s),
      }),
    );
    await done;
    handle.close();
    expect(envelopes).toEqual(['TELEMETRY_RECEIVED']);
    expect(statuses[0]).toBe('connecting');
    expect(statuses).toContain('open');
  });

  it('requires the Authorization header from the token provider (never a URL token)', async () => {
    let seenHeaders: Headers | undefined;
    const handle = connectLiveStream(
      options({
        fetchImpl: (async (input: RequestInfo, init?: RequestInit) => {
          seenHeaders = new Headers(init?.headers);
          return new Response(null, { status: 401 });
        }) as unknown as typeof fetch,
      }),
    );
    await new Promise((r) => setTimeout(r, 20));
    handle.close();
    expect(seenHeaders?.get('authorization')).toBe('Bearer dev tenant=t subject=s role=viewer');
    expect(seenHeaders?.get('accept')).toBe('text/event-stream');
  });

  it('surfaces the paused-stale status and a resync notice on stream end (no silent gap-fill)', async () => {
    const notices: string[] = [];
    let sawPausedStale = false;
    const handle = connectLiveStream(
      options({
        fetchImpl: streamingFetch([': heartbeat\n\n']),
        onResyncNotice: (n) => notices.push(n.notice),
        onStatusChange: (s) => {
          if (s === 'paused-stale') sawPausedStale = true;
        },
      }),
    );
    // Wait past the first stream end and the backoff before the retry.
    await new Promise((r) => setTimeout(r, 300));
    handle.close();
    expect(sawPausedStale).toBe(true);
    expect(notices).toContain('resync_required');
  });

  it('close() ends the stream permanently (no further reconnects)', async () => {
    let calls = 0;
    const handle = connectLiveStream(
      options({
        fetchImpl: (async () => {
          calls += 1;
          return new Response(null, { status: 401 });
        }) as unknown as typeof fetch,
      }),
    );
    await new Promise((r) => setTimeout(r, 50));
    handle.close();
    const callsAtClose = calls;
    await new Promise((r) => setTimeout(r, 150));
    expect(calls).toBe(callsAtClose);
  });

  it('passes the resync notice body through to the consumer', async () => {
    const notices: string[] = [];
    let settled: (() => void) | undefined;
    const done = new Promise<void>((resolve) => {
      settled = resolve;
    });
    const handle = connectLiveStream(
      options({
        fetchImpl: streamingFetch([
          'event: RESYNC_REQUIRED\ndata: {"notice":"resync_required","skipped_events":7,"statement":"lag"}\n\n',
        ]),
        onResyncNotice: (n) => {
          notices.push(n.notice);
          settled?.();
        },
      }),
    );
    await done;
    handle.close();
    expect(notices).toEqual(['resync_required']);
  });
});
