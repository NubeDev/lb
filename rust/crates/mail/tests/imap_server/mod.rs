//! A **real IMAP server** on a real socket, for the receive-half tests.
//!
//! Not a mock (testing-scope §0), and the distinction matters more here than almost anywhere: the
//! things most likely to be wrong in an IMAP client are *protocol* things — the `n:*` range that
//! always matches something, the `{len}` literal framing of a fetched body, whether the mailbox was
//! opened read-only. None of those are observable by asserting our own recorder; all of them are
//! observable by speaking IMAP4rev1 over TCP and handing back what was actually asked for.
//!
//! Deliberately minimal: greeting, `LOGIN` / `AUTHENTICATE XOAUTH2`, `EXAMINE`, `UID SEARCH`,
//! `UID FETCH`, `LOGOUT`. It also **records every command line it received**, which is how the
//! "never mutate the mailbox" contract is proven rather than asserted — the test reads the log and
//! checks no `SELECT`/`STORE` ever went out and that the body was requested with `BODY.PEEK[]`.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

/// One message in the test mailbox.
#[derive(Clone, Debug)]
pub struct StoredMessage {
    pub uid: u32,
    pub raw: Vec<u8>,
}

/// How the server should behave for one test.
#[allow(dead_code)]
#[derive(Clone)]
pub struct Script {
    /// The mailbox's `UIDVALIDITY`. Bump it between two runs to exercise a cursor reset.
    pub uid_validity: u32,
    /// The messages, ascending by UID.
    pub messages: Vec<StoredMessage>,
    /// Reject `LOGIN` with this `NO` text (proves a bad credential is permanent, and redacted).
    pub login_failure: Option<String>,
    /// Answer `EXAMINE` without a `UIDVALIDITY` line — a server with no UID support.
    pub omit_uid_validity: bool,
    /// Accept the connection and then say nothing — the hung session the timeout exists for.
    pub silent: bool,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            uid_validity: 42,
            messages: Vec::new(),
            login_failure: None,
            omit_uid_validity: false,
            silent: false,
        }
    }
}

/// What the server actually received: every command line, in order.
#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
pub struct Received {
    pub commands: Vec<String>,
}

#[allow(dead_code)]
impl Received {
    /// Did any received command line contain `needle` (case-insensitively)?
    pub fn saw(&self, needle: &str) -> bool {
        let needle = needle.to_ascii_uppercase();
        self.commands
            .iter()
            .any(|c| c.to_ascii_uppercase().contains(&needle))
    }
}

/// A running test server. Drop it and the accept loop stops.
///
/// `dead_code` is allowed because this module is included by TWO crates' test suites (`lb-mail`'s
/// own fetch tests and `lb-host`'s import tests, via `#[path]`), and each uses a different part of
/// it. Splitting it to silence the warning would give the two suites two servers to drift apart.
#[allow(dead_code)]
pub struct TestImapServer {
    pub addr: SocketAddr,
    received: Arc<Mutex<Received>>,
    _task: tokio::task::JoinHandle<()>,
}

#[allow(dead_code)]
impl TestImapServer {
    /// Bind an ephemeral port and serve `script` until dropped.
    pub async fn start(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Received::default()));
        let log = Arc::clone(&received);
        let task = tokio::spawn(async move {
            loop {
                let Ok((socket, _)) = listener.accept().await else {
                    return;
                };
                let script = script.clone();
                let log = Arc::clone(&log);
                tokio::spawn(async move {
                    let _ = serve(socket, script, log).await;
                });
            }
        });
        Self {
            addr,
            received,
            _task: task,
        }
    }

    pub fn received(&self) -> Received {
        self.received.lock().expect("received").clone()
    }
}

