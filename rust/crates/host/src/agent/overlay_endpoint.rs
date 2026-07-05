//! `overlay_config_endpoint` — merge the workspace's live `agent.config.model_endpoint` selection
//! layer over an agent definition's `model_endpoint` preset (active-agent-wiring scope; the read-side
//! twin of the catalog's `setActiveKey` write).
//!
//! **Why this exists.** A built-in definition is read-only: its endpoint (provider/model/env-name)
//! comes verbatim from the seeded `agents.toml` and carries **no** `api_key_secret`. But a workspace
//! can still key its *pick* of that built-in — the UI's "Set model key" seals a token and writes the
//! resulting PATH onto `agent.config.model_endpoint.api_key_secret` (the config is workspace-scoped and
//! *can* own a sealed secret path). Without this overlay, [`resolve_workspace_model`] read the key from
//! the DEFINITION's endpoint only and never saw the config's path — so "model key: set ✓" in the UI
//! resolved to no key at run time. This is the missing read-side half of that contract.
//!
//! **Precedence (decided): config field wins, else inherit the definition.** Each
//! [`ModelEndpointPatch`] field is nullable; a present field overrides the definition's, an absent one
//! leaves the preset's value intact. That matches the config's documented merge semantics (a partial
//! patch sets one axis). The typical case overrides only `api_key_secret`, but provider/model/base_url
//! are overlaid the same way so a workspace can retarget its pick without cloning a built-in.
//!
//! Names-only, no secret values: this operates purely on the endpoint SHAPE (a secret PATH is a name).
//! Key resolution still happens downstream in [`resolve_endpoint_key_host`](super::resolve_key).

use super::config::ModelEndpointPatch;
use super::defs::DefinitionEndpoint;

/// Overlay `cfg` (the workspace's `agent.config.model_endpoint`, if any) onto `def` (the active
/// definition's endpoint). A present config field wins; an absent one inherits the definition. Returns
/// the definition endpoint unchanged when `cfg` is `None`.
pub fn overlay_config_endpoint(
    def: &DefinitionEndpoint,
    cfg: Option<&ModelEndpointPatch>,
) -> DefinitionEndpoint {
    let Some(cfg) = cfg else {
        return def.clone();
    };
    DefinitionEndpoint {
        // provider/model are required on a definition — a config override is honored only when present
        // and non-empty, else the preset's value stands (never blank out a built-in's provider).
        provider: pick(cfg.provider.as_deref(), &def.provider),
        model: pick(cfg.model.as_deref(), &def.model),
        // Optional names: config wins when present, else inherit. `api_key_secret` is the whole point —
        // a workspace's sealed key path for its pick of a (possibly built-in) definition.
        api_key_secret: cfg
            .api_key_secret
            .clone()
            .or_else(|| def.api_key_secret.clone()),
        api_key_env: cfg.api_key_env.clone().or_else(|| def.api_key_env.clone()),
        base_url: cfg.base_url.clone().or_else(|| def.base_url.clone()),
    }
}

/// A required string field: the config override when present and non-empty, else the definition's.
fn pick(over: Option<&str>, base: &str) -> String {
    match over {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => base.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builtin_ep() -> DefinitionEndpoint {
        // Mirrors the seeded `in-house-glm-4.6` built-in: provider/model/env-name, NO sealed key.
        DefinitionEndpoint {
            provider: "zaicoding".into(),
            model: "glm-4.6".into(),
            api_key_env: Some("ZAI_API_KEY".into()),
            api_key_secret: None,
            base_url: None,
        }
    }

    #[test]
    fn none_config_returns_definition_unchanged() {
        let ep = builtin_ep();
        assert_eq!(overlay_config_endpoint(&ep, None), ep);
    }

    #[test]
    fn config_sealed_key_overlays_onto_builtin() {
        // The bug this fixes: a workspace keyed its pick of a read-only built-in. The sealed PATH lives
        // on `agent.config`, not the definition — the overlay must carry it onto the resolved endpoint.
        let cfg = ModelEndpointPatch {
            api_key_secret: Some("agent/config-default-glm-4.6-key".into()),
            ..Default::default()
        };
        let merged = overlay_config_endpoint(&builtin_ep(), Some(&cfg));
        assert_eq!(
            merged.api_key_secret.as_deref(),
            Some("agent/config-default-glm-4.6-key")
        );
        // The preset's provider/model/env are inherited untouched (config left them absent).
        assert_eq!(merged.provider, "zaicoding");
        assert_eq!(merged.model, "glm-4.6");
        assert_eq!(merged.api_key_env.as_deref(), Some("ZAI_API_KEY"));
    }

    #[test]
    fn present_config_field_wins_absent_inherits() {
        let cfg = ModelEndpointPatch {
            model: Some("glm-5.1".into()),
            base_url: Some("https://proxy.example".into()),
            ..Default::default()
        };
        let merged = overlay_config_endpoint(&builtin_ep(), Some(&cfg));
        assert_eq!(merged.model, "glm-5.1"); // config wins
        assert_eq!(merged.base_url.as_deref(), Some("https://proxy.example"));
        assert_eq!(merged.provider, "zaicoding"); // absent in config → inherit
    }

    #[test]
    fn empty_required_override_falls_back_to_definition() {
        // An empty provider/model in the config must NOT blank out the built-in's value.
        let cfg = ModelEndpointPatch {
            provider: Some(String::new()),
            model: Some(String::new()),
            ..Default::default()
        };
        let merged = overlay_config_endpoint(&builtin_ep(), Some(&cfg));
        assert_eq!(merged.provider, "zaicoding");
        assert_eq!(merged.model, "glm-4.6");
    }
}
