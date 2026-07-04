//! The `ui_theme` axis (theme-customizer scope) over a REAL `mem://` store: the opaque theme blob
//! round-trips through set/get, folds WHOLE in the resolve chain (a member's theme wins entirely
//! over the workspace default, never merges), a workspace default fills in for a member who set no
//! theme, and — the mandatory isolation category — a member's theme in ws-A is never read in ws-B.
//!
//! `ui_theme` is deliberately opaque to prefs (the frontend `ThemePreference`): these tests treat it
//! as an arbitrary JSON object and only assert it moves through the store/fold unchanged.

use lb_prefs::{
    get_user_prefs, resolve_chain, set_user_prefs, set_workspace_prefs, Prefs, USER_PREFS_TABLE,
};
use lb_store::Store;
use serde_json::json;

fn member_theme() -> serde_json::Value {
    json!({ "mode": "dark", "preset": "teal", "radius": "0.75rem" })
}

fn ws_default_theme() -> serde_json::Value {
    json!({ "mode": "light", "preset": "amber", "radius": "0.5rem" })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn theme_blob_round_trips_unchanged() {
    let store = Store::memory().await.unwrap();
    let p = Prefs {
        ui_theme: Some(member_theme()),
        ..Prefs::default()
    };
    set_user_prefs(&store, "acme", "user:ada", &p)
        .await
        .unwrap();

    let got = get_user_prefs(&store, "acme", "user:ada")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        got.ui_theme,
        Some(member_theme()),
        "the opaque theme blob round-trips byte-for-byte through option<object>"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn theme_patch_leaves_i18n_axes_untouched() {
    // Setting ONLY ui_theme must not clear a previously-set i18n axis (MERGE semantics).
    let store = Store::memory().await.unwrap();
    set_user_prefs(
        &store,
        "acme",
        "user:ada",
        &Prefs {
            language: Some("es".into()),
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
            ui_theme: Some(member_theme()),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();

    let got = get_user_prefs(&store, "acme", "user:ada")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.language, Some("es".to_string()), "language untouched");
    assert_eq!(got.ui_theme, Some(member_theme()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn member_theme_wins_whole_over_workspace_default() {
    // The member set a theme; the workspace also has a default. Resolve returns the MEMBER's theme
    // entirely (whole-fold, not a per-field merge of the two).
    let store = Store::memory().await.unwrap();
    set_workspace_prefs(
        &store,
        "acme",
        &Prefs {
            ui_theme: Some(ws_default_theme()),
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
            ui_theme: Some(member_theme()),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();

    let r = resolve_chain(&store, "acme", "user:ada", None)
        .await
        .unwrap();
    assert_eq!(
        r.ui_theme,
        Some(member_theme()),
        "member theme resolves whole — no merge with the ws default's light/amber"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_default_fills_in_for_member_with_no_theme() {
    // A member who set no theme resolves to the workspace default theme (the "branded house theme
    // everyone sees on first load" property).
    let store = Store::memory().await.unwrap();
    set_workspace_prefs(
        &store,
        "acme",
        &Prefs {
            ui_theme: Some(ws_default_theme()),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();

    let r = resolve_chain(&store, "acme", "user:bob", None)
        .await
        .unwrap();
    assert_eq!(r.ui_theme, Some(ws_default_theme()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn no_theme_anywhere_resolves_none() {
    // Neither member nor workspace set a theme → None (the shell falls back to its compiled default).
    let store = Store::memory().await.unwrap();
    let r = resolve_chain(&store, "acme", "user:ada", None)
        .await
        .unwrap();
    assert_eq!(r.ui_theme, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn theme_is_workspace_isolated() {
    // Mandatory isolation: the SAME global user has a DIFFERENT theme in ws-A and ws-B; a resolve in
    // ws-B returns ws-B's theme and can never read ws-A's.
    let store = Store::memory().await.unwrap();
    let user = "user:ada";
    let theme_a = json!({ "mode": "dark", "preset": "blue", "radius": "0" });
    let theme_b = json!({ "mode": "light", "preset": "teal", "radius": "1rem" });

    set_user_prefs(
        &store,
        "ws-a",
        user,
        &Prefs {
            ui_theme: Some(theme_a.clone()),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();
    set_user_prefs(
        &store,
        "ws-b",
        user,
        &Prefs {
            ui_theme: Some(theme_b.clone()),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();

    let rb = resolve_chain(&store, "ws-b", user, None).await.unwrap();
    assert_eq!(
        rb.ui_theme,
        Some(theme_b),
        "ws-B resolves ws-B's theme only"
    );
    let ra = resolve_chain(&store, "ws-a", user, None).await.unwrap();
    assert_eq!(
        ra.ui_theme,
        Some(theme_a),
        "ws-A resolves ws-A's theme only"
    );

    // Raw read in ws-B never surfaces ws-A's blob.
    let mut resp = store
        .query_ws(
            "ws-b",
            &format!("SELECT ui_theme FROM {USER_PREFS_TABLE} WHERE ui_theme IS NOT NONE"),
            vec![],
        )
        .await
        .unwrap();
    let rows: Vec<serde_json::Value> = resp.take(0).unwrap();
    let blob = format!("{rows:?}");
    assert!(!blob.contains("blue"), "ws-A's theme never appears in ws-B");
}
