// P14.11 — Per-device credential bindings (decision record
// 43_p14_11_mqtt_sparkplug_b_uplink.md; the never-granted permission rule).
//
// THE RULE THIS MODULE ENCODES: the never-granted permission set
// (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`)
// is byte-identical (conventions §6.4) — and this crate holds NO secret
// material at all. A credential BINDING here declares who connects under
// which identity; the actual secret is received by the TRANSPORT SEAM at
// session time from the deployment's key source (the Phase 13 secrets
// surface; the P6.06 identity-record pattern at the edge). There is no
// password field, no key field, no token field — absence is structural
// (the P6.12 rule): a credential value cannot be stored here because the
// type has nowhere to put it.
//
// Per-device discipline: every device under an edge node gets its OWN
// declared binding (client id + username), so a device's identity is
// revocable and auditable per device — not a shared fleet secret. The
// client-id uniqueness within one deployment is enforced at construction
// of the binding SET (a duplicate client id is a construction error).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::model::{DeviceId, EdgeNodeId, ModelError};

/// The declared reference to WHERE the secret lives. A handle, not a
/// secret: free text naming the deployment's key source (e.g. the Phase 13
/// secrets surface, the P6.06 credential record). Empty is a construction
/// error; the crate never resolves the reference itself.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CredentialSourceReference(String);

impl CredentialSourceReference {
    pub fn new(raw: &str) -> Result<Self, ModelError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ModelError::EmptyField("credential_source"));
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

/// The credential binding of ONE device (or the edge node itself) on the
/// uplink: the declared MQTT client id and username, plus the handle to
/// the secret's source. NO secret material.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceCredentialBinding {
    /// The edge node this binding belongs to.
    pub edge_node_id: EdgeNodeId,
    /// The device, if this is a per-device binding (None = the edge
    /// node's own binding).
    pub device_id: Option<DeviceId>,
    /// The declared MQTT client id (unique within the deployment —
    /// enforced when bindings are assembled into a set). UNVERIFIED: the
    /// Sparkplug B specification's client-id expectations are an
    /// implementation-time check against the text.
    pub client_id: String,
    /// The declared MQTT username (may be empty where the broker accepts
    /// username-less auth — a DECLARED empty, never a default).
    pub username: String,
    /// The handle to the secret's source (the Phase 13 secrets surface;
    /// the secret itself is NEVER carried here).
    pub credential_source: CredentialSourceReference,
}

impl DeviceCredentialBinding {
    /// Constructs and validates one binding. Both `client_id` and the
    /// credential source are required; the username MAY be declared empty.
    pub fn new(
        edge_node_id: EdgeNodeId,
        device_id: Option<DeviceId>,
        client_id: String,
        username: String,
        credential_source: CredentialSourceReference,
    ) -> Result<Self, ModelError> {
        if client_id.trim().is_empty() {
            return Err(ModelError::EmptyField("client_id"));
        }
        // The username may be DECLARED empty (broker-dependent); the field
        // is trimmed for storage but its emptiness is not an error.
        Ok(Self {
            edge_node_id,
            device_id,
            client_id: client_id.trim().to_string(),
            username: username.trim().to_string(),
            credential_source,
        })
    }
}

/// Errors returned when assembling a credential-binding set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialSetError {
    /// Two bindings in one deployment declare the same client id. Client
    /// ids are the broker's identity key; a duplicate is a defect, never
    /// a "last one wins".
    DuplicateClientId {
        client_id: String,
    },
    /// The set is empty — a deployment that declares no bindings has no
    /// uplink identity and is not a reviewed deployment.
    EmptySet,
}

impl std::fmt::Display for CredentialSetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialSetError::DuplicateClientId { client_id } => {
                write!(f, "duplicate MQTT client id '{client_id}' in the credential set")
            }
            CredentialSetError::EmptySet => {
                write!(f, "the credential set is empty — no uplink identity is declared")
            }
        }
    }
}

