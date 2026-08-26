//! `mail.source.check` — prove the credentials, without importing anything.
//!
//! The setup verb. Registering a mailbox is a config ceremony with several independent ways to be
//! wrong (host, port, TLS mode, username, the secret path, the mailbox name), and the alternative to
//! this verb is "save it and watch the log for a minute". It fetches **one** message and throws it
//! away: the cursor is untouched, nothing is stored, no ledger row is written.
//!
//! It is gated on `mail.source.check` rather than riding the register grant because it *spends an
//! external connection* — that is the same reasoning that made `federation.profile_refresh` its own
//! cap next to a free read.

use lb_auth::Principal;
use lb_mail::send::auth::TokenCache;
use lb_mail::ParsedMail;
use lb_store::Store;
use serde::Serialize;

use super::authorize::authorize_mail_source;
use super::error::MailSourceError;
use super::fetcher::build_fetcher;
use super::source::MailSource;
use super::store::read_source;

/// What `check` found.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub source: String,
    /// The endpoint, credential-free.
    pub endpoint: String,
    pub uid_validity: u32,
    /// Whether the mailbox held anything past the source's current cursor.
    pub has_new: bool,
    /// A peek at the newest unimported message, so an operator can see they are looking at the
    /// right mailbox. Metadata only — no body, no attachment bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest: Option<MessagePeek>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagePeek {
    pub uid: u32,
    pub from: String,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_ms: Option<u64>,
    pub attachments: Vec<String>,
    /// Would this sender be admitted by the allowlist as configured? The single most common reason
    /// a correctly-configured source imports nothing.
    pub sender_allowed: bool,
}

/// Fetch one message from `id`'s mailbox and report what was found. Imports nothing.
pub async fn mail_source_check(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    tokens: &TokenCache,
    http: &reqwest::Client,
) -> Result<CheckResult, MailSourceError> {
    authorize_mail_source(principal, ws, "check")?;
    let source = read_source(store, ws, id)
        .await?
        .ok_or(MailSourceError::NotFound)?;
    check_source(store, ws, &source, tokens, http).await
}

/// [`mail_source_check`] against an un-stored source — so an operator can validate settings BEFORE
/// registering them, and so registration is not a prerequisite for finding out the password is wrong.
///
/// **Imports nothing, advances nothing.** The signature says so — `&MailSource`, never `&mut` — but
/// a signature is not a test, so the guarantee is asserted behaviourally in
/// `mail_import_test::check_reports_the_mailbox_without_importing_or_advancing_anything`: it runs a
/// check against a real IMAP server holding a real message and then asserts the stored cursor is
/// still `MailboxCursor::default()` and the inbox is still empty.
pub async fn check_source(
    store: &Store,
    ws: &str,
    source: &MailSource,
    tokens: &TokenCache,
    http: &reqwest::Client,
) -> Result<CheckResult, MailSourceError> {
    source.validate()?;
    let fetcher = build_fetcher(store, ws, source, tokens, http).await?;
    let batch = fetcher.fetch_since(&source.cursor, 1).await?;

    let newest = batch.messages.last().map(|message| {
        let mail: ParsedMail = lb_mail::parse_message(&message.raw);
        MessagePeek {
            uid: message.uid,
            from: mail.from_address().to_string(),
            subject: mail.subject.clone(),
            date_ms: mail.date_ms,
            attachments: mail
                .attachments
                .iter()
                .map(|a| a.filename.clone())
                .collect(),
            sender_allowed: source.sender_allowed(mail.from_address()),
        }
    });

    Ok(CheckResult {
        source: source.id.clone(),
        endpoint: fetcher.describe(),
        uid_validity: batch.uid_validity,
        has_new: !batch.messages.is_empty() || batch.more,
        newest,
    })
}
