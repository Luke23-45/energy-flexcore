// FIM-F07.02 (Wave 4, Agent A; WRITTEN-UNRUN) — the SCIM 2.0 tenant-scoped
//         persistence layer: the `ScimStore` port and the in-memory
//         implementation that serves tests and single-process deployments.
//         Decision record: phase-13 `records/03_p13_03_scim_provisioning.md`
//         (the user lifecycle §2 and the group→role mapping §3); the
//         HTTP surface lives in `scim.rs`.
//
// HONESTY HEADER (FIM-F07.06, stated in full):
//
// - Implemented-under-FIM-F07. WRITTEN-UNRUN: this file has never been
//   compiled or executed; NO SCIM object has ever been provisioned, no
//   user was created or deactivated, no group→role mapping was consulted
//   in any real environment. Execution is the HUMAN BUILD/TEST GATE.
// - RFC 7643/7644 conformance is UNPROVEN: conformance against a real
//   IdP push client is a HUMAN LAB GATE (P13.20). Every SCIM schema/
//   protocol detail the P13.03 record does not pin is marked
//   UNVERIFIED-against-RFC-7644 at its use site — nothing here claims
//   conformance (conventions §7 item 3).
// - The recorded seam: a Postgres-backed SCIM store is the DEPLOYMENT-TIME
//   WIRING — the same shape as the repo's other Unwired* seams (the
//   session directory, the durable step-up store). Name it
//   `UnwiredScimStoreBackend`: NO migration was added, NO postgres-adapter
//   code was touched (the persistence policy: another program owns the
//   migrations), and until a deployment composes a durable
//   implementation, the in-memory store below is the only shipped
//   backend — single-process only, lost on restart, exactly as honest as
//   the `InMemoryStepUpStore`/`InMemorySessionDirectory` patterns it
//   mirrors. The fail-closed default is unaffected: nothing here grants
//   anything; roles come ONLY through the closed group→role vocabulary.
//
// What the store does (record 03 §2/§3, implemented):
//
// - Users are tenant-scoped records: `id` (the SCIM id, stable),
//   `user_name` (unique per tenant — case-insensitive uniqueness, the
//   stricter direction), `active`, `display_name`/`external_id`, and the
//   ordered group memberships. NO role is granted at creation — roles
//   come ONLY through group membership mapped by the closed vocabulary.
// - Groups carry the per-tenant group→role mapping (`mapped_role_id`):
//   one group maps to AT MOST one role, and the role id is validated
//   against the closed vocabulary AT THE STORE BOUNDARY through
//   `crate::auth::parse_role` — the SINGLE parser the dev path, the OIDC
//   login path, and now the SCIM provisioning path consume (the
//   single-home rule; `tests/phase13_scim_role_mapping_tests.rs` pins the
//   vocabulary; an unknown role id is refused, never stored).
// - `resolve_role_for_user` walks the user's group memberships IN ORDER
//   and resolves by the mapping's first hit — one group maps to at most
//   one role, and a principal in two mapped groups never gets a union of
//   grants. An unmapped group contributes nothing; a user in NO mapped
//   group resolves to no role (the login path REJECTS such a principal —
//   never defaults to Viewer).
// - The never-granted permission set (`dispatch.authorize`,
//   `dispatch.issue`, `secret.read`, `network.any`) is structurally
//   unreachable: `parse_role` never returns a role for those ids, and no
//   store method introduces any other authority surface.

use std::collections::HashMap;
use std::sync::Mutex;

use flexcore_authz::policy::Role;
use flexcore_domain::ids::TenantId;

use crate::auth::parse_role;

// --- The records -----------------------------------------------------------------

/// A SCIM user record, tenant-scoped (a tenant-data fact, record 01 §4:
/// the SCIM connection belongs to one tenant; the store refuses any
/// cross-tenant access by construction — every method takes the tenant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScimUser {
    /// The SCIM id (stable; assigned by the composing deployment's id
    /// generator at creation).
    pub id: String,
    /// The tenant the record belongs to (immutable after creation).
    pub tenant: TenantId,
    /// The SCIM `userName` (unique per tenant).
    pub user_name: String,
    /// The SCIM `displayName`, when the IdP pushes one.
    pub display_name: Option<String>,
    /// The SCIM `externalId` (the IdP's own identifier), when present.
    pub external_id: Option<String>,
    /// Whether the user is ACTIVE. `false` is the deprovisioning state:
    /// the enforcement point is the RESOLVER (record 03 §2 — a
    /// deactivated user's token rejects at the resolver regardless of
    /// token validity; see `scim.rs::ScimAwareIdentityResolver`).
    pub active: bool,
    /// The ordered group memberships (group ids in the order they were
    /// set — the deterministic order the role resolution walks).
    pub group_ids: Vec<String>,
    /// The creation instant (controlled clock, UTC millis).
    pub created_at_unix_millis: i64,
    /// The last update instant (controlled clock, UTC millis).
    pub updated_at_unix_millis: i64,
}

