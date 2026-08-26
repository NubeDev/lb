//! **Email → inbox + ingest, end to end**, against a real IMAP server on a real socket.
//!
//! The message under test is built by the platform's OWN send half (`lb_mail::MailMessage`) and
//! carries the genuine 163 KB four-channel NEM12 export from `lb-ingest`'s fixtures — so what these
//! tests exercise is the actual outbound MIME shape arriving at the actual inbound parser, with a
//! real meter file inside it. Nothing about the message is hand-shaped to be easy to parse.
//!
//! The IMAP server is the one `lb-mail`'s own fetch tests use, included by path rather than copied:
//! two implementations of a test server drift, and the whole point of it is that it speaks the
//! protocol faithfully (in particular the `n:*` range that always matches something).
//!
//! Mandatory categories, per testing-scope: **capability deny** (a member cannot register a source,
//! and no record is written), **workspace isolation** (two workspaces, two mailboxes, no leakage in
//! either direction), and **re-delivery/offline** (a re-read imports nothing twice).

#[path = "../../mail/tests/imap_server/mod.rs"]
mod imap_server;

use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    already_imported, check_source, list_inbox, mail_source_list, mail_source_register,
    message_key, poll_source, read_source, save_source, AttachmentPolicy, MailSource,
};
use lb_mail::{ImapEndpoint, ImapFetch, MailCredentials, MailMessage, MailboxCursor, TlsMode};
use lb_store::Store;

use imap_server::{Script, StoredMessage, TestImapServer};

/// The real four-channel NEM12 export (55 days, 15-minute intervals, kWh + kVArh).
const REAL_NEM12: &[u8] = include_bytes!("../../ingest/tests/fixtures/nem12-4-channel.csv");

const NEM12_FILENAME: &str = "ZZZZ035361_nem12#0045575584#TCAUSTM.csv";

/// Every sample the file holds: 220 `300` records × 96 fifteen-minute intervals.
const EXPECTED_SAMPLES: usize = 220 * 96;

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
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

