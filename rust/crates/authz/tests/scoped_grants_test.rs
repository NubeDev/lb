//! Entity-scoped grants (entity-scoped-grants scope): row-level reach inside a workspace.
//! Tests the full surface: scoped `grant_assign`/`grant_revoke`, `resolve_caps_scoped` (scope
//! union), `check_scoped` (point check), `scope_filter` (query-side filter), plus the mandatory
//! capability-deny and workspace-isolation tests. Real store, real resolver — no mocks.

use lb_authz::{
    grant_assign_scoped, grant_list_scoped, grant_revoke_scoped, resolve_caps_scoped, Scope,
    ScopedCap, Subject,
};
use lb_store::Store;

const WS: &str = "acme";

fn ids(table: &str, ids: &[&str]) -> Scope {
    Scope::Ids {
        table: table.into(),
        ids: ids.iter().map(|s| s.to_string()).collect(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scoped_grant_narrows_check_scoped() {
    let store = Store::memory().await.unwrap();
    let ana = Subject::User("ana".into());
    let cap = "mcp:care.log.list:call";

    grant_assign_scoped(&store, WS, &ana, cap, &ids("child", &["leo"]))
        .await
        .unwrap();

    // Ana can reach leo, not mia.
    assert!(
        lb_authz::check_scoped(&store, WS, "ana", cap, "child", "leo")
            .await
            .unwrap()
    );
    assert!(
        !lb_authz::check_scoped(&store, WS, "ana", cap, "child", "mia")
            .await
            .unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn all_scope_grant_allows_any_record() {
    let store = Store::memory().await.unwrap();
    let bob = Subject::User("bob".into());
    let cap = "mcp:care.log.list:call";

    grant_assign_scoped(&store, WS, &bob, cap, &Scope::All)
        .await
        .unwrap();

    assert!(
        lb_authz::check_scoped(&store, WS, "bob", cap, "child", "leo")
            .await
            .unwrap()
    );
    assert!(
        lb_authz::check_scoped(&store, WS, "bob", cap, "child", "mia")
            .await
            .unwrap()
    );
    assert!(
        lb_authz::check_scoped(&store, WS, "bob", cap, "site", "north")
            .await
            .unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scope_filter_returns_ids_for_scoped_grant() {
    let store = Store::memory().await.unwrap();
    let ana = Subject::User("ana".into());
    let cap = "mcp:care.log.list:call";

    grant_assign_scoped(&store, WS, &ana, cap, &ids("child", &["leo", "mia"]))
        .await
        .unwrap();

    let filter = lb_authz::scope_filter(&store, WS, "ana", cap, "child")
        .await
        .unwrap();
    match filter {
        lb_authz::ScopeFilter::Ids(ids) => {
            assert_eq!(ids.len(), 2);
            assert!(ids.contains(&"leo".to_string()));
            assert!(ids.contains(&"mia".to_string()));
        }
        lb_authz::ScopeFilter::All => panic!("expected Ids, got All"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scope_filter_returns_all_for_all_grant() {
    let store = Store::memory().await.unwrap();
    let bob = Subject::User("bob".into());
    let cap = "mcp:care.log.list:call";

    grant_assign_scoped(&store, WS, &bob, cap, &Scope::All)
        .await
        .unwrap();

    let filter = lb_authz::scope_filter(&store, WS, "bob", cap, "child")
        .await
        .unwrap();
    assert_eq!(filter, lb_authz::ScopeFilter::All);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scope_filter_returns_empty_for_different_table() {
    let store = Store::memory().await.unwrap();
    let ana = Subject::User("ana".into());
    let cap = "mcp:care.log.list:call";

    grant_assign_scoped(&store, WS, &ana, cap, &ids("child", &["leo"]))
        .await
        .unwrap();

    // Ask for the site table — ana's cap is scoped to child, so no sites are reachable.
    let filter = lb_authz::scope_filter(&store, WS, "ana", cap, "site")
        .await
        .unwrap();
    assert_eq!(filter, lb_authz::ScopeFilter::Ids(vec![]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scope_filter_returns_empty_for_unheld_cap() {
    let store = Store::memory().await.unwrap();

    // No grants at all — a cap the principal doesn't hold → empty, not error.
    let filter = lb_authz::scope_filter(&store, WS, "ana", "mcp:care.log.list:call", "child")
        .await
        .unwrap();
    assert_eq!(filter, lb_authz::ScopeFilter::Ids(vec![]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn union_of_multiple_scoped_grants_merges_ids() {
    let store = Store::memory().await.unwrap();
    let sam = Subject::User("sam".into());
    let cap = "mcp:care.log.list:call";

    // Sam has edges to Leo AND Mia — two scoped grants that union.
    grant_assign_scoped(&store, WS, &sam, cap, &ids("child", &["leo"]))
        .await
        .unwrap();
    grant_assign_scoped(&store, WS, &sam, cap, &ids("child", &["mia"]))
        .await
        .unwrap();

    let scoped = resolve_caps_scoped(&store, WS, "sam").await.unwrap();
    let care_cap = scoped
        .iter()
        .find(|sc: &&ScopedCap| sc.cap == cap)
        .expect("cap must be present");

    match &care_cap.scope {
        Scope::Ids { table, ids } => {
            assert_eq!(table, "child");
            assert_eq!(ids.len(), 2);
            assert!(ids.contains(&"leo".to_string()));
            assert!(ids.contains(&"mia".to_string()));
        }
        other => panic!("expected Ids union, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn union_across_tables_reaches_only_granted_rows_not_everything() {
    // Regression (review fix): child:[leo] + site:[north] used to widen to Scope::All. The union
    // must reach exactly those two rows and nothing else, in every table.
    let store = Store::memory().await.unwrap();
    let sam = Subject::User("sam".into());
    let cap = "mcp:care.log.list:call";
    grant_assign_scoped(&store, WS, &sam, cap, &ids("child", &["leo"]))
        .await
        .unwrap();
    grant_assign_scoped(&store, WS, &sam, cap, &ids("site", &["north"]))
        .await
        .unwrap();

    for (table, id, want) in [
        ("child", "leo", true),
        ("site", "north", true),
        ("child", "mia", false),
        ("site", "south", false),
        ("project", "leo", false),
    ] {
        let ok = lb_authz::check_scoped(&store, WS, "sam", cap, table, id)
            .await
            .unwrap();
        assert_eq!(ok, want, "check_scoped({table}, {id})");
    }
    // Query-side filter stays per-table — never All.
    let filter = lb_authz::scope_filter(&store, WS, "sam", cap, "child")
        .await
        .unwrap();
    assert_eq!(filter, lb_authz::ScopeFilter::Ids(vec!["leo".into()]));
    let filter = lb_authz::scope_filter(&store, WS, "sam", cap, "project")
        .await
        .unwrap();
    assert_eq!(filter, lb_authz::ScopeFilter::Ids(vec![]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn all_grant_wins_over_scoped_grants() {
    let store = Store::memory().await.unwrap();
    let sam = Subject::User("sam".into());
    let cap = "mcp:care.log.list:call";

    // Sam has a scoped grant AND an All grant — All wins.
    grant_assign_scoped(&store, WS, &sam, cap, &ids("child", &["leo"]))
        .await
        .unwrap();
    grant_assign_scoped(&store, WS, &sam, cap, &Scope::All)
        .await
        .unwrap();

    let scoped = resolve_caps_scoped(&store, WS, "sam").await.unwrap();
    let care_cap = scoped
        .iter()
        .find(|sc: &&ScopedCap| sc.cap == cap)
        .expect("cap must be present");
    assert_eq!(care_cap.scope, Scope::All);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn revoke_scoped_grant_denies_after_revoke() {
    let store = Store::memory().await.unwrap();
    let ana = Subject::User("ana".into());
    let cap = "mcp:care.log.list:call";
    let scope = ids("child", &["leo"]);

    grant_assign_scoped(&store, WS, &ana, cap, &scope)
        .await
        .unwrap();
    assert!(
        lb_authz::check_scoped(&store, WS, "ana", cap, "child", "leo")
            .await
            .unwrap()
    );

    // Revoke the scoped grant — next resolution excludes leo.
    grant_revoke_scoped(&store, WS, &ana, cap, &scope)
        .await
        .unwrap();
    assert!(
        !lb_authz::check_scoped(&store, WS, "ana", cap, "child", "leo")
            .await
            .unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn revoking_one_scope_keeps_the_other() {
    let store = Store::memory().await.unwrap();
    let sam = Subject::User("sam".into());
    let cap = "mcp:care.log.list:call";
    let leo_scope = ids("child", &["leo"]);
    let mia_scope = ids("child", &["mia"]);

    grant_assign_scoped(&store, WS, &sam, cap, &leo_scope)
        .await
        .unwrap();
    grant_assign_scoped(&store, WS, &sam, cap, &mia_scope)
        .await
        .unwrap();

    // Revoke the leo scope only.
    grant_revoke_scoped(&store, WS, &sam, cap, &leo_scope)
        .await
        .unwrap();

    assert!(
        !lb_authz::check_scoped(&store, WS, "sam", cap, "child", "leo")
            .await
            .unwrap()
    );
    assert!(
        lb_authz::check_scoped(&store, WS, "sam", cap, "child", "mia")
            .await
            .unwrap()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn empty_scope_lists_return_empty_not_error() {
    let store = Store::memory().await.unwrap();

    // A principal with no grants → scope_filter returns empty Ids, not an error.
    let filter = lb_authz::scope_filter(&store, WS, "nobody", "mcp:x.y:call", "child")
        .await
        .unwrap();
    assert_eq!(filter, lb_authz::ScopeFilter::Ids(vec![]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn grant_list_scoped_returns_full_records() {
    let store = Store::memory().await.unwrap();
    let ana = Subject::User("ana".into());
    let cap = "mcp:care.log.list:call";

    grant_assign_scoped(&store, WS, &ana, cap, &ids("child", &["leo"]))
        .await
        .unwrap();
    grant_assign_scoped(&store, WS, &ana, "mcp:other:call", &Scope::All)
        .await
        .unwrap();

    let grants = grant_list_scoped(&store, WS, &ana).await.unwrap();
    assert_eq!(grants.len(), 2);
    let care = grants.iter().find(|g| g.cap == cap).unwrap();
    assert_eq!(care.scope, ids("child", &["leo"]));
    let other = grants.iter().find(|g| g.cap == "mcp:other:call").unwrap();
    assert_eq!(other.scope, Scope::All);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn old_grant_record_without_scope_field_deserializes_to_all() {
    // Simulate an old grant record (pre-scope) stored without a `scope` field.
    let store = Store::memory().await.unwrap();
    let ana = Subject::User("ana".into());
    let cap = "mcp:care.log.list:call";

    // Write a raw record with no scope field (as old data would have).
    let old_record = serde_json::json!({ "subject": ana.as_key(), "cap": cap });
    lb_store::write(
        &store,
        WS,
        "grant",
        &format!("{}::{}", ana.as_key(), cap),
        &old_record,
    )
    .await
    .unwrap();

    // The grant loads with scope=All (zero migration).
    let grants = grant_list_scoped(&store, WS, &ana).await.unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].cap, cap);
    assert_eq!(grants[0].scope, Scope::All);

    // And check_scoped allows any record (All behaviour).
    assert!(
        lb_authz::check_scoped(&store, WS, "ana", cap, "child", "leo")
            .await
            .unwrap()
    );
}

// ── Mandatory: workspace isolation ───────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scoped_grants_never_cross_the_workspace_wall() {
    let store = Store::memory().await.unwrap();
    let ana = Subject::User("ana".into());
    let cap = "mcp:care.log.list:call";

    grant_assign_scoped(&store, WS, &ana, cap, &ids("child", &["leo"]))
        .await
        .unwrap();

    // In workspace globex, ana has no grants — the scope doesn't leak.
    assert!(
        !lb_authz::check_scoped(&store, "globex", "ana", cap, "child", "leo")
            .await
            .unwrap()
    );
    let filter = lb_authz::scope_filter(&store, "globex", "ana", cap, "child")
        .await
        .unwrap();
    assert_eq!(filter, lb_authz::ScopeFilter::Ids(vec![]));

    // The scoped resolution in globex is empty.
    let scoped = resolve_caps_scoped(&store, "globex", "ana").await.unwrap();
    assert!(scoped.is_empty());
}

// ── Role-expanded caps are always All scope ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn role_expanded_caps_are_all_scope() {
    let store = Store::memory().await.unwrap();
    let ana = Subject::User("ana".into());

    // Grant a role to ana with... well, roles are granted as `role:<name>` caps.
    // The scope on a role grant is irrelevant — role-expanded caps are All.
    lb_authz::role_define(
        &store,
        WS,
        "caregiver",
        &["mcp:care.log.list:call".to_string()],
    )
    .await
    .unwrap();
    grant_assign_scoped(
        &store,
        WS,
        &ana,
        "role:caregiver",
        &ids("child", &["leo"]), // this scope is IGNORED for role grants
    )
    .await
    .unwrap();

    let scoped = resolve_caps_scoped(&store, WS, "ana").await.unwrap();
    let care_cap = scoped
        .iter()
        .find(|sc: &&ScopedCap| sc.cap == "mcp:care.log.list:call")
        .expect("cap must be present");
    assert_eq!(care_cap.scope, Scope::All);
}
