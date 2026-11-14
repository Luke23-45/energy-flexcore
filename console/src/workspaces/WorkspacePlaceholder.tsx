// P5.01 — the honest placeholder every workspace stub renders.
//
// §20.2 honesty and the ledger non-negotiables: a stub NEVER fakes data,
// never renders sample numbers, never implies an integration that does not
// exist. The empty state is the default (CONTEXT.md open question 6
// default position: empty states preferred, labelled demo only if ever
// needed — no demo data exists here).
import type { JSX } from 'react';

export interface WorkspacePlaceholderProps {
  title: string;
  owningSubtasks: string;
  /** What the workspace will show once its subtask lands (from §20.1). */
  plannedElements: readonly string[];
}

export function WorkspacePlaceholder({
  title,
  owningSubtasks,
  plannedElements,
}: WorkspacePlaceholderProps): JSX.Element {
  return (
    <section aria-label={title}>
      <h2>{title}</h2>
      <p role="status" data-testid="workspace-placeholder">
        NOT IMPLEMENTED YET — Phase 5 wave in progress (planned by{' '}
        {owningSubtasks}). This workspace shows no data because none exists
        in the console yet; nothing on this screen is simulated, measured,
        or estimated.
      </p>
      <h3>Planned elements (report §20.1)</h3>
      <ul>
        {plannedElements.map((element) => (
          <li key={element}>{element}</li>
        ))}
      </ul>
      <p>
        Control-authority note: the platform is simulation-only in the 0.x
        line (§21.4); every value this workspace will eventually show is
        labelled by type (measured / estimated / stale / simulated, §20.2)
        with its freshness timestamp. Acknowledgement is never physical
        delivery (§14.1).
      </p>
    </section>
  );
}
