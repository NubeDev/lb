//! [`MailSource`] — the durable description of one watched mailbox.
//!
//! **The record holds names, never values.** `secret_path` / `secret_env` say *where* the credential
//! lives; the credential itself is resolved inside the poll, in the source's own workspace, and
//! dropped when the pass returns ([`super::fetcher`]). This is the same posture the SMTP provider
//! keeps, and it is what makes `mail.source.list` safe to expose at all — the scope's "credential
//! custody is the whole game" risk is answered by there being nothing to leak in the record.
//!
//! The cursor lives here too, and that is deliberate rather than lazy: the mail-source scope calls
//! the source record **node-authoritative** (two nodes must not both poll one mailbox), and keeping
//! the cursor on the same record as the config means the thing that claims the mailbox and the thing
//! that remembers its position cannot get separated.

use lb_mail::MailboxCursor;
use serde::{Deserialize, Serialize};

use super::error::MailSourceError;

/// The store table. Reserved (host-owned) — a generic `store.write` may not forge a mail source and
/// thereby aim a poller at an arbitrary host with the workspace's own secrets.
pub const MAIL_SOURCE_TABLE: &str = "mail_source";

/// The default poll cadence. Mail is not latency-critical and providers rate-limit aggressively; a
/// minute is responsive enough for "email your data in" and cheap enough to run per workspace.
pub const DEFAULT_POLL_SECONDS: u64 = 60;

/// The floor on the poll cadence. Not a nicety: Gmail and Microsoft 365 both throttle (and then
/// temporarily lock) an account that connects too often, so a misconfigured `pollSeconds: 1` would
/// take the mailbox offline for everyone using it, not just for us.
pub const MIN_POLL_SECONDS: u64 = 15;

/// The inbox channel imported mail lands on when a source names none.
pub const DEFAULT_CHANNEL: &str = "mail";

/// One watched mailbox.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailSource {
    /// Workspace-unique, caller-supplied. Re-registering the same id updates in place.
    pub id: String,
    /// A human label for the roster.
    #[serde(default)]
    pub name: String,
    /// The fetch protocol. `imap` is the only one implemented; the field exists because
    /// [`MailFetch`](lb_mail::MailFetch) is the seam a Gmail-API or JMAP adapter arrives behind, and
    /// a record written today must still parse when it does.
    #[serde(default = "default_protocol")]
    pub protocol: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// `implicit` (993, the real-world default) or `none` (a trusted LAN server).
    #[serde(default = "default_tls")]
    pub tls: String,
    #[serde(default = "default_mailbox")]
    pub mailbox: String,
    #[serde(default)]
    pub username: String,
    /// `plain` | `login` | `xoauth2`.
    #[serde(default = "default_auth")]
    pub auth: String,
    /// Where the credential is sealed (a path, never a value).
    #[serde(default)]
    pub secret_path: String,
    /// The node env var to fall back to when the sealed path is empty — the same precedence the
    /// SMTP provider uses (`sealed → env → unset`), so a dev node can run from the environment
    /// without a seal ceremony.
    #[serde(default)]
    pub secret_env: String,
    /// The OAuth2 settings for `auth: "xoauth2"`. Paths and ids only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OauthConfig>,
    /// Who may send into this workspace. **Empty means allow every sender** — correct for a mailbox
    /// dedicated to one feed, and stated plainly rather than hidden, because the alternative (an
    /// implicit deny-all default) makes a freshly-registered source silently import nothing and look
    /// broken. Entries are an exact address (`data@example.com`) or a domain (`@example.com` /
    /// `example.com` / `*@example.com`). See [`sender_allowed`].
    #[serde(default)]
    pub allow_senders: Vec<String>,
    #[serde(default = "default_poll_seconds")]
    pub poll_seconds: u64,
    /// The inbox channel arriving mail is projected onto.
    #[serde(default = "default_channel")]
    pub channel: String,
    #[serde(default)]
    pub attachments: AttachmentPolicy,
    /// A paused source keeps its cursor and its history; it simply stops being polled.
    #[serde(default)]
    pub paused: bool,
    /// How far into the mailbox the poller has read.
    #[serde(default)]
    pub cursor: MailboxCursor,
    /// The principal that registered it.
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub created_ts: u64,
    #[serde(default)]
    pub last_poll_ts: u64,
    /// The last poll's failure, for the roster. Already redacted by the transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// Lifetime counters, for the roster and for "is this thing working".
    #[serde(default)]
    pub imported: u64,
    #[serde(default)]
    pub rejected: u64,
}

/// OAuth2 settings for XOAUTH2. Ids and paths only — the refresh token is what the `secret_path`
/// resolves to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OauthConfig {
    pub token_endpoint: String,
    pub client_id: String,
    #[serde(default)]
    pub client_secret_path: String,
    #[serde(default)]
    pub client_secret_env: String,
}

