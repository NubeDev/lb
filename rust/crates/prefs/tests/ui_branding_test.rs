//! The `ui_branding` axis (workspace-branding scope) over a REAL `mem://` store. The opaque
//! branding blob (`{ site_name, site_abbr, tagline, login_heading }`) round-trips through set/get,
//! folds WHOLE in the resolve chain (a workspace's brand wins entirely, never merges field-by-
//! field), a workspace default fills in for a member who set no brand, and — the mandatory
//! isolation category — a workspace's brand in ws-A is never read in ws-B.
//!
//! `ui_branding` is deliberately opaque to prefs (the frontend `Branding` shape in `lib/branding`):
//! these tests treat it as an arbitrary JSON object and only assert it moves through the store/fold
//! unchanged. Image marks (logo/favicon/icon) are NOT in this blob — they live as assets at the
//! reserved ids `branding:{logo,favicon,icon}` and are exercised through the assets surface, not
//! prefs.
//!
//! Sibling of `ui_theme_test.rs`; same shape, identical fold rules, one new axis.

use lb_prefs::{
    get_user_prefs, resolve_chain, set_user_prefs, set_workspace_prefs, Prefs, USER_PREFS_TABLE,
};
use lb_store::Store;
use serde_json::json;

fn acme_brand() -> serde_json::Value {
    json!({ "site_name": "Acme", "site_abbr": "AC", "tagline": "ops", "login_heading": "Sign in to Acme" })
}

fn default_brand() -> serde_json::Value {
    json!({ "site_name": "Lazybones", "site_abbr": "lb", "tagline": "workspace ops" })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn branding_blob_round_trips_unchanged() {
    let store = Store::memory().await.unwrap();
    let p = Prefs {
        ui_branding: Some(acme_brand()),
        ..Prefs::default()
    };
    set_workspace_prefs(&store, "acme", &p).await.unwrap();

    // set_default writes the workspace-default record; resolve returns it for a member who set
    // nothing. (Branding is admin-owned — only the ws-default link ever carries it.)
    let r = resolve_chain(&store, "acme", "user:ada", None)
        .await
        .unwrap();
    assert_eq!(
        r.ui_branding,
        Some(acme_brand()),
        "the opaque branding blob round-trips byte-for-byte through option<object>"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn branding_patch_leaves_i18n_axes_untouched() {
    // Setting ONLY ui_branding must not clear a previously-set i18n axis (MERGE semantics).
    let store = Store::memory().await.unwrap();
    set_workspace_prefs(
        &store,
        "acme",
        &Prefs {
            language: Some("es".into()),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();
    set_workspace_prefs(
        &store,
        "acme",
        &Prefs {
            ui_branding: Some(acme_brand()),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();

    let r = resolve_chain(&store, "acme", "user:ada", None)
        .await
        .unwrap();
    assert_eq!(r.language, "es", "language untouched across patches");
    assert_eq!(r.ui_branding, Some(acme_brand()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn branding_does_not_merge_with_member_ui_theme_patch() {
    // A member who set a ui_theme (a member axis) still resolves the workspace's ui_branding
    // (an admin axis) — the two ride independent axes on the same record.
    let store = Store::memory().await.unwrap();
    set_workspace_prefs(
        &store,
        "acme",
        &Prefs {
            ui_branding: Some(acme_brand()),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();
    set_user_prefs(
        &store,
        "acme",
        "user:ada",
        &Prefs {
            ui_theme: Some(json!({ "mode": "dark" })),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();

    let r = resolve_chain(&store, "acme", "user:ada", None)
        .await
        .unwrap();
    assert_eq!(r.ui_branding, Some(acme_brand()));
    assert_eq!(r.ui_theme, Some(json!({ "mode": "dark" })));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn no_branding_anywhere_resolves_none() {
    // No workspace default set → None (the shell falls back to its compiled Lazybones default).
    let store = Store::memory().await.unwrap();
    let r = resolve_chain(&store, "acme", "user:ada", None)
        .await
        .unwrap();
    assert_eq!(r.ui_branding, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn branding_is_workspace_isolated() {
    // Mandatory isolation: ws-A's brand never appears in ws-B's resolve, and ws-B's raw read
    // never surfaces ws-A's blob.
    let store = Store::memory().await.unwrap();
    let brand_a = acme_brand();
    let brand_b = default_brand();

    set_workspace_prefs(
        &store,
        "ws-a",
        &Prefs {
            ui_branding: Some(brand_a.clone()),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();
    set_workspace_prefs(
        &store,
        "ws-b",
        &Prefs {
            ui_branding: Some(brand_b.clone()),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();

    let rb = resolve_chain(&store, "ws-b", "user:ada", None)
        .await
        .unwrap();
    assert_eq!(
        rb.ui_branding,
        Some(brand_b),
        "ws-B resolves ws-B's brand only"
    );
    let ra = resolve_chain(&store, "ws-a", "user:ada", None)
        .await
        .unwrap();
    assert_eq!(
        ra.ui_branding,
        Some(brand_a),
        "ws-A resolves ws-A's brand only"
    );

    // Raw read in ws-B never surfaces ws-A's blob.
    let mut resp = store
        .query_ws(
            "ws-b",
            &format!("SELECT ui_branding FROM {USER_PREFS_TABLE} WHERE ui_branding IS NOT NONE"),
            vec![],
        )
        .await
        .unwrap();
    let rows: Vec<serde_json::Value> = resp.take(0).unwrap();
    let blob = format!("{rows:?}");
    assert!(
        !blob.contains("Acme"),
        "ws-A's brand never appears in ws-B's read"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn user_prefs_ui_branding_is_round_tripped_but_member_never_writes_it() {
    // The member-level record carries the axis (schema is symmetric), even though in production
    // only the workspace-default link ever holds a brand. This proves the axis is read-back
    // faithfully at the user_prefs shape too — a future member-local brand preview would not need
    // a schema change.
    let store = Store::memory().await.unwrap();
    set_user_prefs(
        &store,
        "acme",
        "user:ada",
        &Prefs {
            ui_branding: Some(acme_brand()),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();
    let got = get_user_prefs(&store, "acme", "user:ada")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.ui_branding, Some(acme_brand()));
}
