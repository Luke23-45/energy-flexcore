// P5.02/P5.03 — the P2.16 asset-availability rule, mirrored for display.
//
// SOURCE OF TRUTH: `crates/flexcore-application/src/availability.rs`
// (P2.16 step 3, "the availability rule ... recorded in the design" — the
// rule is decided ONCE on the Rust side; this file is a DISPLAY mirror of
// it, not a second definition). The frozen `AssetAvailabilityChanged` event
// (§15.3) carries the FACT of a change only; consumers re-derive the
// concrete state from the asset's `availability` telemetry quantity — a
// boolean signal, unit `1`. That re-derivation is exactly what this module
// does for the console.
//
// THE MIRRORED RULE (traceable to availability.rs; each branch cites it):
// - no availability record at all                    -> unknown
// - MISSING (gap marker, carries no value)           -> no-new-signal
//   (a missing interval says nothing new; the previous assessment stands)
// - STALE (age-based; staleness never invents a state) -> no-new-signal
// - BAD (hard validation failure; never read a flag
//   that failed hard validation)                     -> unknown
// - GOOD/SUSPECT (well-formed, interpretable — the
//   P2.08 hard-vs-soft distinction) with value:
//     - true  -> available
//     - false -> unavailable (the asset's own word outranks everything)
//     - absent -> unknown
//
// THE SOURCE-REPORTING INPUT, honestly recorded (DEFAULT, decision record
// 02_04_workspaces_a.md): the Rust rule also takes a source-connectivity
// signal (a non-reporting source contradicts a stale "available" claim ->
// unknown). The Phase 1–4 read APIs do NOT expose per-asset source
// connectivity to the console. What the console CAN record: the
// current-state endpoint serves a record only because the ingestion
// pipeline emitted it, and per availability.rs "the signal is true at
// every pipeline emission point" — so a served GOOD/SUSPECT `availability`
// value was emitted while the source reported. The mirror therefore
// evaluates the GOOD/SUSPECT branches with sourceReporting = true and
// surfaces that derivation in the rendered basis text; it can NOT observe
// connectivity at the viewer's instant, which is exactly what the STALE
// branch (no-new-signal) and the age display cover. This is a recorded
// display limitation, not a silent assumption.
import type { JSX } from 'react';
import type { TelemetryQuality } from '../api/types';
import { FreshnessLine } from './ValueType';

export type AvailabilityState = 'available' | 'unavailable' | 'unknown' | 'no-new-signal';

export const AVAILABILITY_DERIVATION_STATEMENT =
  'Derived per the P2.16 availability rule from the asset\'s latest ' +
  '`availability` telemetry quantity (the frozen AssetAvailabilityChanged ' +
  'event carries the fact of change only; the concrete state is re-derived ' +
  'from telemetry). Source connectivity at the viewer\'s instant is not ' +
  'observable from the Phase 1–4 read APIs; a GOOD/SUSPECT served value was ' +
  'emitted while the source reported (crates/flexcore-application/src/' +
  'availability.rs), and STALE/MISSING signals render as "no new signal" ' +
  'rather than an invented state.';

/**
 * The P2.16 rule, mirrored for display (see the module header for the
 * branch-by-branch trace). `latestQuality === null` means no availability
 * record has ever been received; `latestValue === undefined` means the
 * record carries no value (a MISSING gap marker never does, P2.04 §3).
 */
export function deriveAvailability(
  latestQuality: TelemetryQuality | null,
  latestValue: boolean | undefined,
): AvailabilityState {
  if (latestQuality === null) return 'unknown';
  switch (latestQuality) {
    case 'MISSING':
    case 'STALE':
      // A gap marker says nothing new; staleness never invents a state.
      return 'no-new-signal';
    case 'BAD':
      // Non-silencing: never read a flag that failed hard validation.
      return 'unknown';
    case 'GOOD':
    case 'SUSPECT':
      if (latestValue === true) return 'available';
      if (latestValue === false) return 'unavailable';
      return 'unknown';
    default:
      // Outside the frozen six — cannot be interpreted; fail closed.
      return 'unknown';
  }
}

export const AVAILABILITY_LABEL: Record<AvailabilityState, string> = {
  available: 'available (per telemetry, P2.16 rule)',
  unavailable: 'unavailable (the asset reports itself unavailable)',
  unknown: 'unknown (no trustworthy availability signal)',
  'no-new-signal': 'no new signal (MISSING/STALE — the previous assessment stands; the console holds no previous state to display)',
};

/**
 * The availability line: the derived state, its basis (quality + value
 * actually present), and the derivation statement — so an operator can
 * always tell a derived display state from a recorded one.
 */
export function AvailabilityLine({
  state,
  quality,
  value,
  eventTime,
  ageMs,
}: {
  state: AvailabilityState;
  quality: TelemetryQuality | null;
  value: boolean | undefined;
  /** P5.10: the availability record's own freshness, when the caller has it. */
  eventTime?: string | null;
  ageMs?: number | null;
}): JSX.Element {
  const basis =
    quality === null
      ? 'no availability record received'
      : `latest availability record: quality ${quality}, value ${
          value === undefined ? 'absent' : String(value)
        }`;
  return (
    <p className="availability" data-testid={`availability-${state}`}>
      Availability: {AVAILABILITY_LABEL[state]} — {basis}.{' '}
      <FreshnessLine eventTime={eventTime ?? null} ageMs={ageMs ?? null} />.{' '}
      {AVAILABILITY_DERIVATION_STATEMENT}
    </p>
  );
}