fn admin(ws: &str) -> Principal {
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
fn nem12_source(id: &str) -> MailSource {
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
fn meter_email(from: &str, message_id: &str) -> Vec<u8> {
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
fn pdf_email(message_id: &str) -> Vec<u8> {
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

fn fetcher_for(server: &TestImapServer) -> ImapFetch {
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

async fn series_count(store: &Store, ws: &str, series: &str) -> usize {
    lb_ingest::read(store, ws, series, None, None)
        .await
        .expect("read series")
        .len()
}

/// The tags a series carries. Ingest converts a sample's wire `labels` into **tag-graph edges** at
/// commit (`lb_ingest::labels`), which is where `series.find` reads them from — so asserting here
/// rather than on the sample row proves the dimensions actually reached the graph, not just that
/// the decoder attached them.
async fn series_tag(store: &Store, ws: &str, series: &str, key: &str) -> Option<serde_json::Value> {
    lb_tags::of(store, ws, &format!("series:{series}"))
        .await
        .expect("tags")
        .into_iter()
        .find(|t| t.key == key)
        .map(|t| t.value)
}

// ---------------------------------------------------------------------------------------------
// The headline: an email with a meter file becomes series data and an inbox item.
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_emailed_meter_file_becomes_series_data_and_shows_up_in_the_inbox() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    let server = TestImapServer::start(Script {
        messages: vec![StoredMessage {
            uid: 1,
            raw: meter_email("data@example.com", "nem12-july@example.com"),
        }],
        ..Default::default()
    })
    .await;

    let mut source = nem12_source("meter-data");
    save_source(&store, ws, &source).await.expect("seed");

    let pass = poll_source(&store, ws, &mut source, &fetcher_for(&server), 10, 1_000)
        .await
        .expect("poll");

    // --- the pass itself
    assert_eq!(pass.fetched, 1);
    assert_eq!(pass.imported, 1, "error: {:?}", pass.error);
    assert_eq!(pass.samples, EXPECTED_SAMPLES);
    assert_eq!(
        pass.series,
        vec![
            "nem12.ZZZZ035361.B1",
            "nem12.ZZZZ035361.E1",
            "nem12.ZZZZ035361.K1",
            "nem12.ZZZZ035361.Q1",
        ],
        "one series per meter channel"
    );

    // --- the data plane really holds the samples
    let b1 = series_count(&store, ws, "nem12.ZZZZ035361.B1").await;
    assert_eq!(b1, 55 * 96, "55 days of 15-minute readings for channel B1");
    let latest = lb_ingest::latest(&store, ws, "nem12.ZZZZ035361.B1")
        .await
        .expect("latest")
        .expect("a newest sample");
    assert!(
        latest.payload.is_number(),
        "the payload must be the meter's number, not a blob: {:?}",
        latest.payload
    );
    // The producer is ROOTED at the importer and sub-namespaced per source, so two sources feeding
    // one series are two independent seq spaces.
    assert_eq!(latest.producer, "node:mail/meter-data");

    // The meter's own dimensions, and the provenance, both reached the tag graph.
    assert_eq!(
        series_tag(&store, ws, "nem12.ZZZZ035361.B1", "nmi").await,
        Some(serde_json::json!("ZZZZ035361"))
    );
    assert_eq!(
        series_tag(&store, ws, "nem12.ZZZZ035361.B1", "uom").await,
        Some(serde_json::json!("KWH"))
    );
    assert_eq!(
        series_tag(&store, ws, "nem12.ZZZZ035361.K1", "uom").await,
        Some(serde_json::json!("KVARH")),
        "the reactive channel keeps its own unit"
    );
    assert_eq!(
        series_tag(&store, ws, "nem12.ZZZZ035361.B1", "mailSource").await,
        Some(serde_json::json!("meter-data")),
        "provenance: months later, 'why is this series here' must be answerable"
    );
    assert_eq!(
        series_tag(&store, ws, "nem12.ZZZZ035361.B1", "mailFrom").await,
        Some(serde_json::json!("data@example.com"))
    );

    // --- the inbox
    let items = list_inbox(&store, &admin(ws), ws, "mail")
        .await
        .expect("list inbox");
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert!(item.body.contains("NEM12 interval data"), "{}", item.body);
    assert!(item.body.contains("data@example.com"), "{}", item.body);
    assert!(
        item.body.contains(&format!("{EXPECTED_SAMPLES} samples")),
        "the item must say what arrived: {}",
        item.body
    );
    assert_eq!(item.author, "node:mail");

    let meta = item
        .meta
        .as_ref()
        .expect("the item carries its mail payload");
    assert_eq!(meta["sourceId"], "meter-data");
    assert_eq!(meta["from"]["address"], "data@example.com");
    assert_eq!(meta["subject"], "NEM12 interval data — ZZZZ035361");
    assert_eq!(meta["samples"], EXPECTED_SAMPLES);
    let attachments = meta["attachments"].as_array().expect("attachments");
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0]["filename"], NEM12_FILENAME);
    assert_eq!(attachments[0]["ingest"]["format"], "nem12");
    assert_eq!(attachments[0]["ingest"]["accepted"], EXPECTED_SAMPLES);

    // --- the bytes survive, byte-for-byte, as the fidelity escape hatch
    let attachment_asset = attachments[0]["assetId"].as_str().expect("stored");
    let stored = lb_assets::get_asset(&store, ws, attachment_asset)
        .await
        .expect("read asset")
        .expect("the attachment is stored");
    assert_eq!(
        stored.bytes, REAL_NEM12,
        "the attachment must round-trip byte-identical"
    );
    let raw = lb_assets::get_asset(&store, ws, meta["rawAssetId"].as_str().unwrap())
        .await
        .expect("read raw")
        .expect("the raw message is stored FIRST, before anything parses it");
    assert!(raw.bytes.starts_with(b"From:") || raw.bytes.windows(5).any(|w| w == b"From:"));
    assert_eq!(raw.mime, "message/rfc822");

    // --- the cursor advanced
    let stored_source = read_source(&store, ws, "meter-data")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored_source.cursor.last_uid, 1);
    assert_eq!(stored_source.imported, 1);
    assert_eq!(stored_source.last_error, None);
}

// ---------------------------------------------------------------------------------------------
// Re-delivery / offline resume
// ---------------------------------------------------------------------------------------------

/// The ledger, not the cursor, is what makes this safe — so the cursor is deliberately rewound to
/// force a re-read of a message that was already imported. Without the ledger this doubles
/// everything.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_re_read_message_imports_nothing_twice() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    let server = TestImapServer::start(Script {
        messages: vec![StoredMessage {
            uid: 1,
            raw: meter_email("data@example.com", "nem12-july@example.com"),
        }],
        ..Default::default()
    })
    .await;

    let mut source = nem12_source("meter-data");
    save_source(&store, ws, &source).await.unwrap();
    poll_source(&store, ws, &mut source, &fetcher_for(&server), 10, 1_000)
        .await
        .expect("first poll");

    // Rewind the cursor: exactly what a UIDVALIDITY bump or a crash-before-cursor-write produces.
    source.cursor = MailboxCursor::default();
    let second = poll_source(&store, ws, &mut source, &fetcher_for(&server), 10, 2_000)
        .await
        .expect("second poll");

    assert_eq!(second.fetched, 1, "the message really was re-fetched");
    assert_eq!(second.imported, 0);
    assert_eq!(second.duplicates, 1, "the ledger caught it");
    assert_eq!(second.samples, 0);

    let items = list_inbox(&store, &admin(ws), ws, "mail").await.unwrap();
    assert_eq!(items.len(), 1, "one message, one inbox item");
    assert_eq!(
        series_count(&store, ws, "nem12.ZZZZ035361.B1").await,
        55 * 96,
        "re-importing must not double the series"
    );
}

