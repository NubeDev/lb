//! **Real Gmail reachability** — the one test that needs the internet, so it is `#[ignore]`d.
//!
//! Run on demand: `cargo test -p lb-mail --test gmail_reach_test -- --ignored --nocapture`
//!
//! Why it exists: every other transport test runs over **plaintext** against the in-test SMTP server, so
//! the TLS paths (`connect()` → STARTTLS upgrade, real certificate verification) had no coverage at all —
//! and STARTTLS on 587 is exactly what Gmail and Microsoft 365 require. This closes that gap without a
//! credential: it authenticates with a deliberately bogus one and asserts we get Gmail's own **`535`
//! rejection**, which can only happen if DNS, TCP, the STARTTLS upgrade, certificate verification, `EHLO`,
//! the advertised-mechanism negotiation and the SASL framing all worked. The only untested link left is
//! whether a real credential is accepted, which no test can own.
//!
//! It is `#[ignore]`d rather than deleted because it is the fastest way to answer "is the transport
//! broken, or is my credential wrong?" on a live box — and it is not in the default run, so
//! `cargo test --workspace` stays offline and self-contained.

use std::time::Duration;

use lb_mail::send::auth::MailCredentials;
use lb_mail::{send_smtp, MailMessage, SmtpEndpoint, TlsMode};

fn probe_message() -> MailMessage {
    MailMessage::new(
        "lb-transport-probe@gmail.com",
        "nobody@example.com",
        "probe",
        "probe",
    )
}

fn gmail() -> SmtpEndpoint {
    SmtpEndpoint::new(
        "smtp.gmail.com",
        587,
        TlsMode::Starttls,
        Duration::from_secs(20),
    )
}

/// A password credential reaches Gmail's authenticator over a real STARTTLS session.
#[tokio::test]
#[ignore = "needs outbound internet on port 587"]
async fn a_starttls_session_reaches_real_gmails_authenticator() {
    let creds = MailCredentials::Password {
        username: "lb-transport-probe@gmail.com".into(),
        password: "definitely-not-a-real-password".into(),
    };
    let err = send_smtp(&gmail(), &creds, &probe_message())
        .await
        .expect_err("a bogus credential must be rejected");

    // 535 = we got all the way to AUTH and Gmail read our SASL blob. Anything else (TLS error, timeout,
    // MissingStartTls, UnsupportedAuthMechanism) means the TRANSPORT is broken, not the credential.
    assert!(
        err.message().contains("535"),
        "expected Gmail's auth rejection, got: {err}"
    );
    assert!(
        err.is_permanent(),
        "bad credentials will not fix themselves"
    );
    assert!(
        !err.message().contains("definitely-not-a-real-password"),
        "the credential reached the error: {err}"
    );
}

/// The same over XOAUTH2 — proving Gmail advertises the mechanism and accepts our bearer framing (a
/// mechanism it did NOT advertise would fail as `UnsupportedAuthMechanism`, never as `535`).
#[tokio::test]
#[ignore = "needs outbound internet on port 587"]
async fn xoauth2_negotiates_with_real_gmail() {
    let creds = MailCredentials::XOauth2 {
        username: "lb-transport-probe@gmail.com".into(),
        access_token: "ya29.definitely-not-a-real-access-token".into(),
    };
    let err = send_smtp(&gmail(), &creds, &probe_message())
        .await
        .expect_err("a bogus access token must be rejected");

    assert!(
        err.message().contains("535"),
        "expected Gmail's auth rejection, got: {err}"
    );
    assert!(
        !err.message()
            .contains("ya29.definitely-not-a-real-access-token"),
        "the token reached the error: {err}"
    );
}
