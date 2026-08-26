//! The shared harness for the mail-source suites — the real IMAP server, the real NEM12 export,
//! and the message builders both suites use.
//!
//! `mail_import_test.rs` (what an arriving message becomes) and `mail_source_test.rs` (the roster
//! verbs and their gates) are two test binaries over one setup. This module is the setup, included
//! by both: one definition of "a source configured for NEM12" is what stops the two suites from
//! quietly testing different things.
//!
//! The IMAP server is the one `lb-mail`'s own fetch tests use, included by path rather than copied —
//! two implementations of a protocol server drift, and the whole point of it is that it speaks IMAP
//! faithfully (in particular the `n:*` range that always matches something).

#![allow(dead_code)] // each suite uses a different part of this harness

#[path = "../../../mail/tests/imap_server/mod.rs"]
pub mod imap_server;

use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::mail::MailSource;
use lb_mail::{ImapEndpoint, ImapFetch, MailCredentials, MailMessage, TlsMode};
use lb_store::Store;

use imap_server::TestImapServer;

/// The real four-channel NEM12 export (55 days, 15-minute intervals, kWh + kVArh).
pub const REAL_NEM12: &[u8] = include_bytes!("../../../ingest/tests/fixtures/nem12-4-channel.csv");

pub const NEM12_FILENAME: &str = "ZZZZ035361_nem12#0045575584#TCAUSTM.csv";

/// Every sample the file holds: 220 `300` records × 96 fifteen-minute intervals.
pub const EXPECTED_SAMPLES: usize = 220 * 96;

pub fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::WorkspaceAdmin,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

pub fn admin(ws: &str) -> Principal {
    principal(
        "user:ada",
        ws,
        &[
            "mcp:mail.source.register:call",
            "mcp:mail.source.list:call",
            "mcp:mail.source.check:call",
            "mcp:mail.source.poll:call",
            "mcp:inbox.list:call",
        ],
    )
}

/// A source configured the way an operator would for NEM12: decode CSVs, NEM time, `nem12.` prefix.
pub fn nem12_source(id: &str) -> MailSource {
    let json = serde_json::json!({
        "id": id,
        "name": "Meter data",
        "host": "127.0.0.1",
        "port": 993,
        "tls": "none",
        "mailbox": "INBOX",
        "username": "alerts@nube-io.com",
        "auth": "plain",
        "secretPath": "mail/inbox-password",
        "channel": "mail",
        "pollSeconds": 60,
        "attachments": {
            "storeBytes": true,
            "ingest": true,
            "format": "auto",
            "extensions": ["csv"],
            "seriesPrefix": "nem12.",
            "offsetMinutes": 600
        }
    });
    serde_json::from_value(json).expect("a valid source")
}

/// One RFC 5322 message with the real NEM12 export attached, built by the platform's send half.
pub fn meter_email(from: &str, message_id: &str) -> Vec<u8> {
    let mut message = MailMessage::new(
        from,
        "alerts@nube-io.com",
        "NEM12 interval data — ZZZZ035361",
        "Attached is the interval data for July and August.",
    );
    message.from_name = "Meter Data".into();
    message.message_id = Some(message_id.to_string());
    message.attachments = vec![(
        "text/csv".into(),
        NEM12_FILENAME.into(),
        REAL_NEM12.to_vec(),
    )];
    message.to_rfc5322().expect("render the message")
}

/// A message with a file the source will not decode.
pub fn pdf_email(message_id: &str) -> Vec<u8> {
    let mut message = MailMessage::new(
        "billing@example.com",
        "alerts@nube-io.com",
        "Your invoice",
        "See attached.",
    );
    message.message_id = Some(message_id.to_string());
    message.attachments = vec![(
        "application/pdf".into(),
        "invoice.pdf".into(),
        b"%PDF-1.4 not really a pdf".to_vec(),
    )];
    message.to_rfc5322().expect("render the message")
}

pub fn fetcher_for(server: &TestImapServer) -> ImapFetch {
    ImapFetch::new(
        ImapEndpoint::new(
            server.addr.ip().to_string(),
            server.addr.port(),
            TlsMode::None,
            Duration::from_secs(10),
        ),
        MailCredentials::Password {
            username: "alerts@nube-io.com".into(),
            password: "hunter2hunter2".into(),
        },
    )
}

pub async fn series_count(store: &Store, ws: &str, series: &str) -> usize {
    lb_ingest::read(store, ws, series, None, None)
        .await
        .expect("read series")
        .len()
}

/// The tags a series carries. Ingest converts a sample's wire `labels` into **tag-graph edges** at
/// commit (`lb_ingest::labels`), which is where `series.find` reads them from — so asserting here
/// rather than on the sample row proves the dimensions actually reached the graph, not just that
/// the decoder attached them.
pub async fn series_tag(
    store: &Store,
    ws: &str,
    series: &str,
    key: &str,
) -> Option<serde_json::Value> {
    lb_tags::of(store, ws, &format!("series:{series}"))
        .await
        .expect("tags")
        .into_iter()
        .find(|t| t.key == key)
        .map(|t| t.value)
}
