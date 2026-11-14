// P5.02–P5.04 — the §20.2 value-type and quality treatments, as components.
//
// The treatments are DATA-BORNE and TEXT-BORNE, never colour alone (the
// P5.12 accessibility baseline anticipated here; the cross-workspace
// treatment audit and the design-note record are P5.10's):
//
// - The value TYPE (measured / estimated / stale / simulated, §20.2) is
//   always rendered as its literal word inside a bracketed token, plus a
//   data-testid attribute a test pins. There is no default type: a value
//   without enough information to type it renders `untyped` and is never
//   shown as measured (fail-open typing is forbidden — a simulated value
//   mistaken for a measured one is the P5.02 risk row).
// - The telemetry QUALITY state (the frozen six, §10.4) is always rendered
//   as its literal code. Exactly one state per value; the vocabulary is
//   closed and an unknown code renders as `UNKNOWN(<code>)` rather than a
//   plausible state.
// - Freshness (event time + age) is always rendered next to the value;
//   a STALE quality additionally marks the whole reading stale (§20.2:
//   never show stale as live).
//
// Type resolution (documented, decided once — see the decision record
// 02_04_workspaces_a.md): an ESTIMATED quality types the value `estimated`
// and a STALE quality types it `stale` (those states describe the value
// itself); otherwise the record's `simulated` provenance flag types it
// `simulated` (true) or `measured` (false). In Phase 2 every telemetry
// value carries `simulated: true` (P2.11/P2.12), so every telemetry value
// on screen is typed simulated — the flag is part of the record, and the
// badge is part of the component, not an optional prop (the P5.02 risk
// mitigation).
import type { JSX } from 'react';
import type { TelemetryQuality } from '../api/types';
import { formatAgeMs, formatRfc3339, NOT_PRESENT } from './format';

export type ValueType = 'measured' | 'estimated' | 'stale' | 'simulated' | 'untyped';

/**
 * Resolves the §20.2 value type from a record's quality + provenance.
 * Decided once (see module header); exported for the workspace tests.
 */
export function valueTypeFor(quality: TelemetryQuality | string, simulated: boolean): ValueType {
  if (quality === 'ESTIMATED') return 'estimated';
  if (quality === 'STALE') return 'stale';
  if (quality === 'GOOD' || quality === 'SUSPECT' || quality === 'BAD' || quality === 'MISSING') {
    return simulated ? 'simulated' : 'measured';
  }
  // An unknown quality cannot be typed fail-open.
  return 'untyped';
}

const TYPE_LABEL: Record<ValueType, string> = {
  measured: 'measured',
  estimated: 'estimated',
  stale: 'stale',
  simulated: 'simulated',
  untyped: 'untyped value — type not derivable; never read as measured',
};

/** The §20.2 value-type token: literal word + pinned testid. */
export function ValueTypeBadge({ type }: { type: ValueType }): JSX.Element {
  return (
    <span className={`value-type value-type-${type}`} data-testid={`value-type-${type}`}>
      [{TYPE_LABEL[type]}]
    </span>
  );
}

/** Renders the frozen six-state quality code verbatim (never paraphrased). */
export function QualityStateBadge({ quality }: { quality: string }): JSX.Element {
  const frozen = [
    'GOOD',
    'SUSPECT',
    'BAD',
    'MISSING',
    'ESTIMATED',
    'STALE',
  ] as const;
  const known = (frozen as readonly string[]).includes(quality);
  return (
    <span
      className={`quality quality-${quality.toLowerCase()}`}
      data-testid={known ? `quality-${quality}` : 'quality-UNKNOWN'}
    >
      {known ? `quality: ${quality}` : `quality: UNKNOWN(${quality}) — outside the frozen six (§10.4)`}
    </span>
  );
}

export interface FreshnessProps {
  /** The value's event time, RFC 3339 (absent = the record carries none). */
  eventTime?: string | null;
  /** The record's age in milliseconds, when the API supplies one. */
  ageMs?: number | null;
  /** Extra anchor (e.g. the console's read-at time for list records). */
  readAt?: string | null;
}

/**
 * The freshness line (§20.2): every displayed value carries its event time
 * and age; nothing is presented as live without a timestamp.
 */
export function FreshnessLine({ eventTime, ageMs, readAt }: FreshnessProps): JSX.Element {
  return (
    <span className="freshness" data-testid="freshness">
      {eventTime ? `event time ${formatRfc3339(eventTime)}` : `event time ${NOT_PRESENT}`}
      {typeof ageMs === 'number' ? ` (age ${formatAgeMs(ageMs)})` : ''}
      {readAt ? ` — read at ${formatRfc3339(readAt)}` : ''}
    </span>
  );
}

