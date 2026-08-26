//! The receive half against a **real IMAP server on a real socket** (`imap_server/`).
//!
//! What these prove that a recorder never could: the `n:*` range trap, the `{len}` literal framing
//! of a fetched body, that the mailbox is opened read-only and the body peeked, and that a rejected
//! login is classified permanent with the password redacted out of the server's own words.

mod imap_server;

use std::time::Duration;

use lb_mail::fetch::MailFetch;
use lb_mail::{ImapEndpoint, ImapFetch, MailCredentials, MailboxCursor, TlsMode};

use imap_server::{Script, StoredMessage, TestImapServer};

fn creds() -> MailCredentials {
    MailCredentials::Password {
        username: "alerts@nube-io.com".into(),
        password: "hunter2hunter2".into(),
    }
}

fn message(uid: u32, subject: &str) -> StoredMessage {
    StoredMessage {
        uid,
        raw: format!(
            "From: sender@example.com\r\nTo: alerts@nube-io.com\r\nSubject: {subject}\r\n\
             Message-ID: <{uid}@example.com>\r\n\r\nbody of {subject}\r\n"
        )
        .into_bytes(),
    }
}

fn fetcher(server: &TestImapServer) -> ImapFetch {
    ImapFetch::new(
        ImapEndpoint::new(
            server.addr.ip().to_string(),
            server.addr.port(),
            TlsMode::None,
            Duration::from_secs(5),
        ),
        creds(),
    )
}

#[tokio::test]
async fn a_fresh_cursor_reads_the_whole_mailbox_oldest_first() {
    let server = TestImapServer::start(Script {
        messages: vec![message(1, "one"), message(2, "two"), message(3, "three")],
        ..Default::default()
    })
    .await;

    let batch = fetcher(&server)
        .fetch_since(&MailboxCursor::default(), 25)
        .await
        .expect("fetch");

    assert_eq!(batch.uid_validity, 42);
    assert_eq!(
        batch.messages.iter().map(|m| m.uid).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "messages must arrive ascending by uid"
    );
    assert!(
        String::from_utf8_lossy(&batch.messages[0].raw).contains("Subject: one"),
        "the literal-framed body must survive byte-for-byte"
    );
    assert!(!batch.more);
}

/// The bug this whole client is shaped around. `UID SEARCH UID 4:*` against a mailbox whose highest
/// UID is 3 returns **3**, per RFC 3501 — so a poller that trusted the range would re-import the
/// newest message on every idle tick, forever.
#[tokio::test]
async fn an_idle_poll_returns_nothing_even_though_the_server_matches_the_newest_message() {
    let server = TestImapServer::start(Script {
        messages: vec![message(1, "one"), message(2, "two"), message(3, "three")],
        ..Default::default()
    })
    .await;

    let cursor = MailboxCursor::new(42, 3);
    let batch = fetcher(&server)
        .fetch_since(&cursor, 25)
        .await
        .expect("fetch");

    assert!(
        batch.messages.is_empty(),
        "the `n:*` range matched uid 3 again; the cursor filter is what must reject it — got {:?}",
        batch.messages.iter().map(|m| m.uid).collect::<Vec<_>>()
    );
    // And the server really did offer it, so this is not a vacuous pass.
    assert!(server.received().saw("UID SEARCH"));
}

#[tokio::test]
async fn only_messages_after_the_cursor_come_back() {
    let server = TestImapServer::start(Script {
        messages: vec![message(1, "one"), message(2, "two"), message(3, "three")],
        ..Default::default()
    })
    .await;

    let batch = fetcher(&server)
        .fetch_since(&MailboxCursor::new(42, 1), 25)
        .await
        .expect("fetch");

    assert_eq!(
        batch.messages.iter().map(|m| m.uid).collect::<Vec<_>>(),
        vec![2, 3]
    );
}

#[tokio::test]
async fn the_limit_bounds_one_pass_and_reports_that_there_is_more() {
    let server = TestImapServer::start(Script {
        messages: (1..=10).map(|uid| message(uid, "bulk")).collect(),
        ..Default::default()
    })
    .await;

    let batch = fetcher(&server)
        .fetch_since(&MailboxCursor::default(), 3)
        .await
        .expect("fetch");

    assert_eq!(
        batch.messages.iter().map(|m| m.uid).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "a bounded pass takes the OLDEST first — a backlog must drain in order"
    );
    assert!(batch.more, "the caller must learn to poll again promptly");
    assert_eq!(batch.highest_uid(), Some(3));
}

/// The read-only contract, proven from the server's log rather than by inspecting our own code.
#[tokio::test]
async fn the_mailbox_is_never_mutated() {
    let server = TestImapServer::start(Script {
        messages: vec![message(1, "one")],
        ..Default::default()
    })
    .await;

    fetcher(&server)
        .fetch_since(&MailboxCursor::default(), 25)
        .await
        .expect("fetch");

    let seen = server.received();
    assert!(seen.saw("EXAMINE"), "the mailbox must be opened read-only");
    assert!(
        !seen.saw("STORE"),
        "nothing may set a flag on a human's mailbox: {:?}",
        seen.commands
    );
    assert!(
        seen.saw("BODY.PEEK["),
        "the body must be PEEKed so `\\Seen` is not set: {:?}",
        seen.commands
    );
}

