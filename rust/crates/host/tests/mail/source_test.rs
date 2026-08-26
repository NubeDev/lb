//! The **mail-source roster verbs** and the gates in front of them, against a real IMAP server.
//!
//! The sibling of `mail_import_test.rs`: that suite is about what an arriving message becomes, this
//! one is about who may point the platform at a mailbox in the first place — the two mandatory
//! categories (capability deny with **nothing written**, workspace isolation across two mailboxes),
//! plus `check` proving credentials while importing nothing, and re-registration keeping its cursor.

use lb_host::list_inbox;
use lb_host::mail::{
    check_source, mail_source_list, mail_source_register, poll_source, read_source, save_source,
};
use lb_mail::MailboxCursor;
use lb_store::Store;

use crate::harness::imap_server::{Script, StoredMessage, TestImapServer};
use crate::harness::{
    admin, fetcher_for, meter_email, nem12_source, principal, series_count, NEM12_FILENAME,
};

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
    assert!(
        matches!(error, lb_host::mail::MailSourceError::Denied),
        "{error}"
    );

    // The deny must land BEFORE the write, not after it — assert the absence of the record, not
    // merely the error (a gate that refuses the caller and stores the row anyway is still a hole).
    assert!(
        read_source(&store, ws, "sneaky").await.unwrap().is_none(),
        "a denied register wrote a mail_source record anyway"
    );

    // The property only the OUTER gate has: a caller holding broad store caps still cannot list.
    let listed = mail_source_list(&store, &member, ws).await;
    assert!(matches!(
        listed,
        Err(lb_host::mail::MailSourceError::Denied)
    ));
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
        matches!(cross, Err(lb_host::mail::MailSourceError::Denied)),
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
        lb_host::mail::token_cache(),
        lb_host::mail::http_client(),
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
        matches!(error, lb_host::mail::MailSourceError::BadInput(_)),
        "{error}"
    );
    assert!(read_source(&store, ws, "broken").await.unwrap().is_none());
}