/// The same message arriving at a DIFFERENT uid — a provider that moved and re-delivered it. The
/// cursor cannot catch this; only the ledger can.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_message_re_delivered_at_a_new_uid_is_still_a_duplicate() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    let raw = meter_email("data@example.com", "nem12-july@example.com");

    let first = TestImapServer::start(Script {
        messages: vec![StoredMessage {
            uid: 1,
            raw: raw.clone(),
        }],
        ..Default::default()
    })
    .await;
    let mut source = nem12_source("meter-data");
    save_source(&store, ws, &source).await.unwrap();
    poll_source(&store, ws, &mut source, &fetcher_for(&first), 10, 1_000)
        .await
        .unwrap();

    // Same message, new UID, same UIDVALIDITY — the shape a "move to folder and back" produces.
    let again = TestImapServer::start(Script {
        messages: vec![StoredMessage { uid: 9, raw }],
        ..Default::default()
    })
    .await;
    let pass = poll_source(&store, ws, &mut source, &fetcher_for(&again), 10, 2_000)
        .await
        .unwrap();

    assert_eq!(pass.fetched, 1);
    assert_eq!(pass.duplicates, 1, "keyed on the MESSAGE, not the uid");
    assert_eq!(
        list_inbox(&store, &admin(ws), ws, "mail")
            .await
            .unwrap()
            .len(),
        1
    );
}

