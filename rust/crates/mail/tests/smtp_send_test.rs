//! The transport against a **real SMTP server** (email-transport scope, "Testing plan" → the point of
//! the slice). The listener in `smtp_server/` speaks the protocol on a real socket and hands back what
//! it received, so these assertions are about bytes on a wire: the AUTH framing, the MIME structure
//! parsed out of the `DATA` payload with `mail-parser`, and the client's 4xx/5xx/timeout/TLS mapping.
//!
//! Everything asserted here was previously unprovable: the only `EmailProvider` was the one that logged
//! the send and acked it (issue #118).

mod smtp_server;

use std::time::Duration;

use base64::Engine;
use lb_mail::send::auth::MailCredentials;
use lb_mail::{send_smtp, MailMessage, SmtpEndpoint, TlsMode};
use smtp_server::{Script, TestSmtpServer};

fn endpoint(server: &TestSmtpServer) -> SmtpEndpoint {
    // Plain TCP: the in-test server has no certificate. TLS itself is covered by the docker-gated
    // `smtp_tls_test.rs` against a real relay — this file proves protocol, auth and MIME.
    SmtpEndpoint::new(
        server.host(),
        server.port(),
        TlsMode::None,
        Duration::from_secs(5),
    )
}

fn invite_message() -> MailMessage {
    MailMessage {
        from_name: "Acme".into(),
        from_addr: "reports@acme.com".into(),
        to: "sam@example.com".into(),
        reply_to: None,
        subject: "Te han invitado a unirte a ácme".into(),
        text: "Accept: https://acme/accept?token=lbi_abc".into(),
        html: Some(
            "<p>You are invited. <a href=\"https://acme/accept?token=lbi_abc\">Accept</a></p>"
                .into(),
        ),
        message_id: Some("invite-hash1@lazybones".into()),
        attachments: vec![(
            "text/calendar".into(),
            "invite.ics".into(),
            b"BEGIN:VCALENDAR".to_vec(),
        )],
    }
}

#[tokio::test]
async fn a_real_session_delivers_a_multipart_message_the_server_can_parse() {
    let server = TestSmtpServer::start(Script {
        auth_mechanisms: String::new(), // no auth on this relay
        ..Default::default()
    })
    .await;

    send_smtp(
        &endpoint(&server),
        &MailCredentials::None,
        &invite_message(),
    )
    .await
    .expect("the relay accepted the message");

    let received = server.received();
    assert!(
        received
            .mail_from
            .as_deref()
            .unwrap_or_default()
            .contains("reports@acme.com"),
        "{:?}",
        received.mail_from
    );
    assert_eq!(received.rcpt_to.len(), 1);
    assert!(received.rcpt_to[0].contains("sam@example.com"));

    // Parse what the server actually got — not what our builder thinks it wrote.
    let raw = received
        .message
        .expect("the server received a DATA payload");
    let parsed = mail_parser::MessageParser::default()
        .parse(&raw)
        .expect("the received bytes are a parseable RFC 5322 message");
    assert_eq!(
        parsed.subject(),
        Some("Te han invitado a unirte a ácme"),
        "an 8-bit subject must survive the encoding round-trip"
    );
    assert_eq!(
        parsed.message_id(),
        Some("invite-hash1@lazybones"),
        "the injected Message-ID is the cross-retry dedup handle"
    );
    let text = parsed.body_text(0).expect("a text/plain alternative");
    let html = parsed.body_html(0).expect("a text/html part");
    assert!(text.contains("https://acme/accept?token=lbi_abc"), "{text}");
    assert!(html.contains("<a href="), "{html}");
    assert_eq!(
        parsed.attachments().count(),
        1,
        "the attachment must arrive as its own part"
    );
}

#[tokio::test]
async fn auth_plain_puts_the_exact_sasl_blob_on_the_wire() {
    let server = TestSmtpServer::start(Script {
        auth_mechanisms: "PLAIN".into(),
        ..Default::default()
    })
    .await;
    let creds = MailCredentials::Password {
        username: "reports@acme.com".into(),
        password: "hunter2hunter2".into(),
    };

    send_smtp(&endpoint(&server), &creds, &invite_message())
        .await
        .expect("authenticated send");

    let auth_line = server.received().auth_line.expect("the server saw AUTH");
    let expected = base64::engine::general_purpose::STANDARD
        .encode("\u{0}reports@acme.com\u{0}hunter2hunter2");
    assert_eq!(auth_line, format!("AUTH PLAIN {expected}"), "{auth_line}");
}

#[tokio::test]
async fn auth_xoauth2_frames_the_bearer_token_per_the_google_spec() {
    let server = TestSmtpServer::start(Script {
        auth_mechanisms: "XOAUTH2".into(),
        ..Default::default()
    })
    .await;
    let creds = MailCredentials::XOauth2 {
        username: "reports@acme.com".into(),
        access_token: "ya29.a0AfB_access".into(),
    };

    send_smtp(&endpoint(&server), &creds, &invite_message())
        .await
        .expect("xoauth2 send");

    let auth_line = server.received().auth_line.expect("the server saw AUTH");
    // The framing Gmail/M365 require: base64("user=<u>\x01auth=Bearer <t>\x01\x01"). Getting this
    // wrong is the whole "supports Gmail" story failing at the last byte.
    let expected = base64::engine::general_purpose::STANDARD
        .encode("user=reports@acme.com\u{1}auth=Bearer ya29.a0AfB_access\u{1}\u{1}");
    assert_eq!(auth_line, format!("AUTH XOAUTH2 {expected}"), "{auth_line}");
}

