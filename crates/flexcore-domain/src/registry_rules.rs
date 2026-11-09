// P2.02/P2.03 — Registry deletion rule and parent-emptiness guards.
//
// THE DELETION RULE, DECIDED ONCE (P2.02 step 5):
//
// **RESTRICT for non-empty parents.** A registry node that still owns
// dependents cannot be deleted:
//
// - a tenant that still owns one or more portfolios cannot be deleted;
// - a portfolio that still owns one or more sites cannot be deleted;
// - a site that still owns one or more assets cannot be deleted.
//
// Empty parents and leaves (portfolio without sites, site without assets)
// may be deleted through the normal CRUD surface (P2.12 API `DELETE`).
//
// Rationale (recorded for review, not re-decidable per call site):
//
// - **Cascade would orphan referenced state.** The ownership chain
//   tenant → portfolio → site → asset is referenced by every downstream
//   artefact: canonical telemetry (P2.04) carries tenant/site/asset IDs,
//   the §15.3 events carry the full chain, and deterministic replay (P2.24)
//   compares stored records against re-generated ones. Deleting a parent and
//   silently dropping its subtree would strand telemetry rows and event
//   payloads pointing at identifiers that no longer resolve; deleting the
//   rows "along with" the parent would destroy the historical record the
//   append-only audit discipline (P1.10) exists to protect.
// - **Restrict is the safe default under incomplete information.** A delete
//   request can always be re-issued after the dependents are explicitly
//   removed (each removal individually visible and auditable); an accidental
//   cascade cannot be undone. Deletion is a working-data lifecycle operation
//   (`StateChangeOperation::Delete`), never an audit operation.
// - **The audit trail is preserved either way.** Every accepted deletion is
//   recorded as a state change and paired with an outbox event (§6.3), and
//   the append-only audit history (P1.10 `AuditSink`) is never cascaded —
//   audit records outlive the registry rows they describe by design.
//
// Mechanical form: the guards below are **pure functions** over the child
// counts a caller already holds (fetched through the tenant-scoped
// repository ports). The domain has no I/O, so the count is supplied, not
// queried; the application/API layer (P2.12 `DELETE` routes) is the caller
// that fetches `list_for_tenant` and refuses the deletion when the guard
// rejects. Keeping the rule here means there is exactly one spelling of it —
// the fakes, the future adapters, and the API tests all cite the same rule.

/// Errors returned by the registry deletion guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeletionRuleError {
    /// The parent still owns `count` dependents and is therefore protected
    /// by the RESTRICT rule. Remove the dependents explicitly (each an
    /// individually visible, auditable operation) before deleting the parent.
    NonEmptyParent {
        /// The parent entity kind (a `flexcore_ports::state_change::entities`
        /// name at the call sites; a free `&'static str` here so the domain
        /// stays port-free).
        entity: &'static str,
        /// The dependent entity kind.
        dependent: &'static str,
        /// How many dependents the parent still owns.
        count: usize,
    },
}

impl std::fmt::Display for DeletionRuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeletionRuleError::NonEmptyParent {
                entity,
                dependent,
                count,
            } => write!(
                f,
                "{entity} cannot be deleted while it still owns {count} {dependent} \
                 (RESTRICT rule, P2.02)"
            ),
        }
    }
}

impl std::error::Error for DeletionRuleError {}

/// Guard: a **tenant** may only be deleted when it owns no portfolios
/// (P2.02 RESTRICT rule).
pub fn check_tenant_deletion(portfolio_count: usize) -> Result<(), DeletionRuleError> {
    if portfolio_count > 0 {
        Err(DeletionRuleError::NonEmptyParent {
            entity: "tenants",
            dependent: "portfolios",
            count: portfolio_count,
        })
    } else {
        Ok(())
    }
}

/// Guard: a **portfolio** may only be deleted when it owns no sites (P2.02
/// RESTRICT rule).
pub fn check_portfolio_deletion(site_count: usize) -> Result<(), DeletionRuleError> {
    if site_count > 0 {
        Err(DeletionRuleError::NonEmptyParent {
            entity: "portfolios",
            dependent: "sites",
            count: site_count,
        })
    } else {
        Ok(())
    }
}

/// Guard: a **site** may only be deleted when it owns no assets (P2.02
/// RESTRICT rule).
pub fn check_site_deletion(asset_count: usize) -> Result<(), DeletionRuleError> {
    if asset_count > 0 {
        Err(DeletionRuleError::NonEmptyParent {
            entity: "sites",
            dependent: "assets",
            count: asset_count,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_restrict_rule_rejects_every_non_empty_parent() {
        let err = check_tenant_deletion(1).unwrap_err();
        assert_eq!(
            err,
            DeletionRuleError::NonEmptyParent {
                entity: "tenants",
                dependent: "portfolios",
                count: 1,
            }
        );
        let err = check_portfolio_deletion(3).unwrap_err();
        assert_eq!(
            err,
            DeletionRuleError::NonEmptyParent {
                entity: "portfolios",
                dependent: "sites",
                count: 3,
            }
        );
        let err = check_site_deletion(7).unwrap_err();
        assert_eq!(
            err,
            DeletionRuleError::NonEmptyParent {
                entity: "sites",
                dependent: "assets",
                count: 7,
            }
        );
    }

    #[test]
    fn empty_parents_and_leaves_pass_the_guard() {
        assert_eq!(check_tenant_deletion(0), Ok(()));
        assert_eq!(check_portfolio_deletion(0), Ok(()));
        assert_eq!(check_site_deletion(0), Ok(()));
    }

    #[test]
    fn the_guard_error_names_the_rule_in_its_display() {
        let err = check_site_deletion(2).unwrap_err();
        let text = err.to_string();
        assert!(text.contains("sites"), "{text}");
        assert!(text.contains("assets"), "{text}");
        assert!(text.contains("RESTRICT"), "{text}");
    }
}
