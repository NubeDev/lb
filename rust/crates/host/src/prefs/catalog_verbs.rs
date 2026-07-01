//! The gated i18n-catalog host verbs (i18n-catalogs scope) — the capability chokepoint over the raw
//! `lb_prefs::catalog` render + `message_catalog` store. Three verbs, mirroring the shipped `prefs.*`
//! wiring:
//!   - `message.render` — render a catalog `key` in the recipient's resolved language. Member-level
//!     for the CALLER's own render; rendering FOR ANOTHER recipient (the outbox fan-out) additionally
//!     requires the `message.render_recipient` grant (producing content on their behalf, parallel to
//!     `prefs.get(other)`). The recipient is always resolved in the CALLER's token workspace (a
//!     producer can't render a foreign-ws recipient — the wall holds).
//!   - `prefs.catalog` — read the MERGED (override-over-builtin) catalog for the caller's own ws.
//!   - `message.set_catalog` — admin: merge a sparse override patch, then publish the "catalog
//!     changed" hint (state in the store, the hint is only motion).

use std::collections::BTreeMap;

use lb_auth::Principal;
use lb_bus::Bus;
use lb_prefs::{
    get_catalog_override, lint_catalog, merged_catalog, render_message, resolve_chain,
    set_catalog_override, RenderedMessage,
};
use lb_store::Store;
use serde_json::Value;

use super::catalog_authorize::{authorize_catalog, authorize_render};
use super::catalog_motion::publish_catalog_changed;
use super::error::PrefsSvcError;

/// `message.render` — render `key` with `args` for `recipient` (default: the caller). Resolves the
/// target's prefs in the caller's workspace, loads that language's override, and renders. Never
/// panics/blanks (the fallback + placeholder-failure contracts live in `lb_prefs::catalog`).
pub async fn message_render(
    store: &Store,
    principal: &Principal,
    ws: &str,
    key: &str,
    args: &Value,
    recipient: Option<&str>,
) -> Result<RenderedMessage, PrefsSvcError> {
    // The target user: the named recipient, or the caller. Rendering FOR ANOTHER needs the fan-out
    // grant on TOP of the base render cap — checked in one place so the deny is opaque + total.
    let target = recipient.unwrap_or_else(|| principal.sub());
    let for_another = target != principal.sub();
    authorize_render(principal, ws, for_another)?;

    // Resolve the target's prefs — ALWAYS in the caller's workspace (the wall; a producer cannot
    // name a foreign-ws recipient). No request override on a server-side render.
    let resolved = resolve_chain(store, ws, target, None).await?;
    let (override_, _has) = get_catalog_override(store, ws, &resolved.language).await?;
    Ok(render_message(key, args, &override_, &resolved))
}

/// `prefs.catalog` — the merged override-over-builtin catalog for `locale` in the caller's own ws.
/// An unknown locale merges over the `en` builtin (never empty — the no-block rule).
pub async fn prefs_catalog(
    store: &Store,
    principal: &Principal,
    ws: &str,
    locale: &str,
) -> Result<CatalogView, PrefsSvcError> {
    authorize_catalog(principal, ws, "prefs.catalog")?;
    let (override_, has_override) = get_catalog_override(store, ws, locale).await?;
    let (messages, catalog_version) = merged_catalog(locale, &override_);
    Ok(CatalogView {
        locale: locale.to_string(),
        catalog_version,
        messages,
        has_override,
    })
}

/// `message.set_catalog` (ADMIN) — merge a sparse override patch into `(ws, locale)`, lint it against
/// the MF1 subset FIRST (an out-of-subset message is a bad-input authoring error, never stored), then
/// publish the "catalog changed" hint so open clients re-fetch.
pub async fn message_set_catalog(
    store: &Store,
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    locale: &str,
    messages: BTreeMap<String, String>,
) -> Result<(), PrefsSvcError> {
    authorize_catalog(principal, ws, "message.set_catalog")?;
    // Lint before write: a message outside the pinned subset is rejected here (parallel to the
    // build-time catalog-lint test), so the store never holds an unrenderable override.
    if let Err((key, e)) = lint_catalog(&messages) {
        return Err(PrefsSvcError::BadInput(format!(
            "catalog lint failed for `{key}`: {e}"
        )));
    }
    let merged = set_catalog_override(store, ws, locale, &messages).await?;
    // Fire-and-forget motion: the store holds the state; this only nudges open clients. The version
    // echoed is the builtin stamp for the locale (the human-facing catalog-version).
    let (_m, version) = merged_catalog(locale, &merged);
    publish_catalog_changed(bus, ws, locale, &version).await;
    Ok(())
}

/// The `prefs.catalog` result — the merged map + whether a workspace override exists (the DTO).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CatalogView {
    pub locale: String,
    pub catalog_version: String,
    pub messages: BTreeMap<String, String>,
    pub has_override: bool,
}
