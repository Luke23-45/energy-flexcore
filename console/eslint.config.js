// P5.01 — ESLint flat config. UNPINNED toolchain (eslint ^9.0.0,
// typescript-eslint ^8.0.0 — RESEARCH.md does not name versions; the
// caret ranges are resolved at the first install and recorded, package.json
// x-pin-policy). Known risk from RESEARCH.md: typescript-eslint peer caps
// may lag a TypeScript major — re-verify the combination at first install.
// NEVER RUN in the Phase 5 loop; the human executes `npm run lint`.
import tseslint from 'typescript-eslint';

export default tseslint.config(
  { ignores: ['dist/', 'node_modules/', 'coverage/'] },
  ...tseslint.configs.recommended,
  {
    rules: {
      // The console mirrors wire shapes verbatim (snake_case fields from
      // the Rust DTOs and the frozen §15.3 event payloads); the naming
      // convention check would fight the mirror on every field.
      '@typescript-eslint/naming-convention': 'off',
      'no-console': ['warn', { allow: ['warn', 'error'] }],
    },
  },
);
