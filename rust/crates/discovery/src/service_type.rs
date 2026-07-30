//! The DNS-SD service type — validated at construction, product-agnostic by default.
//!
//! `mdns-sd` panics or errors on a malformed type string, and a type is not something a node can
//! renegotiate after boot: a typo means the node advertises into a namespace nothing browses, and
//! it fails SILENTLY (an empty roster looks exactly like an empty network). Validating here turns
//! that into a boot error naming the field.

use crate::error::DiscoveryError;

/// The default service type. Deliberately **generic**: `lb` is the platform core and does not know
/// which product embeds it (rule 10). A product host sets its own via [`ServiceType::new`].
pub const DEFAULT_SERVICE_TYPE: &str = "_lb._tcp";

/// A validated DNS-SD service type such as `_lb._tcp`.
///
/// The `.local.` domain suffix is appended internally when registering, so callers pass the bare
/// type. RFC 6763 §7 caps the application protocol label at 15 characters *excluding* the leading
/// underscore, and it must be one label — this type enforces both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceType(String);

impl ServiceType {
    /// Build a service type, rejecting anything DNS-SD would not accept.
    ///
    /// Accepts the two-label `_name._tcp` / `_name._udp` form only.
    pub fn new(ty: impl Into<String>) -> Result<Self, DiscoveryError> {
        let ty = ty.into();
        let (name, proto) = ty
            .split_once('.')
            .ok_or_else(|| DiscoveryError::ServiceType {
                ty: ty.clone(),
                why: "expected the two-label form `_name._tcp` or `_name._udp`",
            })?;

        if proto != "_tcp" && proto != "_udp" {
            return Err(DiscoveryError::ServiceType {
                ty: ty.clone(),
                why: "transport label must be `_tcp` or `_udp`",
            });
        }
        let Some(label) = name.strip_prefix('_') else {
            return Err(DiscoveryError::ServiceType {
                ty: ty.clone(),
                why: "the service label must start with `_`",
            });
        };
        if label.is_empty() || label.len() > 15 {
            return Err(DiscoveryError::ServiceType {
                ty: ty.clone(),
                why: "the service label must be 1..=15 characters after the leading `_` (RFC 6763 §7)",
            });
        }
        // RFC 6763 restricts the label to letters, digits and hyphens; a `.` here would silently
        // add a third label and change the type's shape, which is the failure worth catching.
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(DiscoveryError::ServiceType {
                ty: ty.clone(),
                why: "the service label may contain only ASCII letters, digits and `-`",
            });
        }

        Ok(Self(ty))
    }

    /// The fully-qualified type `mdns-sd` wants: `_lb._tcp.local.`
    pub(crate) fn fqdn(&self) -> String {
        format!("{}.local.", self.0)
    }

    /// The bare type as configured, e.g. `_lb._tcp`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ServiceType {
    fn default() -> Self {
        Self(DEFAULT_SERVICE_TYPE.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_default_and_a_product_type() {
        assert_eq!(ServiceType::default().as_str(), "_lb._tcp");
        assert!(ServiceType::new("_rubix-ai._tcp").is_ok());
        assert!(ServiceType::new("_lb._udp").is_ok());
    }

    #[test]
    fn appends_the_local_domain_for_the_responder() {
        assert_eq!(ServiceType::default().fqdn(), "_lb._tcp.local.");
    }

    #[test]
    fn rejects_malformed_types() {
        for bad in [
            "lb._tcp",                         // missing leading underscore
            "_lb._sctp",                       // not a DNS-SD transport
            "_lb",                             // single label
            "_._tcp",                          // empty label
            "_this-name-is-far-too-long._tcp", // >15 chars
            "_lb.x._tcp",                      // sneaks in a third label
        ] {
            assert!(
                ServiceType::new(bad).is_err(),
                "{bad:?} must be refused before it silently advertises into nowhere"
            );
        }
    }
}