/// What to do with a message's attachments.
///
/// The two switches are separate on purpose. `store_bytes` keeps the file (an audit trail, and the
/// thing a human clicks in the inbox); `ingest` turns it into series data. A workspace that only
/// wants the numbers can turn the first off, and one that receives PDFs it cannot decode still keeps
/// them with the second off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentPolicy {
    /// Keep each attachment as a workspace asset.
    #[serde(default = "yes")]
    pub store_bytes: bool,
    /// Decode matching attachments into series samples.
    #[serde(default = "yes")]
    pub ingest: bool,
    /// The decoder to run: `auto` (identify from the bytes) or a named format id. Opaque here —
    /// `lb_ingest::decode` owns the registry, and this service never branches on the value.
    #[serde(default = "default_format")]
    pub format: String,
    /// Only attachments with one of these (lower-case, dotless) extensions are decoded. Empty ⇒ try
    /// every attachment. A filter, not a security control: it saves work, it does not gate reach.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Prefixed onto every series the decode produces.
    #[serde(default)]
    pub series_prefix: String,
    /// How far ahead of UTC the file's wall-clock timestamps are (`600` for NEM time).
    #[serde(default)]
    pub offset_minutes: i64,
    /// The per-file sample ceiling; `0` ⇒ the decoder default.
    #[serde(default)]
    pub max_samples: usize,
}

impl Default for AttachmentPolicy {
    fn default() -> Self {
        Self {
            store_bytes: true,
            ingest: true,
            format: default_format(),
            extensions: Vec::new(),
            series_prefix: String::new(),
            offset_minutes: 0,
            max_samples: 0,
        }
    }
}

impl AttachmentPolicy {
    /// Should this attachment be handed to a decoder?
    pub fn decodes(&self, extension: &str) -> bool {
        if !self.ingest || self.format.trim().is_empty() {
            return false;
        }
        self.extensions.is_empty()
            || self.extensions.iter().any(|e| {
                e.trim()
                    .trim_start_matches('.')
                    .eq_ignore_ascii_case(extension)
            })
    }
}

impl MailSource {
    /// Validate a source as registered. Rejects the shapes that could never poll, with a message an
    /// operator can act on — the same "fail at create time, with a human watching" discipline the
    /// reminder verbs use, rather than letting the first background tick discover it.
    pub fn validate(&self) -> Result<(), MailSourceError> {
        let bad = |m: &str| Err(MailSourceError::BadInput(m.into()));
        if self.id.trim().is_empty() {
            return bad("a mail source needs an id");
        }
        if self.host.trim().is_empty() {
            return bad("a mail source needs a host");
        }
        if !self.protocol.eq_ignore_ascii_case("imap") {
            return Err(MailSourceError::BadInput(format!(
                "protocol '{}' is not supported (only 'imap' today)",
                self.protocol
            )));
        }
        // Parsed through the transport's own parsers so the accepted spellings cannot drift from
        // what the transport actually understands. Their `MailError` is re-classified as
        // `BadInput` rather than carried through: `From<MailError>` maps to `Transport`, so a
        // typo'd `tls: "tsl"` would have been reported to the operator as "mailbox unreachable",
        // which sends them to look at the network instead of at the field they mistyped.
        lb_mail::TlsMode::parse(&self.tls)
            .map_err(|e| MailSourceError::BadInput(e.message().into()))?;
        let auth = lb_mail::AuthMechanism::parse(&self.auth)
            .map_err(|e| MailSourceError::BadInput(e.message().into()))?;
        if auth == lb_mail::AuthMechanism::None {
            return bad("a mailbox needs credentials — auth 'none' cannot log in");
        }
        if self.username.trim().is_empty() {
            return bad("a mail source needs a username");
        }
        if self.secret_path.trim().is_empty() && self.secret_env.trim().is_empty() {
            return bad("a mail source needs a secretPath (or a secretEnv) — the credential is resolved by NAME, never stored here");
        }
        if auth == lb_mail::AuthMechanism::XOauth2 && self.oauth.is_none() {
            return bad("auth 'xoauth2' needs an oauth block (tokenEndpoint + clientId)");
        }
        if self.mailbox.trim().is_empty() {
            return bad("a mail source needs a mailbox (e.g. INBOX)");
        }
        if self.poll_seconds < MIN_POLL_SECONDS {
            return Err(MailSourceError::BadInput(format!(
                "pollSeconds must be at least {MIN_POLL_SECONDS} — polling faster than that gets \
                 a real mailbox rate-limited or locked"
            )));
        }
        if self.channel.trim().is_empty() {
            return bad("a mail source needs an inbox channel");
        }
        Ok(())
    }

