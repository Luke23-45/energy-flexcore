// P5.01 — console entry point (the Vite react-ts template shape).
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
// FRV1-R9 — the console visual system (one stylesheet keyed on the existing
// semantic class names; contract:
// docs/reviews/final_review_for_v1/01_reviews/design_spec_console_ios.md).
// ADDITIVE import only — no other line of this entry point is modified.
import './styles/design-system.css';

const rootElement = document.getElementById('root');
if (!rootElement) {
  throw new Error('the console root element is missing from index.html');
}

// StrictMode: double-invoked effects surface connection-lifecycle bugs in
// the P5.25 live hook early (the SSE client must be strictly
// cleanup-correct: no leaked streams, no duplicated reconnection loops).
createRoot(rootElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