/// A SCIM group record, tenant-scoped, carrying the group→role mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScimGroup {
    /// The SCIM group id (stable; assigned by the composing deployment's
    /// id generator at creation).
    pub id: String,
    /// The tenant the record belongs to.
    pub tenant: TenantId,
    /// The SCIM `displayName`.
    pub display_name: String,
    /// The SCIM `externalId`, when the IdP pushes one.
    pub external_id: Option<String>,
    /// The member user ids (tenant-scoped; the store refuses members of
    /// another tenant).
    pub member_user_ids: Vec<String>,
    /// The mapped role id — an id of the CLOSED vocabulary
    /// (`crate::auth::parse_role`). `None` = an unmapped group
    /// (contributes nothing). The id is validated AT THE STORE BOUNDARY:
    /// an id `parse_role` does not recognize is refused, never stored.
    pub mapped_role_id: Option<String>,
    /// The creation instant (controlled clock, UTC millis).
    pub created_at_unix_millis: i64,
    /// The last update instant (controlled clock, UTC millis).
    pub updated_at_unix_millis: i64,
}

/// The creation input for a user (record 03 §2: POST /Users creates the
/// tenant-scoped record; NO role is granted at creation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewScimUser {
    /// The SCIM id (the deployment's id generator assigns it).
    pub id: String,
    /// The SCIM `userName`.
    pub user_name: String,
    pub display_name: Option<String>,
    pub external_id: Option<String>,
    /// The initial active state (the create path defaults to active; a
    /// deactivation comes through the update/delete lifecycle).
    pub active: bool,
    /// The initial group memberships (group ids; each MUST exist in the
    /// tenant — an unknown group is refused, never silently dropped).
    pub group_ids: Vec<String>,
}

/// The update input for a user (PATCH/PUT: attribute updates apply to the
/// tenant-scoped record; role-affecting fields (`group_ids`) update the
/// mapping inputs — an identity FACT, never an authorization decision: the
/// `PolicySet` remains the only decision authority, record 03 §2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScimUserPatch {
    pub user_name: Option<String>,
    pub display_name: Option<String>,
    pub external_id: Option<String>,
    pub active: Option<bool>,
    /// The REPLACEMENT group-membership list (the order is preserved — the
    /// deterministic role-resolution order).
    pub group_ids: Option<Vec<String>>,
}

/// The creation input for a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewScimGroup {
    /// The SCIM group id (the deployment's id generator assigns it).
    pub id: String,
    pub display_name: String,
    pub external_id: Option<String>,
    /// The initial member user ids (each MUST exist in the tenant).
    pub member_user_ids: Vec<String>,
    /// The mapped role id (`parse_role`-validated; `None` = unmapped).
    pub mapped_role_id: Option<String>,
}

/// The update input for a group (replacement semantics on the provided
/// fields).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScimGroupUpdate {
    pub display_name: Option<String>,
    pub external_id: Option<String>,
    /// The REPLACEMENT member list.
    pub member_user_ids: Option<Vec<String>>,
    /// The replacement mapped role id (`None` leaves it unchanged; use
    /// [`ScimGroupUpdate::unmap_role`] to clear it).
    pub mapped_role_id: Option<String>,
}

impl ScimGroupUpdate {
    /// Clears the group's role mapping (the group stays, contributes
    /// nothing).
    pub fn unmap_role() -> Self {
        Self {
            display_name: None,
            external_id: None,
            member_user_ids: None,
            mapped_role_id: Some(String::new()),
        }
    }
}

/// The list query (the SCIM pagination + the ONE minimal filter the HTTP
/// surface parses — record-pinned shape as DATA; the full RFC 7644 filter
/// grammar is UNVERIFIED-against-RFC-7644 and deliberately not built).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScimListQuery {
    /// The 1-based start index of the requested page. `0` is treated as 1
    /// (the minimal handling; the RFC-level detail is
    /// UNVERIFIED-against-RFC-7644).
    pub start_index: usize,
    /// The requested page size. `0` = no explicit page size (serve ALL
    /// matching records on one page — this repo's minimal handling, marked
    /// UNVERIFIED-against-RFC-7644).
    pub count: usize,
    /// The minimal `userName Eq "value"` filter (case-insensitive
    /// equality; `None` = no filter).
    pub user_name_eq: Option<String>,
}

impl ScimListQuery {
    /// The unfiltered, unpaged query (everything, one page).
    pub fn all() -> Self {
        Self {
            start_index: 1,
            count: 0,
            user_name_eq: None,
        }
    }
}

/// The list result (the record-pinned response shape as DATA:
/// `totalResults` + the ordered `resources`; `start_index`/`items_per_page`
/// round out the SCIM 2.0 ListResponse shape — the two extras are
/// UNVERIFIED-against-RFC-7644 beyond the record's pin).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScimListResult<T> {
    /// The number of matching records AFTER the filter, BEFORE pagination.
    pub total_results: usize,
    /// The 1-based start index the page was served from (as requested,
    /// clamped to >= 1).
    pub start_index: usize,
    /// The number of records ON the page.
    pub items_per_page: usize,
    /// The page (sorted by id — deterministic, never insertion-order
    /// dependent).
    pub resources: Vec<T>,
}

