//! A **real SMTP server** on a real socket, for the transport tests.
//!
//! Not a mock (testing-scope §0): it speaks the protocol over TCP, and the thing under test is our
//! client's bytes going out and the server's replies coming back. The sanctioned
//! `RecordingEmailProvider` fake sits far above this — asserting our own recorder proves nothing about
//! AUTH framing or MIME structure, which is exactly what this listener does prove: it hands back the
//! AUTH line and the `DATA` bytes it actually received, so the tests parse the real message.
//!
//! Deliberately minimal ESMTP: greeting, `EHLO` with an advertised mechanism list, `AUTH`,
//! `MAIL FROM`, `RCPT TO`, `DATA` … `.`, `QUIT`. Replies are **scriptable** so the client's error
//! mapping (4xx retry vs 5xx permanent, an auth failure that echoes the credential) is exercised
//! against a server that really says those things.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// How the server should behave for one test.
#[derive(Clone)]
pub struct Script {
    /// The mechanisms advertised on `EHLO` (`"PLAIN LOGIN XOAUTH2"`); empty ⇒ no `AUTH` line.
    pub auth_mechanisms: String,
    /// Advertise `STARTTLS`? (`false` proves the client refuses to continue in the clear.)
    pub advertise_starttls: bool,
    /// The reply to `AUTH …`. `None` ⇒ `235 2.7.0 Authentication successful`.
    pub auth_reply: Option<String>,
    /// Echo the received AUTH argument back inside the auth reply — the credential-disclosure a
    /// chatty relay really does produce, so the redaction is proven against it.
    pub echo_auth_credential: bool,
    /// The reply to `RCPT TO`. `None` ⇒ `250 Ok`.
    pub rcpt_reply: Option<String>,
    /// The reply after the message body. `None` ⇒ `250 2.0.0 Ok: queued`.
    pub data_reply: Option<String>,
    /// Accept the connection and then say nothing at all — the hung session a per-send timeout exists
    /// for.
    pub silent: bool,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            auth_mechanisms: "PLAIN LOGIN XOAUTH2".into(),
            advertise_starttls: false,
            auth_reply: None,
            echo_auth_credential: false,
            rcpt_reply: None,
            data_reply: None,
            silent: false,
        }
    }
}

/// What the server actually received.
#[derive(Debug, Default, Clone)]
pub struct Received {
    /// The full `AUTH <mech> <base64>` command line, verbatim.
    pub auth_line: Option<String>,
    pub mail_from: Option<String>,
    pub rcpt_to: Vec<String>,
    /// The `DATA` payload (headers + body) with dot-unstuffing NOT applied — the raw wire bytes.
    pub message: Option<Vec<u8>>,
}

/// A running test server. Drop it and the accept loop stops.
pub struct TestSmtpServer {
    pub addr: SocketAddr,
    received: Arc<Mutex<Received>>,
    _task: tokio::task::JoinHandle<()>,
}

impl TestSmtpServer {
    /// Bind on an ephemeral localhost port and serve exactly one connection per accept, forever.
    pub async fn start(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let received = Arc::new(Mutex::new(Received::default()));
        let task_received = received.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let script = script.clone();
                let received = task_received.clone();
                tokio::spawn(async move {
                    let _ = serve(stream, script, received).await;
                });
            }
        });
        Self {
            addr,
            received,
            _task: task,
        }
    }

    pub fn host(&self) -> String {
        self.addr.ip().to_string()
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub fn received(&self) -> Received {
        self.received.lock().unwrap().clone()
    }
}

async fn serve(
    mut stream: TcpStream,
    script: Script,
    received: Arc<Mutex<Received>>,
) -> std::io::Result<()> {
    if script.silent {
        // Never greet: the client must hit its own timeout rather than wait forever.
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        return Ok(());
    }
    stream.write_all(b"220 test.lb ESMTP ready\r\n").await?;

    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut in_data = false;
    let mut data_bytes: Vec<u8> = Vec::new();

    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);

        loop {
            if in_data {
                // The body ends at a bare "." on its own line.
                if let Some(pos) = find(&buf, b"\r\n.\r\n") {
                    data_bytes.extend_from_slice(&buf[..pos]);
                    buf.drain(..pos + 5);
                    in_data = false;
                    received.lock().unwrap().message = Some(std::mem::take(&mut data_bytes));
                    let reply = script
                        .data_reply
                        .clone()
                        .unwrap_or_else(|| "250 2.0.0 Ok: queued as ABC123".into());
                    stream.write_all(format!("{reply}\r\n").as_bytes()).await?;
                    continue;
                }
                // Keep everything but a possible partial terminator.
                let keep = buf.len().saturating_sub(4);
                data_bytes.extend_from_slice(&buf[..keep]);
                buf.drain(..keep);
                break;
            }

            let Some(pos) = find(&buf, b"\r\n") else {
                break;
            };
            let line = String::from_utf8_lossy(&buf[..pos]).to_string();
            buf.drain(..pos + 2);
            let upper = line.to_ascii_uppercase();

            if upper.starts_with("EHLO") || upper.starts_with("LHLO") || upper.starts_with("HELO") {
                let mut reply = String::from("250-test.lb greets you\r\n");
                if script.advertise_starttls {
                    reply.push_str("250-STARTTLS\r\n");
                }
                if !script.auth_mechanisms.is_empty() {
                    reply.push_str(&format!("250-AUTH {}\r\n", script.auth_mechanisms));
                }
                reply.push_str("250 SIZE 10485760\r\n");
                stream.write_all(reply.as_bytes()).await?;
            } else if upper.starts_with("AUTH") {
                received.lock().unwrap().auth_line = Some(line.clone());
                let mut reply = script
                    .auth_reply
                    .clone()
                    .unwrap_or_else(|| "235 2.7.0 Authentication successful".into());
                if script.echo_auth_credential {
                    // The chatty-relay disclosure: the server quotes the credential blob back.
                    let arg = line.split_whitespace().nth(2).unwrap_or_default();
                    reply = format!("{reply} (received: {arg})");
                }
                stream.write_all(format!("{reply}\r\n").as_bytes()).await?;
            } else if upper.starts_with("MAIL FROM") {
                received.lock().unwrap().mail_from = Some(line.clone());
                stream.write_all(b"250 2.1.0 Ok\r\n").await?;
            } else if upper.starts_with("RCPT TO") {
                received.lock().unwrap().rcpt_to.push(line.clone());
                let reply = script
                    .rcpt_reply
                    .clone()
                    .unwrap_or_else(|| "250 2.1.5 Ok".into());
                stream.write_all(format!("{reply}\r\n").as_bytes()).await?;
            } else if upper.starts_with("DATA") {
                stream
                    .write_all(b"354 End data with <CR><LF>.<CR><LF>\r\n")
                    .await?;
                in_data = true;
            } else if upper.starts_with("QUIT") {
                stream.write_all(b"221 2.0.0 Bye\r\n").await?;
                return Ok(());
            } else if upper.starts_with("RSET") || upper.starts_with("NOOP") {
                stream.write_all(b"250 2.0.0 Ok\r\n").await?;
            } else {
                stream.write_all(b"502 5.5.2 Not implemented\r\n").await?;
            }
        }
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