#[tokio::test]
async fn a_5xx_is_permanent_and_a_4xx_is_retryable() {
    // 550: a typo'd recipient domain. Retrying five times with backoff cannot fix a mistake.
    let permanent = TestSmtpServer::start(Script {
        auth_mechanisms: String::new(),
        rcpt_reply: Some("550 5.1.2 Host unknown".into()),
        ..Default::default()
    })
    .await;
    let err = send_smtp(
        &endpoint(&permanent),
        &MailCredentials::None,
        &invite_message(),
    )
    .await
    .expect_err("550 must fail the send");
    assert!(err.is_permanent(), "{err}");
    assert!(err.message().contains("550"), "{err}");

    // 421: Gmail's rate limiter. Emphatically retryable — the outbox backs off and tries later.
    let transient = TestSmtpServer::start(Script {
        auth_mechanisms: String::new(),
        rcpt_reply: Some("421 4.7.0 Too many auth attempts".into()),
        ..Default::default()
    })
    .await;
    let err = send_smtp(
        &endpoint(&transient),
        &MailCredentials::None,
        &invite_message(),
    )
    .await
    .expect_err("421 must fail the send");
    assert!(!err.is_permanent(), "a 4xx must stay retryable: {err}");
}

#[tokio::test]
async fn a_rejected_auth_never_leaks_the_credential_into_the_error() {
    // The scope's explicit secret-hygiene test: mail libraries are chatty on failure, and this server
    // quotes the AUTH blob back in its rejection — exactly the credential disclosure that would end up
    // in an outbox row and a log line.
    let server = TestSmtpServer::start(Script {
        auth_mechanisms: "PLAIN".into(),
        auth_reply: Some("535 5.7.8 Bad credentials".into()),
        echo_auth_credential: true,
        ..Default::default()
    })
    .await;
    let creds = MailCredentials::Password {
        username: "reports@acme.com".into(),
        password: "hunter2hunter2".into(),
    };

    let err = send_smtp(&endpoint(&server), &creds, &invite_message())
        .await
        .expect_err("535 must fail the send");

    let text = format!("{err} / {err:?}");
    let wire_blob = base64::engine::general_purpose::STANDARD
        .encode("\u{0}reports@acme.com\u{0}hunter2hunter2");
    assert!(
        !text.contains("hunter2hunter2"),
        "the password reached the error: {text}"
    );
    assert!(
        !text.contains(&wire_blob),
        "the SASL blob (which decodes to the password) reached the error: {text}"
    );
    // …and the operator still learns what happened.
    assert!(text.contains("535"), "{text}");
    assert!(
        err.is_permanent(),
        "bad credentials will not fix themselves"
    );
}

#[tokio::test]
async fn a_hung_session_times_out_instead_of_stalling_the_relay() {
    // Why this is mandatory rather than nice: send_smtp runs inside the outbox relay tick, so an
    // unbounded SMTP session stalls EVERY outbox delivery behind it, push included.
    let server = TestSmtpServer::start(Script {
        silent: true,
        ..Default::default()
    })
    .await;
    let mut ep = endpoint(&server);
    ep.timeout = Duration::from_millis(300);

    let started = std::time::Instant::now();
    let err = send_smtp(&ep, &MailCredentials::None, &invite_message())
        .await
        .expect_err("a silent server must not hang the caller");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "the send did not respect its timeout ({:?})",
        started.elapsed()
    );
    assert!(!err.is_permanent(), "a timeout is retryable: {err}");
}

#[tokio::test]
async fn starttls_is_required_not_opportunistic() {
    // A server that does not advertise STARTTLS while the transport is configured for it: the client
    // must ABORT, never continue in the clear — the next thing on that socket would be the AUTH line.
    let server = TestSmtpServer::start(Script {
        advertise_starttls: false,
        auth_mechanisms: "PLAIN".into(),
        ..Default::default()
    })
    .await;
    let mut ep = endpoint(&server);
    ep.tls = TlsMode::Starttls;
    let creds = MailCredentials::Password {
        username: "reports@acme.com".into(),
        password: "hunter2hunter2".into(),
    };

    let err = send_smtp(&ep, &creds, &invite_message())
        .await
        .expect_err("a missing STARTTLS must fail the send");
    assert!(
        err.message().contains("STARTTLS"),
        "the operator must learn WHY: {err}"
    );
    let received = server.received();
    assert!(
        received.auth_line.is_none(),
        "the credential was sent over cleartext: {received:?}"
    );
    assert!(
        received.message.is_none(),
        "the message was sent over cleartext: {received:?}"
    );
}