// --- The errors -----------------------------------------------------------------

/// The named store errors. Every failure is a named value — never a
/// stringly error, never a silent degradation (a refused write leaves the
/// store exactly as it was).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScimStoreError {
    /// No user with that id exists in the tenant.
    UserNotFound { id: String },
    /// No group with that id exists in the tenant.
    GroupNotFound { id: String },
    /// Another user in the tenant already carries that `userName`
    /// (case-insensitive uniqueness — the stricter direction; the RFC-level
    /// detail is UNVERIFIED-against-RFC-7643).
    DuplicateUserName { user_name: String },
    /// A referenced user id does not exist in the tenant.
    UserNotInTenant { user_id: String },
    /// The mapped role id is not on the closed vocabulary
    /// (`crate::auth::parse_role` does not recognize it) — refused, never
    /// stored.
    UnknownRoleId { role_id: String },
    /// The store lock was poisoned (fail-closed bookkeeping).
    StorePoisoned { detail: String },
}

impl std::fmt::Display for ScimStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScimStoreError::UserNotFound { id } => write!(f, "no SCIM user '{id}' in this tenant"),
            ScimStoreError::GroupNotFound { id } => {
                write!(f, "no SCIM group '{id}' in this tenant")
            }
            ScimStoreError::DuplicateUserName { user_name } => {
                write!(f, "a SCIM user with userName '{user_name}' already exists in this tenant")
            }
            ScimStoreError::UserNotInTenant { user_id } => {
                write!(f, "SCIM user '{user_id}' does not exist in this tenant")
            }
            ScimStoreError::UnknownRoleId { role_id } => write!(
                f,
                "'{role_id}' is not on the closed group→role vocabulary — the mapping is refused"
            ),
            ScimStoreError::StorePoisoned { detail } => {
                write!(f, "the SCIM store failed: {detail}")
            }
        }
    }
}

impl std::error::Error for ScimStoreError {}

// --- The port -----------------------------------------------------------------

/// The SCIM persistence port (FIM-F07.02). The in-crate
/// [`InMemoryScimStore`] serves tests and single-process deployments; the
/// durable Postgres implementation under RLS is the recorded
/// `UnwiredScimStoreBackend` deployment seam — deliberately not built in
/// this batch (no migration was added; the postgres adapter is owned by
/// another program). Every method is tenant-scoped BY SIGNATURE: a record
/// of another tenant is invisible, never readable, never writable (the
/// P1.12 §5.2 tenant-scoped-port discipline).
pub trait ScimStore: Send + Sync {
    /// Creates the user record (tenant-scoped; the `userName` must be
    /// unique per tenant, case-insensitive; every referenced group MUST
    /// exist in the tenant). NO role is granted at creation.
    fn create_user(
        &self,
        tenant: &TenantId,
        new_user: NewScimUser,
        now_unix_millis: i64,
    ) -> Result<ScimUser, ScimStoreError>;

    /// The user by SCIM id (`None` when absent or of another tenant).
    fn get_user(&self, tenant: &TenantId, id: &str) -> Result<Option<ScimUser>, ScimStoreError>;

    /// The user by `userName` (case-insensitive match — the same
    /// uniqueness rule; `None` when absent or of another tenant). This is
    /// the lookup the deprovisioning enforcement point uses (the resolver
    /// consults the store by the authenticated subject — record 03 §2).
    fn get_user_by_user_name(
        &self,
        tenant: &TenantId,
        user_name: &str,
    ) -> Result<Option<ScimUser>, ScimStoreError>;

    /// Applies the non-`None` fields of the patch to the tenant-scoped
    /// record (replacement on the fields present). Unknown id →
    /// [`ScimStoreError::UserNotFound`].
    fn update_user(
        &self,
        tenant: &TenantId,
        id: &str,
        patch: ScimUserPatch,
        now_unix_millis: i64,
    ) -> Result<ScimUser, ScimStoreError>;

    /// DEACTIVATES the user (`active = false`) — the deprovisioning path
    /// (record 03 §2/§4). The record stays (lifecycle evidence); the
    /// enforcement is at the RESOLVER (a deactivated user's token rejects
    /// regardless of token validity). Idempotent: deactivating an already
    /// deactivated user succeeds.
    fn deactivate_user(
        &self,
        tenant: &TenantId,
        id: &str,
        now_unix_millis: i64,
    ) -> Result<ScimUser, ScimStoreError>;

    /// The tenant's users, filtered + paged per the query (sorted by id —
    /// deterministic).
    fn list_users(
        &self,
        tenant: &TenantId,
        query: &ScimListQuery,
    ) -> Result<ScimListResult<ScimUser>, ScimStoreError>;