/// One IMAP session.
async fn serve(
    socket: TcpStream,
    script: Script,
    log: Arc<Mutex<Received>>,
) -> std::io::Result<()> {
    if script.silent {
        // Hold the socket open, saying nothing at all.
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        return Ok(());
    }
    let (read_half, mut write) = socket.into_split();
    let mut reader = BufReader::new(read_half);
    write
        .write_all(b"* OK [CAPABILITY IMAP4rev1 AUTH=XOAUTH2] test imap ready\r\n")
        .await?;

    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            return Ok(());
        }
        let command = line.trim_end_matches(['\r', '\n']).to_string();
        log.lock().expect("log").commands.push(command.clone());
        let (tag, rest) = command.split_once(' ').unwrap_or((command.as_str(), ""));
        let verb = rest
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();

        match verb.as_str() {
            "LOGIN" => match &script.login_failure {
                Some(text) => {
                    write
                        .write_all(format!("{tag} NO {text}\r\n").as_bytes())
                        .await?
                }
                None => {
                    write
                        .write_all(format!("{tag} OK LOGIN completed\r\n").as_bytes())
                        .await?
                }
            },
            "AUTHENTICATE" => {
                // Ask for the SASL frame, then accept it. The frame itself is logged so a test can
                // assert the XOAUTH2 framing without our client telling it what it sent.
                write.write_all(b"+ \r\n").await?;
                let mut frame = String::new();
                reader.read_line(&mut frame).await?;
                log.lock()
                    .expect("log")
                    .commands
                    .push(format!("SASL {}", frame.trim_end()));
                write
                    .write_all(format!("{tag} OK AUTHENTICATE completed\r\n").as_bytes())
                    .await?;
            }
            "EXAMINE" | "SELECT" => {
                let exists = script.messages.len();
                let next_uid = script.messages.iter().map(|m| m.uid).max().unwrap_or(0) + 1;
                let mut out =
                    format!("* FLAGS (\\Seen \\Answered)\r\n* {exists} EXISTS\r\n* 0 RECENT\r\n");
                if !script.omit_uid_validity {
                    out.push_str(&format!(
                        "* OK [UIDVALIDITY {}] UIDs valid\r\n",
                        script.uid_validity
                    ));
                }
                out.push_str(&format!("* OK [UIDNEXT {next_uid}] Predicted next UID\r\n"));
                out.push_str(&format!("{tag} OK [READ-ONLY] EXAMINE completed\r\n"));
                write.write_all(out.as_bytes()).await?;
            }
            "UID" => {
                let sub = rest
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .to_ascii_uppercase();
                match sub.as_str() {
                    "SEARCH" => {
                        // `UID SEARCH UID <lo>:*`. The `:*` semantics are the point of this server:
                        // RFC 3501 says the range matches the highest UID when nothing is ≥ lo, and
                        // this reproduces that faithfully — it is what the client must defend
                        // against.
                        let range = rest.split_whitespace().nth(3).unwrap_or("1:*");
                        let lo: u32 = range
                            .split(':')
                            .next()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        let mut hits: Vec<u32> = script
                            .messages
                            .iter()
                            .map(|m| m.uid)
                            .filter(|uid| *uid >= lo)
                            .collect();
                        if hits.is_empty() {
                            if let Some(highest) = script.messages.iter().map(|m| m.uid).max() {
                                hits.push(highest);
                            }
                        }
                        let ids = hits
                            .iter()
                            .map(u32::to_string)
                            .collect::<Vec<_>>()
                            .join(" ");
                        write
                            .write_all(
                                format!("* SEARCH {ids}\r\n{tag} OK SEARCH completed\r\n")
                                    .as_bytes(),
                            )
                            .await?;
                    }
                    "FETCH" => {
                        let set = rest.split_whitespace().nth(2).unwrap_or("");
                        let wanted: Vec<u32> =
                            set.split(',').filter_map(|s| s.parse().ok()).collect();
                        for (index, msg) in script.messages.iter().enumerate() {
                            if !wanted.contains(&msg.uid) {
                                continue;
                            }
                            let seq = index + 1;
                            let header = format!(
                                "* {seq} FETCH (UID {} BODY[] {{{}}}\r\n",
                                msg.uid,
                                msg.raw.len()
                            );
                            write.write_all(header.as_bytes()).await?;
                            write.write_all(&msg.raw).await?;
                            write.write_all(b")\r\n").await?;
                        }
                        write
                            .write_all(format!("{tag} OK FETCH completed\r\n").as_bytes())
                            .await?;
                    }
                    other => {
                        write
                            .write_all(format!("{tag} BAD unsupported UID {other}\r\n").as_bytes())
                            .await?
                    }
                }
            }
            "LOGOUT" => {
                write
                    .write_all(format!("* BYE\r\n{tag} OK LOGOUT completed\r\n").as_bytes())
                    .await?;
                return Ok(());
            }
            "CAPABILITY" => {
                write
                    .write_all(
                        format!("* CAPABILITY IMAP4rev1 AUTH=XOAUTH2\r\n{tag} OK\r\n").as_bytes(),
                    )
                    .await?;
            }
            other => {
                write
                    .write_all(format!("{tag} BAD unsupported command {other}\r\n").as_bytes())
                    .await?;
            }
        }
    }
}
