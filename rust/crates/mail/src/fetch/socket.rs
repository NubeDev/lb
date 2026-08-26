//! The socket under the IMAP client: opening it, protecting it, and reading/writing through it.
//!
//! Split out of [`imap`](super::imap) so that file can be about the *protocol* — the `n:*` trap, the
//! read-only contract, the cursor filter — and this one about the *transport*. They change for
//! different reasons: a new auth mechanism touches the protocol half, a new TLS posture touches
//! this one.

use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

use super::imap::ImapEndpoint;
use crate::error::{MailError, MailResult};
use crate::send::TlsMode;

/// Open the socket, protected per [`ImapEndpoint::tls`].
///
/// **STARTTLS is refused, not silently downgraded.** Hosted IMAP (Gmail, Microsoft 365, Fastmail,
/// and every provider whose setup page you will read) is implicit TLS on 993; STARTTLS-on-143 is an
/// on-prem shape that `async-imap` gives us no safe upgrade seam for (its `Client` owns a buffered
/// stream, and re-wrapping it risks losing bytes already read into that buffer). Refusing with a
/// message naming the working mode is honest; the alternative an "opportunistic" mode would
/// eventually pick is putting a mailbox password on a cleartext socket. Stated as an owed gap in the
/// scope rather than half-built here.
pub(super) async fn connect(endpoint: &ImapEndpoint) -> MailResult<ImapSocket> {
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
pub(super) enum ImapSocket {
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
