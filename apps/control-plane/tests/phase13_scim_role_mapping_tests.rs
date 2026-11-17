// P13.19 — the SCIM role-mapping conformance-scenario slice (wave 2;
// WRITTEN-UNRUN — never compiled or executed; execution is the human
// build/test gate, conventions §1 item 3).
//
// SCIM conformance (RFC 7644) is a HUMAN-GATED certification process —
// nothing here claims conformance (record 03 §5; conventions §7 item 3).
// What this file asserts is the slice of the record 03 design that exists
// as code today: the SINGLE group→role vocabulary (record 03 §3) shared
// by the SCIM provisioning design and the P13.02 login path
// (`map_role_claims` / `parse_role`), plus the never-mappable-group rule
// (the never-granted permission set contributes no role and no group can
// grant outside the closed `Action` surface).
//
// The FULL protocol scenarios (POST /Users, PATCH groups, deactivate →
// resolver deny, the 5-minute deprovisioning window) have no SCIM service
// to run against — they are recorded as the UNRUN scenario tables in the
// P13.19 decision record (§5), lab-gated on P13.20. No SCIM object has
// ever been provisioned.
use flexcore_authz::action::Action;
use flexcore_authz::policy::Role;
use flexcore_control_plane::auth::parse_role;
use flexcore_control_plane::oidc::map_role_claims;

const ROLE_IDS: [&str; 5] = [
    "platform-admin",
    "tenant-admin",
    "operator",
    "viewer",
    "site-approver",
];

// --- Scenario 1: a group named for a role id maps to exactly that role ----------------

#[test]
fn scenario_a_scim_group_named_for_a_role_id_maps_to_that_role() {
    // Record 03 §3: one group maps to AT MOST one role; the mapping is
    // through the stable ids, the same set `parse_role` (dev path) and
    // `map_role_claims` (production path) consume.
    for id in ROLE_IDS {
        assert_eq!(
            parse_role(id),
            Some(match id {
                "platform-admin" => Role::PlatformAdmin,
                "tenant-admin" => Role::TenantAdmin,
                "operator" => Role::Operator,
                "viewer" => Role::Viewer,
                _ => Role::SiteApprover,
            }),
            "the group id '{id}' maps through the closed vocabulary"
        );
    }
}

// --- Scenario 2: the login path maps the SAME vocabulary (no fork) --------------------

#[test]
fn scenario_b_the_login_path_maps_the_same_group_ids_as_the_provisioning_vocabulary() {
    // The single-home rule (record 03 §3): SCIM and login cannot disagree
    // about a group's meaning, because both consume one parser.
    for id in ROLE_IDS {
        assert_eq!(
            map_role_claims(&[id.to_string()]),
            parse_role(id),
            "the SCIM group '{id}' and the token group '{id}' map identically"
        );
    }
}

// --- Scenario 3: unmapped groups contribute nothing; no mapped group → reject ---------

#[test]
fn scenario_c_unmapped_groups_contribute_nothing_and_an_empty_set_maps_to_nothing() {
    // Record 03 §3: an unmapped group contributes nothing; a user in NO
    // mapped group resolves to no role — and the login path REJECTS
    // (never defaults to Viewer, the record 02 rule).
    assert_eq!(map_role_claims(&[]), None, "no groups → no role (the login path rejects)");
    assert_eq!(
        map_role_claims(&["engineering".to_string(), "platform-readonly".to_string()]),
        None,
        "groups outside the closed vocabulary contribute nothing"
    );
    assert_eq!(parse_role("administrator"), None);
    assert_eq!(parse_role("root"), None);
    assert_eq!(parse_role(""), None);
    assert_eq!(parse_role("Platform-Admin"), None, "the ids are exact lowercase spellings — no case-folding lenience");
}

// --- Scenario 4: the first mappable group wins (deterministic, order-stable) ----------

#[test]
fn scenario_d_multiple_groups_resolve_deterministically_to_the_first_mappable_one() {
    // Record 03 §3: one group maps to AT MOST one role — a principal in
    // two mapped groups resolves by the mapping's first hit, never a
    // union (a union would silently compound authority).
    let resolved = map_role_claims(&[
        "unmapped-team".to_string(),
        "operator".to_string(),
        "tenant-admin".to_string(),
    ]);
    assert_eq!(
        resolved,
        Some(Role::Operator),
        "the FIRST mappable group in order resolves; no union of grants"
    );
}

// --- Scenario 5: the never-granted permission set contributes no group ----------------

#[test]
fn scenario_e_the_never_granted_names_are_never_group_ids_or_actions() {
    // Record 03 §3's never-mappable rule, byte-identical (conventions
    // §6.4): none of the four names parses as a role (no group maps to
    // them) AND none of them exists on the Action surface (no scope
    // string can re-introduce them).
    for name in ["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"] {
        assert_eq!(
            parse_role(name),
            None,
            "'{name}' is never a role id — no SCIM group can grant it"
        );
        assert_eq!(
            map_role_claims(&[name.to_string()]),
            None,
            "'{name}' is never mappable through the login path"
        );
        assert_eq!(
            Action::parse(name),
            None,
            "'{name}' is not on the Action surface — structurally unreachable"
        );
    }
}

// --- Scenario 6: the deprovisioning enforcement point is the resolver (design pin) ----

#[test]
fn scenario_f_the_vocabulary_cannot_fork_between_provisioning_and_login() {
    // The compile-level pin of the single home (record 03 §3 + record 02
    // §3): both paths call the SAME parser, so the set of mappable ids is
    // one set. If a future change adds an id to one path and not the
    // other, the role-id list test in the authz matrix file
    // (`phase13_authz_matrix_tests.rs`) fails — the pair of files is the
    // drift guard.
    let authz_ids = [
        Role::PlatformAdmin.id(),
        Role::TenantAdmin.id(),
        Role::Operator.id(),
        Role::Viewer.id(),
        Role::SiteApprover.id(),
    ];
    assert_eq!(authz_ids, ROLE_IDS, "the authz Role::id spellings and the mappable group ids are ONE vocabulary");
}
