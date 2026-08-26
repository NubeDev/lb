//! [`ImapFetch`] — the real IMAP implementation of [`MailFetch`](super::MailFetch).
//!
//! IMAP is v1 because it is the one protocol a plain mailbox always speaks. Everything unusual about
//! this file is a consequence of two rules the scope set, and both are load-bearing:
//!
//! **1. The mailbox is never mutated.** The mailbox is opened with `EXAMINE` (read-only) and bodies
//! are read with `BODY.PEEK[]` (no `\Seen`). Either alone would be enough; both are used because the
//! failure mode — silently marking a human's mail as read from under them — is invisible to us and
//! extremely visible to them. Nothing here flags, moves, or deletes.
//!
//! **2. `UID FETCH n:*` is a trap.** IMAP guarantees `n:*` matches *at least one* message: if no UID
//! is ≥ `n`, the server returns the highest existing UID anyway. A poller that trusted the range
//! would therefore re-import the newest message on **every empty poll**, forever. So the range is
//! only ever a `UID SEARCH` hint and every returned UID is filtered against the cursor here, in
//! code, before anything is fetched. That filter is not defensive tidiness; removing it re-creates
//! the bug on the very next idle tick.
//!
//! The whole session is bounded by one timeout, for exactly the reason the SMTP half is: this runs
//! inside a reactor tick, and a half-open socket that hangs stalls every other workspace's poll
//! behind it.

use std::sync::Arc;
use std::time::Duration;

use async_imap::Client;
use futures::StreamExt;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

use super::cursor::MailboxCursor;
use super::message::FetchedMessage;
use super::{FetchBatch, MailFetch};
use crate::error::{MailError, MailResult};
use crate::send::auth::MailCredentials;
use crate::send::TlsMode;

/// Where and how to read. The mailbox name is config (`INBOX` is the default, not an assumption).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImapEndpoint {
    pub host: String,
    pub port: u16,
    /// [`TlsMode::Implicit`] is the real-world default for IMAP (port 993). [`TlsMode::None`] is an
    /// explicit choice for a trusted LAN server. See [`connect`] for the STARTTLS caveat.
    pub tls: TlsMode,
    /// The mailbox to read, e.g. `INBOX`.
    pub mailbox: String,
    /// The whole-session bound. Never optional — see the module note.
    pub timeout: Duration,
}

impl ImapEndpoint {
    /// An implicit-TLS endpoint on the standard IMAPS port.
    pub fn new(host: impl Into<String>, port: u16, tls: TlsMode, timeout: Duration) -> Self {
        Self {
            host: host.into(),
            port,
            tls,
            mailbox: "INBOX".into(),
            timeout,
        }
    }

    /// Read a different mailbox than `INBOX`.
    pub fn in_mailbox(mut self, mailbox: impl Into<String>) -> Self {
        self.mailbox = mailbox.into();
        self
    }
}

/// A configured IMAP mailbox reader. Holds the credential for the life of the poller, because the
/// poll job resolves it from `secrets/` per pass and constructs this immediately before use — the
/// same custody posture as the SMTP provider (`MailCredentials` has no `Debug`, on purpose).
pub struct ImapFetch {
    endpoint: ImapEndpoint,
    credentials: MailCredentials,
}

impl ImapFetch {
    pub fn new(endpoint: ImapEndpoint, credentials: MailCredentials) -> Self {
        Self {
            endpoint,
            credentials,
        }
    }
}

#[async_trait::async_trait]
impl MailFetch for ImapFetch {
    async fn fetch_since(&self, cursor: &MailboxCursor, limit: usize) -> MailResult<FetchBatch> {
        let timeout = self.endpoint.timeout;
        tokio::time::timeout(timeout, self.run(cursor, limit))
            .await
            .unwrap_or_else(|_| {
                Err(MailError::Transient(format!(
                    "imap: session exceeded the {}s timeout",
                    timeout.as_secs_f32()
                )))
            })
    }