#[tokio::test]
async fn a_uid_validity_bump_reads_the_new_generation_from_the_start() {
    let server = TestImapServer::start(Script {
        uid_validity: 99,
        messages: vec![message(1, "reborn")],
        ..Default::default()
    })
    .await;

    // The caller's cursor is from generation 42 at uid 4200 — far past uid 1.
    let batch = fetcher(&server)
        .fetch_since(&MailboxCursor::new(42, 4200), 25)
        .await
        .expect("fetch");

    assert_eq!(batch.uid_validity, 99);
    assert_eq!(
        batch.messages.iter().map(|m| m.uid).collect::<Vec<_>>(),
        vec![1],
        "a renumbered mailbox must be re-read from the start, not skipped to 4200"
    );
}

#[tokio::test]
async fn a_rejected_login_is_permanent_and_never_echoes_the_password() {
    let server = TestImapServer::start(Script {
        // A real server echoing the credential it rejected — the disclosure the redaction exists for.
        login_failure: Some("[AUTHENTICATIONFAILED] Invalid credentials hunter2hunter2".into()),
        ..Default::default()
    })
    .await;

    let err = fetcher(&server)
        .fetch_since(&MailboxCursor::default(), 25)
        .await
        .expect_err("a bad credential must fail");

    assert!(
        err.is_permanent(),
        "retrying a wrong password is a lockout, not a fix: {err}"
    );
    assert!(
        !err.message().contains("hunter2hunter2"),
        "the password reached the error string: {err}"
    );
}

#[tokio::test]
async fn a_mailbox_with_no_uid_support_is_a_permanent_config_error() {
    let server = TestImapServer::start(Script {
        omit_uid_validity: true,
        messages: vec![message(1, "one")],
        ..Default::default()
    })
    .await;

    let err = fetcher(&server)
        .fetch_since(&MailboxCursor::default(), 25)
        .await
        .expect_err("no UIDVALIDITY means no durable cursor");
    assert!(err.is_permanent(), "{err}");
    assert!(err.message().contains("UIDVALIDITY"), "{err}");
}

#[tokio::test]
async fn a_server_that_accepts_and_says_nothing_times_out_rather_than_hanging_the_reactor() {
    let server = TestImapServer::start(Script {
        silent: true,
        ..Default::default()
    })
    .await;

    let fetcher = ImapFetch::new(
        ImapEndpoint::new(
            server.addr.ip().to_string(),
            server.addr.port(),
            TlsMode::None,
            Duration::from_millis(300),
        ),
        creds(),
    );
    let started = std::time::Instant::now();
    let err = fetcher
        .fetch_since(&MailboxCursor::default(), 25)
        .await
        .expect_err("a silent server must not hang the poller");

    assert!(!err.is_permanent(), "a hang is worth retrying: {err}");
    assert!(err.message().contains("timeout"), "{err}");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the timeout did not bound the session: {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn xoauth2_puts_the_expected_sasl_frame_on_the_wire() {
    let server = TestImapServer::start(Script {
        messages: vec![message(1, "one")],
        ..Default::default()
    })
    .await;

    let fetcher = ImapFetch::new(
        ImapEndpoint::new(
            server.addr.ip().to_string(),
            server.addr.port(),
            TlsMode::None,
            Duration::from_secs(5),
        ),
        MailCredentials::XOauth2 {
            username: "alerts@nube-io.com".into(),
            access_token: "ya29.test-token".into(),
        },
    );
    let batch = fetcher
        .fetch_since(&MailboxCursor::default(), 25)
        .await
        .expect("fetch");
    assert_eq!(batch.messages.len(), 1);

    // The frame the SERVER received, decoded — not what we believe we sent.
    let sasl = server
        .received()
        .commands
        .iter()
        .find_map(|c| c.strip_prefix("SASL ").map(str::to_string))
        .expect("the server received a SASL frame");
    let decoded =
        String::from_utf8(base64_decode(sasl.trim()).expect("the frame is base64")).expect("utf-8");
    assert_eq!(
        decoded, "user=alerts@nube-io.com\u{1}auth=Bearer ya29.test-token\u{1}\u{1}",
        "the XOAUTH2 frame must match the SASL spec exactly"
    );
}

/// Minimal standard-alphabet base64 decode, so the test does not need a dependency to read what the
/// server received.
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut bits = 0;
    for byte in s.bytes().filter(|b| *b != b'=' && !b.is_ascii_whitespace()) {
        let value = ALPHABET.iter().position(|c| *c == byte)? as u32;
        acc = (acc << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}
