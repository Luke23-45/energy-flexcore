// P5.07 — Administration workspace data (§20.1 item 12): pure mapping
// functions and the honesty statements (no I/O in the pure layer; the
// loader reads the tenant-scoped APIs).
//
// What exists and what does not (the recorded inventory, decision record
// 07_31_market_plugins_admin_alarms.md):
// - EXISTS: the tenant registry read (P2.12: GET /api/v1/tenants lists the
//   caller's OWN tenant only — read-only; a tenant-administrator surface);
//   the closed authorization action vocabulary + the four roles
//   (flexcore-authz action.rs / policy.rs — mirrored here WITH a drift
//   note, the enum is authoritative); the retention configuration SHAPE
//   (P4.30 RetentionPolicy, flexcore-storage/retention.rs: seven §16.3
//   classes, each Unset by default — a configuration type, NOT a policy
//   engine, and NO read API serves a deployment's configured values); the
//   site-level edge-identity/enrollment record (P2.17, D5 OPEN — lab
//   state).
// - DOES NOT EXIST: a production identity-provider integration (Phase 1
//   ships the dev-auth resolver only, auth.rs — the real OIDC session is
//   P5.26); a deployment-configuration read API for retention values (so
//   NO per-class value is displayed — showing the shape honestly instead
//   of inventing numbers); a real edge enrollment (no edge agent exists in
//   0.x); a D4 decision (the license audit is OPEN — recorded as an input,
//   not a result).
import type { SiteDto } from '../api/types';
import type { TenantDto } from '../api/types';
import type { SiteViewDto } from '../api/views';

/** The identity-integration statement (honest about dev-auth only). */
export const IDENTITY_STATEMENT =
  'Identity integration: NO production identity provider is integrated in ' +
  'this phase. The only shipped identity resolver is the dev-auth path ' +
  '(auth.rs: a DEV-ONLY token format, compiled behind a feature gate, ' +
  'never a production credential); the real OIDC session lands with P5.26. ' +
  'Every tenant/role value this workspace shows comes from authenticated ' +
  'claims and tenant-scoped reads — never from a URL parameter or a ' +
  'selectable field (§16.2).';

/** The tenant-read statement (P2.12 grants, read-only). */
export const TENANT_READ_STATEMENT =
  'Tenants: the read is tenant-scoped and READ-ONLY — GET /api/v1/tenants ' +
  'lists the caller\'s OWN tenant only (no platform-wide tenant ' +
  'enumeration exists in Phase 1). The write surface (create/delete) is ' +
  'granted to the admin roles by policy; this workspace performs no ' +
  'administrative write and shows none.';

/** The retention statement: the configuration shape, NOT values. */
export const RETENTION_STATEMENT =
  'Data retention is shown as the CONFIGURED per-class shape (§16.3), ' +
  'never as values: the RetentionPolicy configuration type (P4.30, ' +
  'flexcore-storage/retention.rs) carries the seven data classes below, ' +
  'each defaulting to UNSET — "no decision has been supplied by the ' +
  'deployment configuration". No universal retention period is chosen in ' +
  'code (§16.3: that is the rule, not an omission), and NO read API ' +
  'serves a deployment\'s configured values in this phase — so no value ' +
  'is displayed and none is invented. A retention value shown here would ' +
  'be fabricated; the classes and their unset state are the honest record.';

/** The seven §16.3 retention classes, in report order (the RetentionPolicy
 * field names, mirrored verbatim from flexcore-storage/retention.rs). */
export const RETENTION_CLASSES: readonly string[] = [
  'raw_high_frequency_telemetry',
  'normalized_telemetry',
  'aggregated_intervals',
  'command_audit_evidence',
  'market_records',
  'simulation_artifacts',
  'model_forecast_outputs',
];

/** The four authorization roles (flexcore-authz policy.rs Role; mirrored
 * with the drift note below). */
export const AUTHZ_ROLES: readonly string[] = [
  'platform-admin',
  'tenant-admin',
  'operator',
  'viewer',
];

/** The closed authorization action vocabulary, mirrored from
 * crates/flexcore-authz/src/action.rs `Action::id()` as of P5.31, grouped
 * by the phase that added each entry. THE ENUM IS AUTHORITATIVE: this is a
 * display MIRROR (like the P2.16 availability mirror), and a drift means
 * this list must be updated by hand — no generated drift test exists for
 * the console mirror (recorded limitation; the server-side enum is pinned
 * by its own round-trip tests). */
export const AUTHZ_ACTION_SURFACE: ReadonlyArray<{ phase: string; actions: readonly string[] }> = [
  {
    phase: 'Phase 1 (registry)',
    actions: [
      'tenant.create',
      'tenant.read',
      'portfolio.create',
      'portfolio.read',
      'site.create',
      'site.read',
      'asset.create',
      'asset.read',
    ],
  },
  {
    phase: 'Phase 2 (registry CRUD + telemetry)',
    actions: [
      'tenant.delete',
      'portfolio.delete',
      'site.delete',
      'asset.delete',
      'asset.update',
      'asset.deactivate',
      'telemetry.read',
      'quarantine.read',
    ],
  },
  {
    phase: 'Phase 3 (flexibility/scheduling)',
    actions: [
      'flexibility.envelope.create',
      'flexibility.envelope.read',
      'schedule.propose',
      'schedule.read',
      'optimization_run.create',
      'optimization_run.read',
    ],
  },
  {
    phase: 'Phase 4 (dispatch evidence, read-only)',
    actions: ['command.create', 'command.validate', 'command.approve', 'command.read', 'feasibility.read', 'audit.event.read'],
  },
  {
    phase: 'Phase 5 (P5.31 alarms, read-only)',
    actions: ['alarm.read'],
  },
];

/** The §5.7 exclusions that must NEVER appear on the action surface (the
 * mirror states them so the drift note covers the dangerous direction). */
export const AUTHZ_EXCLUDED_ACTIONS: readonly string[] = [
  'dispatch.authorize',
  'dispatch.issue',
  'command.issue',
  'command.auto_approve',
];

/** The D4 statement (OPEN — an input, not a result). */
export const D4_STATEMENT =
  'D4 (license/dependency audit) is OPEN — it is a decision gate to ' +
  'resolve BEFORE v0.1.0, not a completed audit. This console and the ' +
  'simulator dependency licenses are recorded as audit INPUTS (the ' +
  'pinned frontend stack and the HELICS/pandapower pins in RESEARCH.md); ' +
  'no license-compliance conclusion is drawn or implied anywhere.';

/** The edge-enrollment statement (D5 OPEN; lab state only). */
export const EDGE_ENROLLMENT_STATEMENT =
  'Edge enrollment: NO real edge agent is enrolled in this phase — the ' +
  'first site integration is v0.4.0 (D5 OPEN). What exists is the ' +
  'registry edge identity per site and the P2.17 edge-connectivity record ' +
  '(simulator / not_connected, live:false), shown verbatim as the ' +
  'simulation/lab state. An enrolled edge device shown here would be ' +
  'fabricated; the lab state is the honest record.';

export interface EdgeEnrollmentRow {
  site: SiteDto;
  view: SiteViewDto | null;
  viewError: string | null;
}

/** The enrollment rows: one per site (the registry edge identity is the
 * API-backed record; the view carries the connectivity state). */
export function edgeEnrollmentRows(
  sites: SiteDto[],
  views: Array<SiteViewDto | null>,
  viewErrors: Array<string | null>,
): EdgeEnrollmentRow[] {
  return sites.map((site, index) => ({
    site,
    view: views[index] ?? null,
    viewError: viewErrors[index] ?? null,
  }));
}