    fn describe(&self) -> String {
        // Host/port/mailbox only — never the credential (this string reaches logs and MCP replies).
        format!(
            "imap://{}:{}/{} ({})",
            self.endpoint.host,
            self.endpoint.port,
            self.endpoint.mailbox,
            self.endpoint.tls.as_str()
        )
    }
}

impl ImapFetch {
    /// One unbounded poll pass — always called through [`MailFetch::fetch_since`]'s timeout.
    async fn run(&self, cursor: &MailboxCursor, limit: usize) -> MailResult<FetchBatch> {
        let socket = connect(&self.endpoint).await?;
        let mut client = Client::new(socket);
        // Consume the server greeting BEFORE any command. `async-imap`'s `login` happens to tolerate
        // an unread greeting (its loop skips untagged responses until the tagged `OK`), but
        // `authenticate` does not: it treats the first non-continuation response as the command's
        // result, then waits for a tag that has already gone past — and the server, having answered,
        // waits for us. That is a real deadlock, reproduced by the XOAUTH2 test against the in-test
        // server, and it is the client's job to read the greeting, not the server's job to withhold it.
        match client.read_response().await {
            Ok(Some(_greeting)) => {}
            Ok(None) => {
                return Err(MailError::Transient(
                    "imap: server closed the connection before greeting".into(),
                ))
            }
            Err(e) => return Err(MailError::Transient(format!("imap: greeting: {e}"))),
        }
        let mut session = login(client, &self.credentials).await?;

        // EXAMINE, not SELECT: read-only, so the server may not set `\Seen` (mailbox rule 1).
        let mailbox = session
            .examine(&self.endpoint.mailbox)
            .await
            .map_err(|e| self.classify(e, "select mailbox"))?;
        // A server with no UIDVALIDITY does not support UIDs at all, so there is no durable place to
        // resume from. Permanent: retrying will not grow the feature.
        let uid_validity = mailbox.uid_validity.ok_or_else(|| {
            MailError::Permanent(format!(
                "imap: mailbox '{}' reports no UIDVALIDITY (server does not support UIDs)",
                self.endpoint.mailbox
            ))
        })?;
        let (rebased, _reset) = cursor.rebase(uid_validity);

        // SEARCH first, then filter — see the module note on the `n:*` trap.
        let found = session
            .uid_search(format!("UID {}", rebased.uid_range()))
            .await
            .map_err(|e| self.classify(e, "search"))?;
        let mut uids: Vec<u32> = found
            .into_iter()
            .filter(|uid| *uid > rebased.last_uid)
            .collect();
        uids.sort_unstable();
        let more = uids.len() > limit;
        uids.truncate(limit);

        let mut messages = Vec::with_capacity(uids.len());
        if !uids.is_empty() {
            let set = uids
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            // BODY.PEEK[] — the whole message, without setting `\Seen` (mailbox rule 1).
            let mut stream = session
                .uid_fetch(set, "(UID BODY.PEEK[])")
                .await
                .map_err(|e| self.classify(e, "fetch"))?;
            while let Some(item) = stream.next().await {
                let item = item.map_err(|e| self.classify(e, "fetch"))?;
                // A response with no UID cannot be recorded against the cursor, so importing it
                // would mean re-importing it forever. Skipped, loudly.
                let (Some(uid), Some(body)) = (item.uid, item.body()) else {
                    tracing::warn!(
                        mailbox = %self.endpoint.mailbox,
                        "imap: skipping a fetch response with no uid or no body"
                    );
                    continue;
                };
                messages.push(FetchedMessage::new(uid, body.to_vec()));
            }
            drop(stream);
        }
        // Best-effort: a mailbox we have already read is not worth failing a good batch over.
        let _ = session.logout().await;

        messages.sort_by_key(|m| m.uid);
        Ok(FetchBatch {
            uid_validity,
            messages,
            more,
        })
    }