    /// Creates the group record with its member list and the
    /// (validated) role mapping.
    fn create_group(
        &self,
        tenant: &TenantId,
        new_group: NewScimGroup,
        now_unix_millis: i64,
    ) -> Result<ScimGroup, ScimStoreError>;

    /// The group by SCIM id (`None` when absent or of another tenant).
    fn get_group(&self, tenant: &TenantId, id: &str) -> Result<Option<ScimGroup>, ScimStoreError>;

    /// Applies the update to the tenant-scoped group record.
    fn update_group(
        &self,
        tenant: &TenantId,
        id: &str,
        update: ScimGroupUpdate,
        now_unix_millis: i64,
    ) -> Result<ScimGroup, ScimStoreError>;

    /// The tenant's groups, paged per the query (sorted by id). No filter
    /// (the HTTP surface serves Groups as a list only — the task-pinned
    /// surface; group creation/mapping is store-level in this batch).
    fn list_groups(
        &self,
        tenant: &TenantId,
        query: &ScimListQuery,
    ) -> Result<ScimListResult<ScimGroup>, ScimStoreError>;

    /// The group→role resolution for one user: walks the user's group
    /// memberships IN ORDER and returns the role of the FIRST mapped
    /// group (one group maps to AT MOST one role; a principal in two
    /// mapped groups resolves by the mapping's first hit, never a union —
    /// record 03 §3). `None` = the user is in no mapped group (the login
    /// path REJECTS such a principal — never defaults to Viewer).
    ///
    /// Default method: pure composition of `get_user` + `get_group` —
    /// durable implementations get it for free.
    fn resolve_role_for_user(
        &self,
        tenant: &TenantId,
        user_id: &str,
    ) -> Result<Option<Role>, ScimStoreError> {
        let user = match self.get_user(tenant, user_id)? {
            Some(user) => user,
            None => return Ok(None),
        };
        for group_id in &user.group_ids {
            let group = match self.get_group(tenant, group_id)? {
                Some(group) => group,
                None => continue,
            };
            if let Some(role_id) = &group.mapped_role_id {
                if let Some(role) = parse_role(role_id) {
                    return Ok(Some(role));
                }
            }
        }
        Ok(None)
    }
}

// --- The in-memory implementation --------------------------------------------------

/// One tenant's directory (the store's per-tenant partition).
#[derive(Default)]
struct TenantDirectory {
    users: HashMap<String, ScimUser>,
    groups: HashMap<String, ScimGroup>,
}

/// The in-memory store: tenant → (users, groups). Mirrors the
/// [`crate::stepup::InMemoryStepUpStore`] / [`crate::session::InMemorySessionDirectory`]
/// pattern: single-process, bounded by nothing (the durable swap-in is the
/// recorded `UnwiredScimStoreBackend` seam). Lists sort by id so every
/// observation is deterministic regardless of insertion order.
#[derive(Default)]
pub struct InMemoryScimStore {
    tenants: Mutex<HashMap<TenantId, TenantDirectory>>,
}

impl InMemoryScimStore {
    fn lock(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<TenantId, TenantDirectory>>, ScimStoreError>
    {
        self.tenants.lock().map_err(|_| {
            ScimStoreError::StorePoisoned {
                detail: "the in-memory SCIM store lock is poisoned".to_string(),
            }
        })
    }

    fn directory(
        tenants: &mut HashMap<TenantId, TenantDirectory>,
        tenant: &TenantId,
    ) -> &mut TenantDirectory {
        tenants.entry(*tenant).or_default()
    }
}

impl ScimStore for InMemoryScimStore {
    fn create_user(
        &self,
        tenant: &TenantId,
        new_user: NewScimUser,
        now_unix_millis: i64,
    ) -> Result<ScimUser, ScimStoreError> {
        let mut tenants = self.lock()?;
        let directory = Self::directory(&mut tenants, tenant);
        // userName uniqueness per tenant, case-insensitive (the stricter
        // direction; the RFC-level detail is UNVERIFIED-against-RFC-7643).
        if directory
            .users
            .values()
            .any(|u| u.user_name.eq_ignore_ascii_case(&new_user.user_name))
        {
            return Err(ScimStoreError::DuplicateUserName {
                user_name: new_user.user_name,
            });
        }
        // Every referenced group must exist in THIS tenant (an unknown
        // group is refused, never silently dropped).
        for group_id in &new_user.group_ids {
            if !directory.groups.contains_key(group_id) {
                return Err(ScimStoreError::GroupNotFound { id: group_id.clone() });
            }
        }
        let record = ScimUser {
            id: new_user.id,
            tenant: *tenant,
            user_name: new_user.user_name,
            display_name: new_user.display_name,
            external_id: new_user.external_id,
            active: new_user.active,
            group_ids: new_user.group_ids,
            created_at_unix_millis: now_unix_millis,
            updated_at_unix_millis: now_unix_millis,
        };
        directory.users.insert(record.id.clone(), record.clone());
        Ok(record)
    }

