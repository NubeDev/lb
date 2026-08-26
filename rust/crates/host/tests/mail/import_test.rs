//! **Email → inbox + ingest, end to end**, against a real IMAP server on a real socket.
//!
//! What an arriving message *becomes*: assets, series, an inbox item — and what it must never become
//! twice. The roster verbs and their gates are the sibling suite (`mail_source_test.rs`); the shared
//! setup is `mail/mod.rs`.
//!
//! The message under test is built by the platform's OWN send half (`lb_mail::MailMessage`) and
//! carries the genuine 163 KB four-channel NEM12 export from `lb-ingest`'s fixtures — so what these
//! tests exercise is the actual outbound MIME shape arriving at the actual inbound parser, with a
//! real meter file inside it. Nothing about the message is hand-shaped to be easy to parse.

use lb_host::list_inbox;
use lb_host::mail::{already_imported, message_key, poll_source, read_source, save_source};
use lb_mail::MailboxCursor;
use lb_store::Store;

use crate::harness::imap_server::{Script, StoredMessage, TestImapServer};
use crate::harness::{
    admin, fetcher_for, meter_email, nem12_source, pdf_email, series_count, series_tag,
    EXPECTED_SAMPLES, NEM12_FILENAME, REAL_NEM12,
};
use lb_host::mail::AttachmentPolicy;

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