// ---------------------------------------------------------------------------------------------
// The sender allowlist — the mailbox-as-attack-surface containment
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sender_off_the_allowlist_imports_nothing_but_is_still_recorded() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    let server = TestImapServer::start(Script {
        messages: vec![
            StoredMessage {
                uid: 1,
                raw: meter_email("stranger@spam.invalid", "spam@spam.invalid"),
            },
            StoredMessage {
                uid: 2,
                raw: meter_email("data@example.com", "good@example.com"),
            },
        ],
        ..Default::default()
    })
    .await;

    let mut source = nem12_source("meter-data");
    source.allow_senders = vec!["@example.com".into()];
    save_source(&store, ws, &source).await.unwrap();

    let pass = poll_source(&store, ws, &mut source, &fetcher_for(&server), 10, 1_000)
        .await
        .expect("poll");

    assert_eq!(pass.rejected, 1);
    assert_eq!(pass.imported, 1);
    let items = list_inbox(&store, &admin(ws), ws, "mail").await.unwrap();
    assert_eq!(
        items.len(),
        1,
        "only the allowed sender's mail reaches the inbox"
    );
    assert_eq!(
        items[0].meta.as_ref().unwrap()["from"]["address"],
        "data@example.com"
    );

    // Nothing of the rejected message was stored — not the raw bytes, not the attachment.
    assert_eq!(
        series_count(&store, ws, "nem12.ZZZZ035361.B1").await,
        55 * 96,
        "only ONE message's samples landed"
    );

    // …but the decision IS recorded, so it is auditable and is never re-made.
    let rejected_key = message_key(Some("spam@spam.invalid"), b"");
    assert!(
        already_imported(&store, ws, "meter-data", &rejected_key)
            .await
            .unwrap(),
        "a rejected message must be ledgered, or widening the allowlist later backfills it"
    );
}

// ---------------------------------------------------------------------------------------------
// An attachment nothing can decode
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_file_the_source_cannot_decode_is_still_stored_and_still_notified() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    let server = TestImapServer::start(Script {
        messages: vec![StoredMessage {
            uid: 1,
            raw: pdf_email("invoice@example.com"),
        }],
        ..Default::default()
    })
    .await;

    let mut source = nem12_source("meter-data");
    save_source(&store, ws, &source).await.unwrap();
    let pass = poll_source(&store, ws, &mut source, &fetcher_for(&server), 10, 1_000)
        .await
        .expect("poll");

    assert_eq!(
        pass.imported, 1,
        "the mail is not lost because a file is not a CSV"
    );
    assert_eq!(pass.samples, 0);
    let items = list_inbox(&store, &admin(ws), ws, "mail").await.unwrap();
    assert_eq!(items.len(), 1);
    let meta = items[0].meta.as_ref().unwrap();
    let attachment = &meta["attachments"][0];
    assert_eq!(attachment["filename"], "invoice.pdf");
    assert!(
        attachment["assetId"].is_string(),
        "the bytes are kept even when nothing can read them: {attachment}"
    );
    assert!(
        attachment.get("ingest").is_none(),
        "a .pdf is not offered to a csv-only decoder at all: {attachment}"
    );
}

// ---------------------------------------------------------------------------------------------
// Mandatory: capability deny
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_caller_without_the_admin_cap_cannot_register_a_mailbox_and_writes_nothing() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    // A full author/member bundle — everything EXCEPT the mail caps.
    let member = principal(
        "user:bob",
        ws,
        &[
            "mcp:ingest.write:call",
            "mcp:inbox.record:call",
            "mcp:assets.put_asset:call",
            "store:*:read",
            "store:*:write",
        ],
    );

    let error = mail_source_register(&store, &member, ws, nem12_source("sneaky"), 1_000)
        .await
        .expect_err("a member must not be able to open an external ingress");
    assert!(matches!(error, lb_host::MailSourceError::Denied), "{error}");

    // The deny must land BEFORE the write, not after it — assert the absence of the record, not
    // merely the error (a gate that refuses the caller and stores the row anyway is still a hole).
    assert!(
        read_source(&store, ws, "sneaky").await.unwrap().is_none(),
        "a denied register wrote a mail_source record anyway"
    );

    // The property only the OUTER gate has: a caller holding broad store caps still cannot list.
    let listed = mail_source_list(&store, &member, ws).await;
    assert!(matches!(listed, Err(lb_host::MailSourceError::Denied)));
}