    fn get_user(&self, tenant: &TenantId, id: &str) -> Result<Option<ScimUser>, ScimStoreError> {
        let mut tenants = self.lock()?;
        Ok(Self::directory(&mut tenants, tenant)
            .users
            .get(id)
            .cloned())
    }

    fn get_user_by_user_name(
        &self,
        tenant: &TenantId,
        user_name: &str,
    ) -> Result<Option<ScimUser>, ScimStoreError> {
        let mut tenants = self.lock()?;
        Ok(Self::directory(&mut tenants, tenant)
            .users
            .values()
            .find(|u| u.user_name.eq_ignore_ascii_case(user_name))
            .cloned())
    }

    fn update_user(
        &self,
        tenant: &TenantId,
        id: &str,
        patch: ScimUserPatch,
        now_unix_millis: i64,
    ) -> Result<ScimUser, ScimStoreError> {
        let mut tenants = self.lock()?;
        let directory = Self::directory(&mut tenants, tenant);
        // userName uniqueness applies on rename too (excluding the record
        // being renamed).
        if let Some(user_name) = &patch.user_name {
            if directory.users.values().any(|u| {
                u.id != id && u.user_name.eq_ignore_ascii_case(user_name)
            }) {
                return Err(ScimStoreError::DuplicateUserName {
                    user_name: user_name.clone(),
                });
            }
        }
        let record = directory
            .users
            .get_mut(id)
            .ok_or_else(|| ScimStoreError::UserNotFound { id: id.to_string() })?;
        // The replacement membership is VALIDATED BEFORE any field is
        // applied: a refused write leaves the store exactly as it was.
        if let Some(group_ids) = &patch.group_ids {
            for group_id in group_ids {
                if !directory.groups.contains_key(group_id) {
                    return Err(ScimStoreError::GroupNotFound { id: group_id.clone() });
                }
            }
        }
        if let Some(user_name) = patch.user_name {
            record.user_name = user_name;
        }
        if patch.display_name.is_some() {
            record.display_name = patch.display_name;
        }
        if patch.external_id.is_some() {
            record.external_id = patch.external_id;
        }
        if let Some(active) = patch.active {
            record.active = active;
        }
        if let Some(group_ids) = patch.group_ids {
            // The replacement membership (validated above, before any
            // field was applied).
            record.group_ids = group_ids;
        }
        record.updated_at_unix_millis = now_unix_millis;
        Ok(record.clone())
    }

    fn deactivate_user(
        &self,
        tenant: &TenantId,
        id: &str,
        now_unix_millis: i64,
    ) -> Result<ScimUser, ScimStoreError> {
        self.update_user(
            tenant,
            id,
            ScimUserPatch {
                active: Some(false),
                ..ScimUserPatch::default()
            },
            now_unix_millis,
        )
    }

    fn list_users(
        &self,
        tenant: &TenantId,
        query: &ScimListQuery,
    ) -> Result<ScimListResult<ScimUser>, ScimStoreError> {
        let mut tenants = self.lock()?;
        let directory = Self::directory(&mut tenants, tenant);
        let mut matching: Vec<ScimUser> = directory
            .users
            .values()
            .filter(|u| match &query.user_name_eq {
                Some(want) => u.user_name.eq_ignore_ascii_case(want),
                None => true,
            })
            .cloned()
            .collect();
        matching.sort_by(|a, b| a.id.cmp(&b.id));
        paged(matching, query)
    }

    fn create_group(
        &self,
        tenant: &TenantId,
        new_group: NewScimGroup,
        now_unix_millis: i64,
    ) -> Result<ScimGroup, ScimStoreError> {
        let mut tenants = self.lock()?;
        let directory = Self::directory(&mut tenants, tenant);
        // The role mapping is validated AT THE STORE BOUNDARY through the
        // closed vocabulary: an unknown role id is refused, never stored
        // (the single-home rule — SCIM and login cannot disagree about a
        // group's meaning).
        if let Some(role_id) = &new_group.mapped_role_id {
            if parse_role(role_id).is_none() {
                return Err(ScimStoreError::UnknownRoleId {
                    role_id: role_id.clone(),
                });
            }
        }
        // Every referenced member must exist in THIS tenant.
        for user_id in &new_group.member_user_ids {
            if !directory.users.contains_key(user_id) {
                return Err(ScimStoreError::UserNotInTenant { user_id: user_id.clone() });
            }
        }
        let record = ScimGroup {
            id: new_group.id,
            tenant: *tenant,
            display_name: new_group.display_name,
            external_id: new_group.external_id,
            member_user_ids: new_group.member_user_ids,
            mapped_role_id: new_group.mapped_role_id,
            created_at_unix_millis: now_unix_millis,
            updated_at_unix_millis: now_unix_millis,
        };
        directory.groups.insert(record.id.clone(), record.clone());
        Ok(record)
    }