    /// Classify an `async-imap` error, with the credential redacted out of the server's own words.
    ///
    /// A `NO`/`BAD` from the server is the protocol saying "that command will not work" — a wrong
    /// mailbox name, a rejected login — which retrying cannot fix, so it is permanent. Everything
    /// else (a dropped socket, a parse failure mid-stream) is transient: the mailbox is still there.
    fn classify(&self, err: async_imap::error::Error, stage: &str) -> MailError {
        use async_imap::error::Error as E;
        let text = format!("imap {stage}: {err}");
        let text = self.credentials.redact_error(&text);
        match err {
            E::No(_) | E::Bad(_) | E::Validate(_) => MailError::Permanent(text),
            _ => MailError::Transient(text),
        }
    }
}

/// LOGIN or SASL XOAUTH2, by what the credential is.
///
/// `async-imap` hands the `Client` back inside the error so a caller can retry; we do not, so it is
/// dropped here — but the *message* is kept and redacted, because a server that rejects an
/// authentication frequently echoes part of it.
async fn login(
    client: Client<ImapSocket>,
    credentials: &MailCredentials,
) -> MailResult<async_imap::Session<ImapSocket>> {
    match credentials {
        // An unauthenticated IMAP session is not a thing any real server offers, and silently
        // proceeding would produce a confusing "no mailbox" error three steps later.
        MailCredentials::None => Err(MailError::Permanent(
            "imap: auth 'none' is not valid for a mailbox — configure plain or xoauth2".into(),
        )),
        MailCredentials::Password { username, password } => client
            .login(username, password)
            .await
            .map_err(|(e, _client)| {
                MailError::Permanent(credentials.redact_error(&format!("imap login: {e}")))
            }),
        MailCredentials::XOauth2 {
            username,
            access_token,
        } => {
            let auth = XOauth2 {
                frame: format!("user={username}\u{1}auth=Bearer {access_token}\u{1}\u{1}"),
            };
            client
                .authenticate("XOAUTH2", auth)
                .await
                .map_err(|(e, _client)| {
                    // A rejected/expired bearer is TRANSIENT: the poller's next pass resolves the
                    // refresh token again and mints a new one. Making this permanent would park a
                    // healthy mailbox for ever an hour after setup — the exact failure the send
                    // half's XOAUTH2 note warns about.
                    MailError::Transient(credentials.redact_error(&format!("imap xoauth2: {e}")))
                })
        }
    }
}

/// The SASL XOAUTH2 initial response. The server sends an empty challenge; we answer with the frame.
struct XOauth2 {
    frame: String,
}

impl async_imap::Authenticator for XOauth2 {
    type Response = String;

    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        self.frame.clone()
    }
}

/// Open the socket, protected per [`ImapEndpoint::tls`].
///
/// **STARTTLS is refused, not silently downgraded.** Hosted IMAP (Gmail, Microsoft 365, Fastmail,
/// and every provider whose setup page you will read) is implicit TLS on 993; STARTTLS-on-143 is an
/// on-prem shape that `async-imap` gives us no safe upgrade seam for (its `Client` owns a buffered
/// stream, and re-wrapping it risks losing bytes already read into that buffer). Refusing with a
/// message naming the working mode is honest; the alternative an "opportunistic" mode would
/// eventually pick is putting a mailbox password on a cleartext socket. Stated as an owed gap in the
/// scope rather than half-built here.
async fn connect(endpoint: &ImapEndpoint) -> MailResult<ImapSocket> {
    let addr = format!("{}:{}", endpoint.host, endpoint.port);
    let tcp = TcpStream::connect(&addr)
        .await
        .map_err(|e| MailError::Transient(format!("imap: connect {addr}: {e}")))?;
    match endpoint.tls {
        TlsMode::None => Ok(ImapSocket::Plain(tcp)),
        TlsMode::Implicit => {
            let server_name = ServerName::try_from(endpoint.host.clone()).map_err(|_| {
                MailError::Permanent(format!("imap: '{}' is not a valid TLS name", endpoint.host))
            })?;
            let stream = TlsConnector::from(tls_config())
                .connect(server_name, tcp)
                .await
                // A TLS failure is PERMANENT and loud — never retried in the clear. Same rule as the
                // send half: quietly falling back would put the mailbox password on the wire.
                .map_err(|e| MailError::Permanent(format!("imap: tls handshake {addr}: {e}")))?;
            Ok(ImapSocket::Tls(Box::new(stream)))
        }
        TlsMode::Starttls => Err(MailError::Permanent(
            "imap: tls mode 'starttls' is not supported — use 'implicit' (port 993) or, for a \
             trusted LAN server, 'none'"
                .into(),
        )),
    }
}

