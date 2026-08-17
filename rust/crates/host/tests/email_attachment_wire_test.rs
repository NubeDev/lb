//! A report's PDF, from the **asset store to the SMTP wire** (report-email-delivery).
//!
//! `email_transport_test.rs` proves the target's behaviour against the sanctioned
//! `RecordingEmailProvider`, and `email_target.rs`'s unit tests prove the fan-out and the attachment
//! resolution. None of that proves the bytes survive: our recorder is our own code, so it agrees with
//! whatever we hand it. The thing that can silently break is everything after the trait — the MIME
//! wrapper, the base64 encoding, the `Content-Disposition` filename — and the only witness that counts
//! is a server on the far end of a socket saying what it received.
//!
//! So this file wires the WHOLE path with nothing stubbed: a real `mem://` store holding a real asset,
//! a real outbox row, the real `relay_outbox` pass, the real `SmtpEmailProvider`, and the real SMTP
//! listener from `lb-mail`'s transport tests. The assertion is on the `DATA` payload the server read
//! off the wire.
//!
//! The listener is INCLUDED from `crates/mail/tests/smtp_server/`, not copied: it is the one sanctioned
//! test server (a true external you cannot run locally), and a second copy would drift from the one the
//! transport tests keep honest.

#[path = "../../mail/tests/smtp_server/mod.rs"]
mod smtp_server;

use std::time::Duration;

use base64::Engine;
use lb_host::{
    relay_outbox, EmailTarget, MailAuthMechanism, SmtpEmailProvider, SmtpTransportConfig, TlsMode,
    EMAIL_TARGET,
};
use lb_store::Store;
use smtp_server::{Script, TestSmtpServer};

/// Long enough that mail-builder wraps its base64 across several lines — the wrapping is exactly what a
/// naive "the encoded blob appears verbatim" check would miss, so the assertion unwraps before matching.
fn pdf_bytes() -> Vec<u8> {
    let mut bytes = b"%PDF-1.7\n% weekly energy report\n".to_vec();
    for page in 0..40 {
        bytes.extend_from_slice(format!("{page} 0 obj << /Type /Page >> endobj\n").as_bytes());
    }
    bytes.extend_from_slice(b"%%EOF\n");
    bytes
}

/// A provider pointed at the in-test listener. No auth and no TLS: the listener has no certificate, and
/// auth framing is already proven in `lb-mail`'s `smtp_send_test.rs` — the subject here is the payload.
fn provider(server: &TestSmtpServer, store: &Store) -> SmtpEmailProvider {
    SmtpEmailProvider::new(
        SmtpTransportConfig {
            host: server.host(),
            port: server.port(),
            tls: TlsMode::None,
            auth: MailAuthMechanism::None,
            from_name: "Nube".into(),
            from_addr: "reports@nube.com".into(),
            timeout: Duration::from_secs(5),
            ..Default::default()
        },
        store.clone(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reports_pdf_reaches_every_recipient_as_a_real_mime_attachment() {
    let store = Store::memory().await.unwrap();
    let pdf = pdf_bytes();
    lb_assets::put_asset(
        &store,
        "nube",
        &lb_assets::Asset::new(
            "report-energy-week",
            "user:test",
            "application/pdf",
            pdf.clone(),
            1,
        ),
    )
    .await
    .unwrap();

    let server = TestSmtpServer::start(Script {
        auth_mechanisms: String::new(),
        ..Default::default()
    })
    .await;
    let target = EmailTarget::new(Box::new(provider(&server, &store)), store.clone());

    // The row a scheduled report stages: the artefact travels by REFERENCE, and the two recipients are
    // one effect — so this also proves the fan-out opens a session per address.
    let payload = serde_json::json!({
        "workspace": "nube",
        "recipients": ["ap@nube-io.com", "ops@nube-io.com"],
        "subject": "energy — 2026-08-10 → 2026-08-17",
        "body": "The weekly energy report is attached.",
        "assetId": "report-energy-week",
    });
    let effect = lb_outbox::Effect::new(
        "report:energy-week",
        EMAIL_TARGET,
        "report",
        payload.to_string(),
        "report:energy-week",
        0,
    );
    lb_outbox::enqueue(
        &store,
        "nube",
        "report",
        "report:energy-week",
        &payload,
        &effect,
    )
    .await
    .unwrap();

    let pass = relay_outbox(&store, "nube", &target, 1).await.unwrap();
    assert_eq!(pass.delivered, 1, "the effect must deliver: {pass:?}");

    let received = server.received();
    let envelopes = received.rcpt_to.join(" ");
    assert!(envelopes.contains("ap@nube-io.com"), "{envelopes}");
    assert!(envelopes.contains("ops@nube-io.com"), "{envelopes}");
    assert_eq!(
        received.messages.len(),
        2,
        "one message per recipient reached the server"
    );

    let encoded = base64::engine::general_purpose::STANDARD.encode(&pdf);
    for (index, raw) in received.messages.iter().enumerate() {
        let wire = String::from_utf8_lossy(raw).to_string();
        assert!(
            wire.contains("filename=\"report-energy-week.pdf\""),
            "message {index} lost the attachment filename:\n{wire}"
        );
        assert!(
            wire.contains("application/pdf"),
            "message {index} lost the attachment content type:\n{wire}"
        );
        // mail-builder wraps base64 at the MIME line limit, so the encoded blob is contiguous only once
        // the folding CRLFs are removed. Matching the WHOLE encoding is what proves no byte was
        // truncated or re-encoded on the way out.
        let unwrapped: String = wire.chars().filter(|c| *c != '\r' && *c != '\n').collect();
        assert!(
            unwrapped.contains(&encoded),
            "message {index} did not carry the PDF's bytes intact"
        );
    }

    // Each session addressed exactly one recipient — an attachment mailed to the wrong tenant is the
    // failure a shared fan-out would produce, and the `To:` headers are how it would show.
    let addressed: Vec<bool> = received
        .messages
        .iter()
        .map(|raw| String::from_utf8_lossy(raw).contains("ap@nube-io.com"))
        .collect();
    assert_eq!(
        addressed,
        vec![true, false],
        "the two sessions must carry the two different recipients, in payload order"
    );
}