// --- P5.10 — the workspace-wide treatment record and its legend --------------
//
// THE TREATMENT RECORD (§20.2 step 4 of ledger P5.10: "record the visual
// treatments ... so a later contributor cannot silently change a
// treatment"). The treatments on this console are TEXT-BORNE, not styled
// surprises; a treatment change is a change to THIS module plus its pinned
// test (designPrinciples.test.tsx), never a per-screen restyle:
//
// | Treatment        | Rendered form (always literal text)                    |
// |------------------|--------------------------------------------------------|
// | measured         | `[measured]`  — data-testid `value-type-measured`      |
// | estimated        | `[estimated]` — data-testid `value-type-estimated`     |
// | stale            | `[stale]`     — data-testid `value-type-stale`         |
// | simulated        | `[simulated]` — data-testid `value-type-simulated`     |
// | untyped          | `[untyped value — …]` — testid `value-type-untyped`    |
// | quality (6)      | `quality: <CODE>` — testid `quality-<CODE>`; unknown   |
// |                  | code renders `UNKNOWN(<code>)` verbatim                |
// | freshness        | `event time <t> (age <a>)` / `read at <t>`; a missing  |
// |                  | time renders "not present in the record", never "now"  |
// | lifecycle (14)   | `COMMAND_STATE_MARKERS` per state — marker + exact     |
// |                  | P0.11 spelling + meaning (components/honesty.tsx)      |
//
// Type resolution is decided ONCE in `valueTypeFor` above (quality-first,
// then the simulated provenance flag; fail-open typing is forbidden). The
// legend below IS the UI element that documents this — it is rendered in
// the persistent shell (App.tsx), so the treatments are on screen on every
// workspace, not tribal knowledge (§20.2 via P5.10 step 2).

/** The one-line citation the legend renders so the record is discoverable. */
export const TREATMENT_RECORD_NOTE =
  'Treatments are decided once (components/ValueType.tsx valueTypeFor + ' +
  'components/honesty.tsx CommandStateBadge) and pinned by the ' +
  'design-principles structural test; a per-screen restyle of a treatment ' +
  'is a vocabulary defect (decision record: 10_12_design_pass.md).';

/** Every value type the console can display, with its literal badge text. */
const LEGEND_TYPES: ReadonlyArray<{ type: ValueType; when: string }> = [
  { type: 'measured', when: 'telemetry quality GOOD/SUSPECT/BAD/MISSING and the record\'s simulated flag is false (never the case on the Phase 2 surface)' },
  { type: 'estimated', when: 'telemetry quality ESTIMATED (the quality state describes the value itself)' },
  { type: 'stale', when: 'telemetry quality STALE (staleness never invents a state)' },
  { type: 'simulated', when: 'the record\'s simulated provenance flag is true — every Phase 2 telemetry value on this console' },
  { type: 'untyped', when: 'the record carries an unknown quality code — never displayed as measured (fail-open typing is forbidden)' },
];

/**
 * The §20.2 legend as a UI element (P5.10 step 2): all four value types
 * plus the untyped fail-safe, the frozen six quality states, and the
 * freshness rule — rendered in the persistent shell so every workspace
 * carries it. Text-borne (colour-independent, P5.12).
 */
export function ValueTypeLegend(): JSX.Element {
  return (
    <details className="value-legend" data-testid="value-type-legend">
      <summary>How to read the values (§20.2 value types, quality states, freshness)</summary>
      <table>
        <caption>
          The §20.2 treatment legend: every displayed value carries a value
          type, a freshness timestamp, and — for telemetry — exactly one of
          the frozen six quality states (§10.4). {TREATMENT_RECORD_NOTE}
        </caption>
        <thead>
          <tr>
            <th scope="col">Treatment (as rendered)</th>
            <th scope="col">When it is used</th>
          </tr>
        </thead>
        <tbody>
          {LEGEND_TYPES.map(({ type, when }) => (
            <tr key={type}>
              <td>
                <ValueTypeBadge type={type} />
              </td>
              <td>{when}</td>
            </tr>
          ))}
          <tr>
            <td>
              <code>quality: GOOD|SUSPECT|BAD|MISSING|ESTIMATED|STALE</code>
            </td>
            <td>
              The frozen six-state telemetry quality (§10.4), exactly one per
              value; an unknown code renders verbatim as UNKNOWN(code).
            </td>
          </tr>
          <tr>
            <td>
              <code>event time … (age …) — read at …</code>
            </td>
            <td>
              Freshness (§20.2): the value's own event time + age where the
              record carries them; otherwise the console's read-at time. A
              missing timestamp renders "not present in the record", never
              "now"; STALE values are additionally marked stale.
            </td>
          </tr>
        </tbody>
      </table>
    </details>
  );
}
