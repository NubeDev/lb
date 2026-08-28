//! The nav FOOTER marker (nav-footer scope) — a top-level entry that renders at the END of the menu's
//! axis (bottom of a rail, trailing end of a top bar) instead of in the tree.
//!
//! What this file pins:
//!   - **the projection trap**: a footer-marked entry survives BOTH doors (`nav.get` = the record,
//!     `nav.resolve` = the rendered payload) — a struct-only change passes a unit round-trip and
//!     still drops the field on the store read (the `home` precedent);
//!   - a footer on a NESTED entry is refused at the door (it is a top-level fact);
//!   - a pre-field record deserializes as `false` — additive, no migration.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    dashboard_save, nav_get, nav_pref_set, nav_resolve, nav_save, Cell, NavError, NavItem,
};
use lb_store::Store;

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const SAVE: &str = "mcp:nav.save:call";
const GET: &str = "mcp:nav.get:call";
const RESOLVE: &str = "mcp:nav.resolve:call";
const DASH_SAVE: &str = "mcp:dashboard.save:call";
const DASH_GET: &str = "mcp:dashboard.get:call";

fn dashboard_item(label: &str, dashboard: &str, footer: bool) -> NavItem {
    NavItem {
        kind: "dashboard".into(),
        label: label.into(),
        dashboard: dashboard.into(),
        footer,
        ..Default::default()
    }
}

async fn seed_dashboard(store: &Store, owner: &Principal, ws: &str, id: &str) {
    dashboard_save(store, owner, ws, id, id, Vec::<Cell>::new(), vec![], 1)
        .await
        .expect("seed dashboard");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn footer_survives_both_doors() {
    let ws = "ws-navfooter-projection";
    let node = Arc::new(lb_host::Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", ws, &[SAVE, GET, RESOLVE, DASH_SAVE, DASH_GET]);
    seed_dashboard(store, &test, ws, "portfolio").await;
    seed_dashboard(store, &test, ws, "support").await;

    nav_save(
        store,
        &test,
        ws,
        "sites",
        "Sites",
        vec![
            dashboard_item("Portfolio", "dashboard:portfolio", false),
            dashboard_item("Support", "dashboard:support", true),
        ],
        10,
    )
    .await
    .unwrap();

    let got = nav_get(store, &test, ws, "sites").await.unwrap();
    assert!(!got.items[0].footer, "an unmarked entry stays unmarked");
    assert!(
        got.items[1].footer,
        "the store read must not drop the marker"
    );

    nav_pref_set(store, &test, ws, Some("sites"), None, 11)
        .await
        .unwrap();
    let resolved = nav_resolve(&node, &test, ws).await.unwrap();
    let by = |l: &str| {
        resolved
            .items
            .iter()
            .find(|i| i.label == l)
            .expect("resolved")
    };
    assert!(by("Support").footer, "ResolvedItem relays the marker");
    assert!(!by("Portfolio").footer, "and only on the marked one");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_nested_footer_is_refused_at_the_door() {
    let ws = "ws-navfooter-nested";
    let node = Arc::new(lb_host::Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", ws, &[SAVE, GET, RESOLVE, DASH_SAVE, DASH_GET]);

    let err = nav_save(
        store,
        &test,
        ws,
        "sites",
        "Sites",
        vec![NavItem {
            kind: "group".into(),
            label: "Folder".into(),
            items: vec![dashboard_item("Support", "dashboard:support", true)],
            ..Default::default()
        }],
        10,
    )
    .await
    .expect_err("a nested footer has no axis to move along");
    match err {
        NavError::BadInput(m) => assert!(m.contains("footer") && m.contains("Support"), "{m}"),
        other => panic!("expected BadInput, got {other:?}"),
    }
}

#[test]
fn a_pre_field_nav_item_deserializes_as_not_footer() {
    let it: NavItem = serde_json::from_str(r#"{"kind":"surface","surface":"channels"}"#).unwrap();
    assert!(!it.footer);
    let s = serde_json::to_string(&it).unwrap();
    assert!(
        !s.contains("footer"),
        "an unmarked item serializes without the key: {s}"
    );
}