// ---------------------------------------------------------------------------------------------
// Mandatory: workspace isolation
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_workspaces_polling_two_mailboxes_never_see_each_other() {
    let store = Store::memory().await.unwrap();
    let server_a = TestImapServer::start(Script {
        messages: vec![StoredMessage {
            uid: 1,
            raw: meter_email("data@example.com", "for-a@example.com"),
        }],
        ..Default::default()
    })
    .await;
    let server_b = TestImapServer::start(Script {
        messages: vec![StoredMessage {
            uid: 1,
            raw: meter_email("data@example.com", "for-b@example.com"),
        }],
        ..Default::default()
    })
    .await;

    let mut a = nem12_source("acme-mail");
    let mut b = nem12_source("beta-mail");
    save_source(&store, "acme", &a).await.unwrap();
    save_source(&store, "beta", &b).await.unwrap();
    poll_source(&store, "acme", &mut a, &fetcher_for(&server_a), 10, 1_000)
        .await
        .unwrap();
    poll_source(&store, "beta", &mut b, &fetcher_for(&server_b), 10, 1_000)
        .await
        .unwrap();

    // Rosters do not cross.
    let acme_sources = mail_source_list(&store, &admin("acme"), "acme")
        .await
        .unwrap();
    assert_eq!(acme_sources.len(), 1);
    assert_eq!(acme_sources[0].id, "acme-mail");
    let beta_sources = mail_source_list(&store, &admin("beta"), "beta")
        .await
        .unwrap();
    assert_eq!(beta_sources.len(), 1);
    assert_eq!(beta_sources[0].id, "beta-mail");

    // Imported items do not cross.
    let acme_items = list_inbox(&store, &admin("acme"), "acme", "mail")
        .await
        .unwrap();
    let beta_items = list_inbox(&store, &admin("beta"), "beta", "mail")
        .await
        .unwrap();
    assert_eq!(acme_items.len(), 1);
    assert_eq!(beta_items.len(), 1);
    assert_eq!(
        acme_items[0].meta.as_ref().unwrap()["messageId"],
        "for-a@example.com"
    );
    assert_eq!(
        beta_items[0].meta.as_ref().unwrap()["messageId"],
        "for-b@example.com"
    );

    // Series do not cross: each workspace holds ONE file's worth, not two.
    assert_eq!(
        series_count(&store, "acme", "nem12.ZZZZ035361.B1").await,
        55 * 96
    );
    assert_eq!(
        series_count(&store, "beta", "nem12.ZZZZ035361.B1").await,
        55 * 96
    );

    // An admin of A cannot list B's sources.
    let cross = mail_source_list(&store, &admin("acme"), "beta").await;
    assert!(
        matches!(cross, Err(lb_host::MailSourceError::Denied)),
        "the workspace wall is checked FIRST"
    );
}

// ---------------------------------------------------------------------------------------------
// `check` proves credentials without importing
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_reports_the_mailbox_without_importing_or_advancing_anything() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    let server = TestImapServer::start(Script {
        messages: vec![StoredMessage {
            uid: 7,
            raw: meter_email("data@example.com", "peek@example.com"),
        }],
        ..Default::default()
    })
    .await;

    // `check` resolves the credential itself, so seal one where the source says it lives.
    // Sealed WORKSPACE-visible: the poller resolves it through `lb_secrets::get_workspace`, which
    // is the workspace-shared read — the same posture the SMTP provider's credential uses (the
    // reactor is not the admin who sealed it, so a `Private` secret would be unreadable to it).
    lb_secrets::set_with(
        &store,
        &principal("user:ada", ws, &["secret:mail/inbox-password:write"]),
        ws,
        "mail/inbox-password",
        "hunter2hunter2",
        lb_secrets::Visibility::Workspace,
    )
    .await
    .expect("seal");

    let mut source = nem12_source("meter-data");
    source.host = server.addr.ip().to_string();
    source.port = server.addr.port();
    source.allow_senders = vec!["@nowhere.invalid".into()];
    save_source(&store, ws, &source).await.unwrap();

    let result = check_source(
        &store,
        ws,
        &source,
        lb_host::mail_token_cache(),
        lb_host::mail_http_client(),
    )
    .await
    .expect("check");

    assert_eq!(result.uid_validity, 42);
    assert!(result.has_new);
    let peek = result.newest.expect("a peek at the newest message");
    assert_eq!(peek.uid, 7);
    assert_eq!(peek.from, "data@example.com");
    assert_eq!(peek.subject, "NEM12 interval data — ZZZZ035361");
    assert_eq!(peek.attachments, vec![NEM12_FILENAME]);
    assert!(
        !peek.sender_allowed,
        "the single most common 'it imports nothing' cause must be visible from check"
    );
    assert!(
        !result.endpoint.contains("hunter2"),
        "the endpoint description must never carry the credential: {}",
        result.endpoint
    );

    // Nothing was imported and nothing advanced.
    assert!(list_inbox(&store, &admin(ws), ws, "mail")
        .await
        .unwrap()
        .is_empty());
    let stored = read_source(&store, ws, "meter-data")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.cursor, MailboxCursor::default());
}