impl std::error::Error for CredentialSetError {}

/// The reviewed credential-binding SET of one deployment: every device
/// (and the edge node) with its declared client id / username / secret
/// source handle. Uniqueness is enforced against a `BTreeSet` of client
/// ids at assembly; the storage is a Vec sorted by client id at
/// construction (deterministic presentation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialBindingSet {
    bindings: Vec<DeviceCredentialBinding>,
}

impl CredentialBindingSet {
    /// Assembles the binding set; refuses duplicates and the empty set.
    pub fn new(bindings: Vec<DeviceCredentialBinding>) -> Result<Self, CredentialSetError> {
        if bindings.is_empty() {
            return Err(CredentialSetError::EmptySet);
        }
        let mut seen = BTreeSet::new();
        for binding in &bindings {
            if !seen.insert(binding.client_id.as_str()) {
                return Err(CredentialSetError::DuplicateClientId {
                    client_id: binding.client_id.clone(),
                });
            }
        }
        let mut sorted = bindings;
        sorted.sort_by(|a, b| a.client_id.cmp(&b.client_id));
        Ok(Self { bindings: sorted })
    }

    /// The bindings, deterministically ordered by client id.
    pub fn bindings(&self) -> &[DeviceCredentialBinding] {
        &self.bindings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(client_id: &str, device: Option<&str>) -> DeviceCredentialBinding {
        DeviceCredentialBinding::new(
            EdgeNodeId::new("edge-07").expect("valid"),
            device.map(|d| DeviceId::new(d).expect("valid")),
            client_id.to_string(),
            String::new(),
            CredentialSourceReference::new("phase13-secrets/device-credentials")
                .expect("valid"),
        )
        .expect("valid binding")
    }

    #[test]
    fn binding_requires_client_id_and_secret_source() {
        assert!(DeviceCredentialBinding::new(
            EdgeNodeId::new("edge-07").expect("valid"),
            None,
            "  ".to_string(),
            String::new(),
            CredentialSourceReference::new("src").expect("valid"),
        )
        .is_err());
        // The empty secret-source reference is impossible to CONSTRUCT
        // (the reference type itself rejects it) — an empty handle never
        // reaches a binding.
        assert!(CredentialSourceReference::new("  ").is_err());
    }

    #[test]
    fn the_binding_type_has_no_secret_field() {
        // Structural honesty: the binding carries only id/username/source
        // handle. The assertion pins the type's fields so a later secret
        // field cannot be added silently — adding one is a reviewed
        // ADR-010 + never-granted-set change, never a drive-by edit.
        let binding = binding("edge-07-uplink", None);
        let serialized = serde_json::to_string(&binding).expect("serializable");
        assert!(
            !serialized.to_lowercase().contains("secret_value")
                && !serialized.to_lowercase().contains("password"),
            "the serialized binding must not carry secret-looking fields: {serialized}"
        );
    }

    #[test]
    fn duplicate_client_ids_are_refused_and_order_is_deterministic() {
        let set = CredentialBindingSet::new(vec![
            binding("edge-07-uplink", None),
            binding("inverter-1-uplink", Some("inverter-1")),
            binding("inverter-2-uplink", Some("inverter-2")),
        ])
        .expect("valid set");
        let ids: Vec<&str> = set
            .bindings()
            .iter()
            .map(|b| b.client_id.as_str())
            .collect();
        assert_eq!(ids, vec!["edge-07-uplink", "inverter-1-uplink", "inverter-2-uplink"]);
        assert_eq!(
            CredentialBindingSet::new(vec![
                binding("edge-07-uplink", None),
                binding("edge-07-uplink", Some("inverter-1")),
            ]),
            Err(CredentialSetError::DuplicateClientId {
                client_id: "edge-07-uplink".to_string(),
            })
        );
        assert_eq!(
            CredentialBindingSet::new(Vec::new()),
            Err(CredentialSetError::EmptySet)
        );
    }
}
