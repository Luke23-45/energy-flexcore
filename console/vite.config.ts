// P5.01 — Vite build/dev config for the operator console (§7.5, §21.4).
//
// RECORDED (decision record 01_25_scaffold_live.md): nothing here has been
// executed in the Phase 5 loop — no install, no vite run, no build. The
// file is written against the pinned vite ^8.2.1 / @vitejs/plugin-react
// ^5.0.0 API surface (secondary-source pins, re-verify at first install —
// package.json x-pin-policy).
//
// Build/distribution form (§21.4): `npm run build` emits a STATIC bundle in
// dist/, served by the control plane or a CDN. The control plane itself
// does not serve the bundle yet (no static-file wiring exists in the Phase
// 1–4 control plane); wiring that is a recorded follow-up, not something
// this scaffold claims.
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// The control plane binds FLEXCORE_BIND_ADDR, default 0.0.0.0:8080
// (apps/control-plane/src/config.rs). The dev server proxies every /api
// request to it, so the browser talks to the vite origin and never needs
// CORS in development.
//
// The proxy streams (http-proxy does not buffer), which is what the P5.25
// SSE endpoint (GET /api/v1/live) requires: an SSE response through a
// buffering intermediary would never deliver events incrementally. If a
// later change breaks live updates in dev, check this proxy FIRST.
const CONTROL_PLANE_TARGET = 'http://127.0.0.1:8080';

export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      '/api': {
        target: CONTROL_PLANE_TARGET,
        // The dev control plane (dev-auth feature) expects the Bearer token
        // verbatim; do not rewrite identity headers here. Tenant context is
        // derived from claims SERVER-SIDE — the console never sends a
        // tenant selector (§16.2, P5.26).
        changeOrigin: false,
      },
    },
  },
  build: {
    outDir: 'dist',
    sourcemap: true,
  },
});