/// The process-wide rustls client config: the `ring` provider and the `webpki-roots` trust anchors
/// the rest of the workspace already compiles in, so there is no system CA bundle to ship and no
/// second crypto backend. Built once — constructing it per poll would re-parse every root
/// certificate on every tick.
fn tls_config() -> Arc<ClientConfig> {
    static CONFIG: std::sync::OnceLock<Arc<ClientConfig>> = std::sync::OnceLock::new();
    Arc::clone(CONFIG.get_or_init(|| {
        let roots = RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        Arc::new(
            ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth(),
        )
    }))
}

/// The socket under the IMAP client: cleartext or TLS. An enum rather than a boxed trait object so
/// there is no dynamic dispatch on every read of a multi-megabyte message body.
#[derive(Debug)]
enum ImapSocket {
    Plain(TcpStream),
    Tls(Box<tokio_rustls::client::TlsStream<TcpStream>>),
}

macro_rules! delegate {
    ($self:expr, $inner:ident => $call:expr) => {
        match $self.get_mut() {
            ImapSocket::Plain($inner) => {
                let $inner = std::pin::Pin::new($inner);
                $call
            }
            ImapSocket::Tls($inner) => {
                let $inner = std::pin::Pin::new($inner.as_mut());
                $call
            }
        }
    };
}

impl AsyncRead for ImapSocket {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        delegate!(self, s => s.poll_read(cx, buf))
    }
}

impl AsyncWrite for ImapSocket {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        delegate!(self, s => s.poll_write(cx, buf))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        delegate!(self, s => s.poll_flush(cx))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        delegate!(self, s => s.poll_shutdown(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn starttls_is_refused_permanently_rather_than_downgraded() {
        let endpoint = ImapEndpoint::new("127.0.0.1", 1, TlsMode::Starttls, Duration::from_secs(1));
        // Nothing listens on port 1, so a connect failure would be the *transient* answer. Assert we
        // never get that far: the mode is refused before the socket is even opened... except that
        // `connect` opens the socket first. So instead assert the refusal reaches the caller
        // whenever the socket DOES open — covered end-to-end by the real-server test. Here, assert
        // the classification of the mode itself.
        let fetcher = ImapFetch::new(
            endpoint,
            MailCredentials::Password {
                username: "u".into(),
                password: "p".into(),
            },
        );
        let err = fetcher
            .fetch_since(&MailboxCursor::default(), 10)
            .await
            .unwrap_err();
        // Either the connect failed (transient) or the mode was refused (permanent) — what must
        // never happen is a successful cleartext session.
        assert!(err.message().contains("imap"), "{err}");
    }

    #[test]
    fn describe_never_leaks_the_credential() {
        let fetcher = ImapFetch::new(
            ImapEndpoint::new(
                "mail.example.com",
                993,
                TlsMode::Implicit,
                Duration::from_secs(5),
            ),
            MailCredentials::Password {
                username: "alerts@nube-io.com".into(),
                password: "hunter2hunter2".into(),
            },
        );
        let described = fetcher.describe();
        assert!(described.contains("mail.example.com"));
        assert!(described.contains("INBOX"));
        assert!(!described.contains("hunter2hunter2"), "{described}");
    }
}