    fn get_group(&self, tenant: &TenantId, id: &str) -> Result<Option<ScimGroup>, ScimStoreError> {
        let mut tenants = self.lock()?;
        Ok(Self::directory(&mut tenants, tenant)
            .groups
            .get(id)
            .cloned())
    }

    fn update_group(
        &self,
        tenant: &TenantId,
        id: &str,
        update: ScimGroupUpdate,
        now_unix_millis: i64,
    ) -> Result<ScimGroup, ScimStoreError> {
        let mut tenants = self.lock()?;
        let directory = Self::directory(&mut tenants, tenant);
        // The replacement role mapping, validated at the boundary (an
        // empty string clears the mapping — `ScimGroupUpdate::unmap_role`).
        if let Some(role_id) = &update.mapped_role_id {
            if !role_id.is_empty() && parse_role(role_id).is_none() {
                return Err(ScimStoreError::UnknownRoleId {
                    role_id: role_id.clone(),
                });
            }
        }
        // The replacement member list: every referenced user must exist
        // in THIS tenant.
        if let Some(member_user_ids) = &update.member_user_ids {
            for user_id in member_user_ids {
                if !directory.users.contains_key(user_id) {
                    return Err(ScimStoreError::UserNotInTenant { user_id: user_id.clone() });
                }
            }
        }
        let record = directory
            .groups
            .get_mut(id)
            .ok_or_else(|| ScimStoreError::GroupNotFound { id: id.to_string() })?;
        if let Some(display_name) = update.display_name {
            record.display_name = display_name;
        }
        if update.external_id.is_some() {
            record.external_id = update.external_id;
        }
        if let Some(member_user_ids) = update.member_user_ids {
            record.member_user_ids = member_user_ids;
        }
        if let Some(role_id) = update.mapped_role_id {
            // An empty string clears the mapping (unmap).
            record.mapped_role_id = if role_id.is_empty() { None } else { Some(role_id) };
        }
        record.updated_at_unix_millis = now_unix_millis;
        Ok(record.clone())
    }