    /// Is `sender` allowed to put mail into this workspace?
    ///
    /// The allowlist is the mail-source scope's answer to "anyone who can email the address can
    /// inject documents into the corpus, and thence into agent context". Matching is on the
    /// already-lower-cased addr-spec, so `Data@Example.COM` cannot slip past an entry.
    pub fn sender_allowed(&self, sender: &str) -> bool {
        if self.allow_senders.is_empty() {
            return true;
        }
        let sender = sender.trim().to_ascii_lowercase();
        let domain = sender.split_once('@').map_or("", |(_, d)| d);
        self.allow_senders.iter().any(|entry| {
            let entry = entry.trim().to_ascii_lowercase();
            let entry = entry.trim_start_matches('*');
            match entry.strip_prefix('@') {
                // A domain rule matches the domain exactly. NOT a suffix match: `@example.com`
                // must not admit `evil-example.com`, which a naive `ends_with` would.
                Some(d) => domain == d,
                None if entry.contains('@') => sender == entry,
                // A bare `example.com` reads as a domain rule — it is what an operator types.
                None => domain == entry,
            }
        })
    }
}

fn yes() -> bool {
    true
}
fn default_protocol() -> String {
    "imap".into()
}
fn default_port() -> u16 {
    993
}
fn default_tls() -> String {
    "implicit".into()
}
fn default_mailbox() -> String {
    "INBOX".into()
}
fn default_auth() -> String {
    "plain".into()
}
fn default_format() -> String {
    lb_ingest::AUTO.into()
}
fn default_poll_seconds() -> u64 {
    DEFAULT_POLL_SECONDS
}
fn default_channel() -> String {
    DEFAULT_CHANNEL.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> MailSource {
        MailSource {
            id: "meter-data".into(),
            name: "Meter data".into(),
            protocol: "imap".into(),
            host: "imap.example.com".into(),
            port: 993,
            tls: "implicit".into(),
            mailbox: "INBOX".into(),
            username: "alerts@nube-io.com".into(),
            auth: "plain".into(),
            secret_path: "mail/inbox-password".into(),
            secret_env: String::new(),
            oauth: None,
            allow_senders: Vec::new(),
            poll_seconds: 60,
            channel: "mail".into(),
            attachments: AttachmentPolicy::default(),
            paused: false,
            cursor: MailboxCursor::default(),
            owner: "user:ada".into(),
            created_ts: 0,
            last_poll_ts: 0,
            last_error: None,
            imported: 0,
            rejected: 0,
        }
    }

    #[test]
    fn an_empty_allowlist_admits_everyone() {
        assert!(source().sender_allowed("anyone@anywhere.com"));
    }

    #[test]
    fn a_domain_rule_is_exact_and_not_a_suffix_match() {
        let mut src = source();
        src.allow_senders = vec!["@example.com".into()];
        assert!(src.sender_allowed("data@example.com"));
        assert!(src.sender_allowed("DATA@Example.COM"));
        assert!(
            !src.sender_allowed("data@evil-example.com"),
            "a suffix match here is a workspace-injection hole"
        );
        assert!(!src.sender_allowed("data@example.com.evil.net"));
    }

    #[test]
    fn the_spellings_an_operator_actually_types_all_work() {
        for entry in ["@example.com", "example.com", "*@example.com"] {
            let mut src = source();
            src.allow_senders = vec![entry.into()];
            assert!(src.sender_allowed("data@example.com"), "entry {entry}");
            assert!(!src.sender_allowed("data@other.com"), "entry {entry}");
        }
    }

    #[test]
    fn an_exact_address_rule_admits_only_that_address() {
        let mut src = source();
        src.allow_senders = vec!["data@example.com".into()];
        assert!(src.sender_allowed("data@example.com"));
        assert!(!src.sender_allowed("other@example.com"));
    }

    #[test]
    fn a_source_that_could_never_poll_is_refused_at_registration() {
        let mut src = source();
        src.secret_path = String::new();
        assert!(src.validate().is_err(), "no credential location");

        let mut src = source();
        src.poll_seconds = 1;
        let err = src.validate().unwrap_err();
        assert!(err.to_string().contains("pollSeconds"), "{err}");

        let mut src = source();
        src.auth = "xoauth2".into();
        assert!(src.validate().is_err(), "xoauth2 with no oauth block");

        let mut src = source();
        src.protocol = "pop3".into();
        assert!(src.validate().is_err());

        let mut src = source();
        src.tls = "tsl".into();
        assert!(
            src.validate().is_err(),
            "a typo'd tls mode must not be accepted"
        );

        assert!(source().validate().is_ok());
    }

    #[test]
    fn the_extension_filter_is_case_and_dot_insensitive() {
        let policy = AttachmentPolicy {
            extensions: vec![".CSV".into()],
            ..Default::default()
        };
        assert!(policy.decodes("csv"));
        assert!(!policy.decodes("pdf"));

        let off = AttachmentPolicy {
            ingest: false,
            ..Default::default()
        };
        assert!(!off.decodes("csv"), "ingest off means no decode at all");
    }
}