// ---------------------------------------------------------------------------------------------
// Register semantics
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn re_registering_a_source_keeps_its_cursor_so_a_config_fix_is_not_a_full_re_import() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    let admin = admin(ws);

    mail_source_register(&store, &admin, ws, nem12_source("meter-data"), 1_000)
        .await
        .expect("register");

    // The poller gets somewhere.
    let mut stored = read_source(&store, ws, "meter-data")
        .await
        .unwrap()
        .unwrap();
    stored.cursor = MailboxCursor::new(42, 4200);
    stored.imported = 900;
    save_source(&store, ws, &stored).await.unwrap();

    // An operator fixes a typo in the host name and re-registers.
    let mut fixed = nem12_source("meter-data");
    fixed.host = "imap.corrected.example.com".into();
    // …and, hostile or careless, tries to set the cursor from the request.
    fixed.cursor = MailboxCursor::new(1, 1);
    let saved = mail_source_register(&store, &admin, ws, fixed, 2_000)
        .await
        .expect("re-register");

    assert_eq!(saved.host, "imap.corrected.example.com", "config is taken");
    assert_eq!(
        saved.cursor,
        MailboxCursor::new(42, 4200),
        "history is host-owned: a re-register must not re-import the mailbox"
    );
    assert_eq!(saved.imported, 900);
    assert_eq!(saved.created_ts, 1_000, "created stays the original");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_source_that_could_never_poll_is_refused_before_it_is_stored() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    let mut broken = nem12_source("broken");
    broken.secret_path = String::new();
    broken.secret_env = String::new();

    let error = mail_source_register(&store, &admin(ws), ws, broken, 1_000)
        .await
        .expect_err("a source with nowhere to find a credential cannot poll");
    assert!(
        matches!(error, lb_host::MailSourceError::BadInput(_)),
        "{error}"
    );
    assert!(read_source(&store, ws, "broken").await.unwrap().is_none());
}

// ---------------------------------------------------------------------------------------------
// A source with the ingest half switched off
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_source_with_ingest_off_still_stores_and_notifies() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    let server = TestImapServer::start(Script {
        messages: vec![StoredMessage {
            uid: 1,
            raw: meter_email("data@example.com", "nem12@example.com"),
        }],
        ..Default::default()
    })
    .await;

    let mut source = nem12_source("meter-data");
    source.attachments = AttachmentPolicy {
        ingest: false,
        ..source.attachments
    };
    save_source(&store, ws, &source).await.unwrap();
    let pass = poll_source(&store, ws, &mut source, &fetcher_for(&server), 10, 1_000)
        .await
        .expect("poll");

    assert_eq!(pass.imported, 1);
    assert_eq!(pass.samples, 0, "ingest off means no series");
    assert!(pass.series.is_empty());
    let items = list_inbox(&store, &admin(ws), ws, "mail").await.unwrap();
    assert_eq!(items.len(), 1);
    assert!(
        items[0].meta.as_ref().unwrap()["attachments"][0]["assetId"].is_string(),
        "the file is still kept"
    );
}
