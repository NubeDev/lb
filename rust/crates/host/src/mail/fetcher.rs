//! Build a live [`MailFetch`] from a stored [`MailSource`] — the one place a credential VALUE
//! exists in this service.
//!
//! Resolved **per pass, in the source's own workspace**, and dropped when the pass returns. The
//! precedence is the platform's (`agent/resolve_key.rs`, the SMTP provider, the update credential):
//! **sealed workspace secret → node env → unset**, and `unset` where the mechanism needs one is a
//! *permanent* error rather than a retry — a typo'd secret path will not resolve on the fifth
//! attempt, and the operator needs the roster to say so.
//!
//! The workspace comes from the caller and is never defaulted (rule 6). That is the exact bug
//! `push_target` shipped with (`debugging/inbox-outbox/push-target-hardcoded-workspace.md`): a
//! hardcoded workspace in a delivery path meant one tenant's effect resolved another tenant's
//! credential. A mail source resolving ws-B's mailbox password from a ws-A poll would be the same
//! class, one direction over.

use std::time::Duration;

use lb_mail::send::auth::{access_token, MailCredentials, RefreshRequest, TokenCache};
use lb_mail::{AuthMechanism, ImapEndpoint, ImapFetch, MailFetch, TlsMode};
use lb_store::Store;

use super::error::MailSourceError;
use super::source::MailSource;

/// The node's shared XOAUTH2 access-token cache.
///
/// Process-global rather than per-call: an access token lives about an hour, and minting a fresh one
/// per poll (or per `mail.source.check`) is exactly the behaviour Google and Microsoft rate-limit.
/// Sharing it across workspaces is safe because the cache key is a **digest of the refresh token**
/// (`lb_mail::send::auth::refresh`), so two workspaces' grants can never collide, and rotating a
/// seal invalidates the bearer minted from the old one.
pub fn token_cache() -> &'static TokenCache {
    static CACHE: std::sync::OnceLock<TokenCache> = std::sync::OnceLock::new();
    CACHE.get_or_init(TokenCache::new)
}

/// The node's shared HTTP client for token refresh. One connection pool, built once.
pub fn http_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

/// The whole-session bound on one poll. Generous enough for a slow relay handing back 25 messages
/// with attachments, short enough that a hung mailbox cannot stall the tick for another workspace.
pub const POLL_TIMEOUT: Duration = Duration::from_secs(60);

/// Build the fetcher for `source` in `ws`.
pub async fn build_fetcher(
    store: &Store,
    ws: &str,
    source: &MailSource,
    tokens: &TokenCache,
    http: &reqwest::Client,
) -> Result<Box<dyn MailFetch>, MailSourceError> {
    let endpoint = ImapEndpoint::new(
        source.host.clone(),
        source.port,
        TlsMode::parse(&source.tls)?,
        POLL_TIMEOUT,
    )
    .in_mailbox(source.mailbox.clone());
    let credentials = credentials(store, ws, source, tokens, http).await?;
    Ok(Box::new(ImapFetch::new(endpoint, credentials)))
}

/// Resolve the credential for one pass.
async fn credentials(
    store: &Store,
    ws: &str,
    source: &MailSource,
    tokens: &TokenCache,
    http: &reqwest::Client,
) -> Result<MailCredentials, MailSourceError> {
    let auth = AuthMechanism::parse(&source.auth)?;
    let secret = resolve_secret(store, ws, &source.secret_path, &source.secret_env)
        .await
        .ok_or_else(|| MailSourceError::Transport {
            // Permanent: a path/env resolving to nothing is a config error, not a blip. Note the
            // message names the PATH and never the value — this string reaches the roster and the log.
            permanent: true,
            message: format!(
                "no credential at secret path '{}' (nor env '{}') for workspace {ws}",
                source.secret_path, source.secret_env
            ),
        })?;

    match auth {
        AuthMechanism::None => Err(MailSourceError::BadInput(
            "a mailbox cannot be read with auth 'none'".into(),
        )),
        AuthMechanism::Plain | AuthMechanism::Login => Ok(MailCredentials::Password {
            username: source.username.clone(),
            password: secret,
        }),
        AuthMechanism::XOauth2 => {
            let oauth = source.oauth.as_ref().ok_or_else(|| {
                MailSourceError::BadInput("auth 'xoauth2' needs an oauth block".into())
            })?;
            let client_secret = resolve_secret(
                store,
                ws,
                &oauth.client_secret_path,
                &oauth.client_secret_env,
            )
            .await
            .unwrap_or_default();
            let request = RefreshRequest {
                token_endpoint: oauth.token_endpoint.clone(),
                client_id: oauth.client_id.clone(),
                client_secret,
                // For xoauth2 the sealed value IS the refresh token (the skill doc's ceremony).
                refresh_token: secret,
            };
            let token = access_token(tokens, http, &request).await?;
            Ok(MailCredentials::XOauth2 {
                username: source.username.clone(),
                access_token: token,
            })
        }
    }
}

/// **Sealed workspace secret → node env → unset.** Duplicated in shape (not in code) with the SMTP
/// provider's resolver on purpose: they are two different config structs reading two different
/// paths, and collapsing them into one shared helper would couple the send and receive halves for
/// the sake of four lines.
async fn resolve_secret(store: &Store, ws: &str, path: &str, env_name: &str) -> Option<String> {
    if let Some(path) = Some(path.trim()).filter(|p| !p.is_empty()) {
        if let Ok(value) = lb_secrets::get_workspace(store, ws, path).await {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    if let Some(name) = Some(env_name.trim()).filter(|n| !n.is_empty()) {
        if let Ok(value) = std::env::var(name) {
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}