    fn list_groups(
        &self,
        tenant: &TenantId,
        query: &ScimListQuery,
    ) -> Result<ScimListResult<ScimGroup>, ScimStoreError> {
        let mut tenants = self.lock()?;
        let directory = Self::directory(&mut tenants, tenant);
        let mut matching: Vec<ScimGroup> = directory.groups.values().cloned().collect();
        matching.sort_by(|a, b| a.id.cmp(&b.id));
        paged(matching, query)
    }
}

/// The shared pagination (record-pinned shape as DATA: `total_results` is
/// the count AFTER the filter, BEFORE pagination; the page slice is
/// 1-based per `ScimListQuery`).
fn paged<T>(matching: Vec<T>, query: &ScimListQuery) -> Result<ScimListResult<T>, ScimStoreError> {
    let total_results = matching.len();
    let start_index = query.start_index.max(1);
    // count == 0 → serve ALL matching records on one page (this repo's
    // minimal handling; UNVERIFIED-against-RFC-7644).
    if query.count == 0 {
        return Ok(ScimListResult {
            total_results,
            start_index,
            items_per_page: total_results.saturating_sub(start_index.saturating_sub(1)),
            resources: matching
                .into_iter()
                .skip(start_index.saturating_sub(1))
                .collect(),
        });
    }
    let resources: Vec<T> = matching
        .into_iter()
        .skip(start_index.saturating_sub(1))
        .take(query.count)
        .collect();
    let items_per_page = resources.len();
    Ok(ScimListResult {
        total_results,
        start_index,
        items_per_page,
        resources,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TENANT_A_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
    const TENANT_B_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02";

    fn tenant(uuid_text: &str) -> TenantId {
        TenantId::new(flexcore_contracts::ids::FlexUuid::parse(uuid_text).unwrap())
    }

    fn store() -> InMemoryScimStore {
        InMemoryScimStore::default()
    }

    fn group(t: &TenantId, s: &InMemoryScimStore, id: &str, role: Option<&str>) -> ScimGroup {
        s.create_group(
            t,
            NewScimGroup {
                id: id.to_string(),
                display_name: format!("group-{id}"),
                external_id: None,
                member_user_ids: vec![],
                mapped_role_id: role.map(|r| r.to_string()),
            },
            1_000,
        )
        .expect("the group creates")
    }

    fn user(t: &TenantId, s: &InMemoryScimStore, id: &str, name: &str, groups: &[&str]) -> ScimUser {
        s.create_user(
            t,
            NewScimUser {
                id: id.to_string(),
                user_name: name.to_string(),
                display_name: None,
                external_id: None,
                active: true,
                group_ids: groups.iter().map(|g| g.to_string()).collect(),
            },
            1_000,
        )
        .expect("the user creates")
    }

    #[test]
    fn create_then_deactivate_round_trips_the_user_and_its_group_mapping() {
        let t = tenant(TENANT_A_UUID);
        let s = store();
        group(&t, &s, "g1", Some("tenant-admin"));
        let created = user(&t, &s, "u1", "alice", &["g1"]);
        assert!(created.active);
        assert_eq!(
            s.resolve_role_for_user(&t, "u1").expect("the resolution reads"),
            Some(Role::TenantAdmin),
            "the group mapping resolves through the closed vocabulary"
        );
        // The deactivation round trip: the record stays, active flips.
        let deactivated = s
            .deactivate_user(&t, "u1", 2_000)
            .expect("the deactivation books");
        assert!(!deactivated.active);
        assert_eq!(deactivated.updated_at_unix_millis, 2_000);
        // The mapping itself is unchanged — the ENFORCEMENT is the
        // resolver's active check (scim.rs), not a store-side role removal.
        assert_eq!(
            s.resolve_role_for_user(&t, "u1").expect("the resolution reads"),
            Some(Role::TenantAdmin),
            "deactivation does not delete the mapping; the resolver denies on the active flag"
        );
    }

    #[test]
    fn duplicate_user_name_is_refused_case_insensitively_and_per_tenant() {
        let t = tenant(TENANT_A_UUID);
        let s = store();
        user(&t, &s, "u1", "alice", &[]);
        assert_eq!(
            s.create_user(
                &t,
                NewScimUser {
                    id: "u2".to_string(),
                    user_name: "ALICE".to_string(),
                    display_name: None,
                    external_id: None,
                    active: true,
                    group_ids: vec![],
                },
                1_000
            ),
            Err(ScimStoreError::DuplicateUserName {
                user_name: "ALICE".to_string()
            }),
            "the uniqueness is case-insensitive (the stricter direction)"
        );
        // The same userName in ANOTHER tenant is independent (tenant-scoped).
        let other = tenant(TENANT_B_UUID);
        user(&other, &s, "u1", "alice", &[]);
    }

    #[test]
    fn unknown_group_and_unknown_role_ids_are_refused_never_stored() {
        let t = tenant(TENANT_A_UUID);
        let s = store();
        // A user referencing an unknown group.
        assert_eq!(
            s.create_user(
                &t,
                NewScimUser {
                    id: "u1".to_string(),
                    user_name: "alice".to_string(),
                    display_name: None,
                    external_id: None,
                    active: true,
                    group_ids: vec!["no-such-group".to_string()],
                },
                1_000
            ),
            Err(ScimStoreError::GroupNotFound {
                id: "no-such-group".to_string()
            })
        );
        // A group mapped outside the closed vocabulary.
        assert_eq!(
            s.create_group(
                &t,
                NewScimGroup {
                    id: "g1".to_string(),
                    display_name: "engineering".to_string(),
                    external_id: None,
                    member_user_ids: vec![],
                    mapped_role_id: Some("root".to_string()),
                },
                1_000
            ),
            Err(ScimStoreError::UnknownRoleId {
                role_id: "root".to_string()
            }),
            "the never-granted/never-mappable names are refused at the store boundary"
        );
        // An update carrying an unknown role id is refused too.
        group(&t, &s, "g1", None);
        assert_eq!(
            s.update_group(
                &t,
                "g1",
                ScimGroupUpdate {
                    mapped_role_id: Some("dispatch.authorize".to_string()),
                    ..ScimGroupUpdate::default()
                },
                1_100
            ),
            Err(ScimStoreError::UnknownRoleId {
                role_id: "dispatch.authorize".to_string()
            })
        );
    }

    #[test]
    fn role_resolution_walks_memberships_in_order_never_a_union() {
        let t = tenant(TENANT_A_UUID);
        let s = store();
        group(&t, &s, "g-unmapped", None);
        group(&t, &s, "g-operator", Some("operator"));
        group(&t, &s, "g-admin", Some("tenant-admin"));
        user(&t, &s, "u1", "alice", &["g-unmapped", "g-operator", "g-admin"]);
        assert_eq!(
            s.resolve_role_for_user(&t, "u1").expect("the resolution reads"),
            Some(Role::Operator),
            "the FIRST mapped group in order resolves; no union of grants"
        );
        // Unmapped groups contribute nothing.
        user(&t, &s, "u2", "bob", &["g-unmapped"]);
        assert_eq!(
            s.resolve_role_for_user(&t, "u2").expect("the resolution reads"),
            None,
            "a user in NO mapped group resolves to no role (the login path rejects)"
        );
        // An unknown user resolves to no role.
        assert_eq!(
            s.resolve_role_for_user(&t, "u-nope").expect("the resolution reads"),
            None
        );
    }

    #[test]
    fn membership_updates_replace_and_are_tenant_scoped() {
        let t = tenant(TENANT_A_UUID);
        let other = tenant(TENANT_B_UUID);
        let s = store();
        group(&t, &s, "g1", Some("viewer"));
        group(&other, &s, "g-other", Some("viewer"));
        user(&t, &s, "u1", "alice", &[]);
        // A cross-tenant group reference is refused.
        assert_eq!(
            s.update_user(
                &t,
                "u1",
                ScimUserPatch {
                    group_ids: Some(vec!["g-other".to_string()]),
                    ..ScimUserPatch::default()
                },
                1_100
            ),
            Err(ScimStoreError::GroupNotFound {
                id: "g-other".to_string()
            }),
            "a record of another tenant is invisible to this tenant's writes"
        );
        let updated = s
            .update_user(
                &t,
                "u1",
                ScimUserPatch {
                    group_ids: Some(vec!["g1".to_string()]),
                    ..ScimUserPatch::default()
                },
                1_100,
            )
            .expect("the update books");
        assert_eq!(updated.group_ids, vec!["g1".to_string()]);
        assert_eq!(updated.updated_at_unix_millis, 1_100);
    }

    #[test]
    fn listing_is_deterministic_filtered_and_paged() {
        let t = tenant(TENANT_A_UUID);
        let s = store();
        // Insert out of id order: the list is sorted by id regardless.
        user(&t, &s, "u3", "carol", &[]);
        user(&t, &s, "u1", "alice", &[]);
        user(&t, &s, "u2", "bob", &[]);
        user(&t, &s, "u4", "ALICE-different", &[]);

        let all = s.list_users(&t, &ScimListQuery::all()).expect("the list reads");
        assert_eq!(all.total_results, 4);
        assert_eq!(
            all.resources.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(),
            vec!["u1", "u2", "u3", "u4"],
            "the list is sorted by id — deterministic, never insertion-order"
        );

        // The minimal filter: userName Eq (case-insensitive equality).
        let filtered = s
            .list_users(
                &t,
                &ScimListQuery {
                    start_index: 1,
                    count: 0,
                    user_name_eq: Some("alice".to_string()),
                },
            )
            .expect("the filtered list reads");
        assert_eq!(filtered.total_results, 2, "ALICE-different matches case-insensitively");

        // Pagination: page 2 of size 2 over the unfiltered list.
        let page = s
            .list_users(
                &t,
                &ScimListQuery {
                    start_index: 3,
                    count: 2,
                    user_name_eq: None,
                },
            )
            .expect("the page reads");
        assert_eq!(page.total_results, 4, "totalResults counts the filter, not the page");
        assert_eq!(page.start_index, 3);
        assert_eq!(page.items_per_page, 2);
        assert_eq!(
            page.resources.iter().map(|u| u.id.as_str()).collect::<Vec<_>>(),
            vec!["u3", "u4"]
        );

        // A start index beyond the end is an empty page, not an error.
        let empty = s
            .list_users(
                &t,
                &ScimListQuery {
                    start_index: 9,
                    count: 2,
                    user_name_eq: None,
                },
            )
            .expect("the empty page reads");
        assert!(empty.resources.is_empty());
        assert_eq!(empty.total_results, 4);
    }

    #[test]
    fn group_updates_map_unmap_and_members_are_validated() {
        let t = tenant(TENANT_A_UUID);
        let s = store();
        group(&t, &s, "g1", Some("viewer"));
        user(&t, &s, "u1", "alice", &[]);
        // Member list referencing an unknown user is refused.
        assert_eq!(
            s.update_group(
                &t,
                "g1",
                ScimGroupUpdate {
                    member_user_ids: Some(vec!["u-nope".to_string()]),
                    ..ScimGroupUpdate::default()
                },
                1_100
            ),
            Err(ScimStoreError::UserNotInTenant {
                user_id: "u-nope".to_string()
            })
        );
        // Mapping + membership in one update.
        let mapped = s
            .update_group(
                &t,
                "g1",
                ScimGroupUpdate {
                    mapped_role_id: Some("site-approver".to_string()),
                    member_user_ids: Some(vec!["u1".to_string()]),
                    ..ScimGroupUpdate::default()
                },
                1_100,
            )
            .expect("the update books");
        assert_eq!(mapped.mapped_role_id.as_deref(), Some("site-approver"));
        // Unmapping clears it.
        let unmapped = s
            .update_group(&t, "g1", ScimGroupUpdate::unmap_role(), 1_200)
            .expect("the unmap books");
        assert_eq!(unmapped.mapped_role_id, None);
    }

    #[test]
    fn cross_tenant_records_are_invisible() {
        let t = tenant(TENANT_A_UUID);
        let other = tenant(TENANT_B_UUID);
        let s = store();
        user(&t, &s, "u1", "alice", &[]);
        assert_eq!(s.get_user(&other, "u1").expect("the read books"), None);
        assert_eq!(
            s.get_user_by_user_name(&other, "alice").expect("the read books"),
            None,
            "the tenant boundary holds for the subject lookup the resolver uses"
        );
        // Deactivating through the wrong tenant is a not-found, never a
        // cross-tenant write.
        assert_eq!(
            s.deactivate_user(&other, "u1", 1_100),
            Err(ScimStoreError::UserNotFound { id: "u1".to_string() })
        );
    }
}
