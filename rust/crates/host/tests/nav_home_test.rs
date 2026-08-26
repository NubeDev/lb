//! The nav HOME marker (nav-home scope) — the one entry a nav-narrowed caller lands on, stated in the
//! record instead of guessed from position.
//!
//! What this file pins:
//!   - **the projection trap**, through the path that actually bites: a plain host nav written via
//!     `nav.save`, read back through BOTH doors (`nav.get` = the record, `nav.resolve` = the rendered
//!     payload). A struct-only change passes a unit round-trip and still drops the field on the store
//!     read — the failure already recorded for `queryOptions` / `entity` / `heading` / `titleTemplate`;
//!   - a home marked on a NESTED entry surviving both doors (the marker is not a top-level-only fact);
//!   - the one-home rule refusing an ambiguous menu at the door, counted over every depth;
//!   - a `template-group` fan-out keeping the marker on the AUTHORED group, never on a generated
//!     instance (the `title_template` precedent, inverted: instances inherit a heading, not a home);
//!   - a pre-field record deserializing as `false` — additive, no migration, `SCHEMA_VERSION`
//!     unchanged.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    dashboard_save, nav_get, nav_pref_set, nav_resolve, nav_save, Cell, NavError, NavItem,
    NavResolvedItem,
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
// The resolver STRIPS a dashboard entry whose board the caller cannot read — so a test asserting on
// the resolved payload must grant the read too, or it is asserting on a cap denial by accident.
const DASH_GET: &str = "mcp:dashboard.get:call";

fn dashboard_item(label: &str, dashboard: &str, home: bool) -> NavItem {
    NavItem {
        kind: "dashboard".into(),
        label: label.into(),
        dashboard: dashboard.into(),
        home,
        ..Default::default()
    }
}

fn group(label: &str, items: Vec<NavItem>, home: bool) -> NavItem {
    NavItem {
        kind: "group".into(),
        label: label.into(),
        items,
        home,
        ..Default::default()
    }
}

async fn seed_dashboard(store: &Store, owner: &Principal, ws: &str, id: &str) {
    dashboard_save(store, owner, ws, id, id, Vec::<Cell>::new(), vec![], 1)
        .await
        .expect("seed dashboard");
}

fn find<'a>(items: &'a [NavResolvedItem], label: &str) -> Option<&'a NavResolvedItem> {
    for item in items {
        if item.label == label {
            return Some(item);
        }
        if let Some(hit) = find(&item.items, label) {
            return Some(hit);
        }
    }
    None
}

// --- THE PROJECTION TRAP ------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn home_survives_the_store_read_on_a_plain_host_nav_record() {
    let ws = "ws-navhome-projection";
    let node = Arc::new(lb_host::Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", ws, &[SAVE, GET, RESOLVE, DASH_SAVE, DASH_GET]);
    seed_dashboard(store, &test, ws, "portfolio").await;
    seed_dashboard(store, &test, ws, "energy").await;

    nav_save(
        store,
        &test,
        ws,
        "sites",
        "Sites",
        vec![
            dashboard_item("Energy", "dashboard:energy", false),
            dashboard_item("Portfolio", "dashboard:portfolio", true),
        ],
        10,
    )
    .await
    .unwrap();

    // Door 1 — the record read.
    let got = nav_get(store, &test, ws, "sites").await.unwrap();
    assert!(!got.items[0].home, "an unmarked entry stays unmarked");
    assert!(
        got.items[1].home,
        "the store read must not drop the marker (the projection trap)"
    );

    // Door 2 — the resolved payload the client lands from.
    nav_pref_set(store, &test, ws, Some("sites"), None, 11)
        .await
        .unwrap();
    let resolved = nav_resolve(&node, &test, ws).await.unwrap();
    assert!(
        find(&resolved.items, "Portfolio").expect("resolved").home,
        "ResolvedItem relays the marker, so the client lands on the AUTHORED home"
    );
    assert!(
        !find(&resolved.items, "Energy").expect("resolved").home,
        "and only on that one — position no longer decides"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn home_marked_on_a_nested_entry_survives_both_doors() {
    // The marker is not a top-level-only fact: an operator may well make one site's Overview the
    // landing page, and that entry lives inside a folder.
    let ws = "ws-navhome-nested";
    let node = Arc::new(lb_host::Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", ws, &[SAVE, GET, RESOLVE, DASH_SAVE, DASH_GET]);
    seed_dashboard(store, &test, ws, "overview").await;

    nav_save(
        store,
        &test,
        ws,
        "sites",
        "Sites",
        vec![group(
            "Welshpool",
            vec![dashboard_item("Overview", "dashboard:overview", true)],
            false,
        )],
        10,
    )
    .await
    .unwrap();

    let got = nav_get(store, &test, ws, "sites").await.unwrap();
    assert!(
        got.items[0].items[0].home,
        "nested marker survives the record read"
    );

    nav_pref_set(store, &test, ws, Some("sites"), None, 11)
        .await
        .unwrap();
    let resolved = nav_resolve(&node, &test, ws).await.unwrap();
    assert!(
        find(&resolved.items, "Overview").expect("resolved").home,
        "nested marker survives the resolve"
    );
}

// --- THE ONE-HOME RULE -------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn two_homes_are_refused_at_the_door() {
    // A menu with two homes has no answer to "where does this person land". Refuse the ambiguity
    // rather than invent a tie-break the author never wrote.
    let ws = "ws-navhome-two";
    let node = Arc::new(lb_host::Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", ws, &[SAVE, GET, DASH_SAVE]);

    let err = nav_save(
        store,
        &test,
        ws,
        "sites",
        "Sites",
        vec![
            dashboard_item("A", "dashboard:a", true),
            dashboard_item("B", "dashboard:b", true),
        ],
        10,
    )
    .await
    .expect_err("two homes is refused");
    assert!(
        matches!(err, NavError::BadInput(_)),
        "refused as bad input: {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_second_home_is_seen_at_any_depth() {
    // Counted over every depth — a marker nested in a folder is just as much the home as a top-level
    // one, so hiding the duplicate inside a group must not slip past the rule.
    let ws = "ws-navhome-depth";
    let node = Arc::new(lb_host::Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", ws, &[SAVE, GET, DASH_SAVE]);

    let err = nav_save(
        store,
        &test,
        ws,
        "sites",
        "Sites",
        vec![
            dashboard_item("A", "dashboard:a", true),
            group(
                "Folder",
                vec![dashboard_item("B", "dashboard:b", true)],
                false,
            ),
        ],
        10,
    )
    .await
    .expect_err("a nested duplicate is still a duplicate");
    assert!(
        matches!(err, NavError::BadInput(_)),
        "refused as bad input: {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn exactly_one_home_is_allowed() {
    let ws = "ws-navhome-one";
    let node = Arc::new(lb_host::Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", ws, &[SAVE, GET, DASH_SAVE]);
    seed_dashboard(store, &test, ws, "a").await;

    nav_save(
        store,
        &test,
        ws,
        "sites",
        "Sites",
        vec![
            dashboard_item("A", "dashboard:a", true),
            dashboard_item("B", "dashboard:b", false),
        ],
        10,
    )
    .await
    .expect("one home saves");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_nav_with_no_home_still_saves() {
    // The marker is OPTIONAL: a menu that names none keeps the client's positional pick, which is
    // every menu authored before this field existed.
    let ws = "ws-navhome-none";
    let node = Arc::new(lb_host::Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", ws, &[SAVE, GET, DASH_SAVE]);

    nav_save(
        store,
        &test,
        ws,
        "sites",
        "Sites",
        vec![dashboard_item("A", "dashboard:a", false)],
        10,
    )
    .await
    .expect("no home saves");
    let got = nav_get(store, &test, ws, "sites").await.unwrap();
    assert!(!got.items[0].home);
}

// --- ADDITIVE: A PRE-FIELD RECORD ---------------------------------------------------------------

#[test]
fn a_pre_field_nav_item_deserializes_as_not_home() {
    // Every record written before this field carries no `home` key. It must read as `false` with no
    // migration — the additive posture `SCHEMA_VERSION` staying put depends on.
    let item: NavItem = serde_json::from_str(
        r#"{"kind":"dashboard","label":"Portfolio","dashboard":"dashboard:portfolio"}"#,
    )
    .expect("a pre-field item deserializes");
    assert!(!item.home, "absent means not home, never a default landing");
}

#[test]
fn an_unmarked_item_serializes_without_the_key() {
    // Skipped when false, so a pre-field client reads exactly the payload it read before.
    let json = serde_json::to_string(&dashboard_item("A", "dashboard:a", false)).unwrap();
    assert!(
        !json.contains("home"),
        "unmarked items carry no `home` key: {json}"
    );
    let marked = serde_json::to_string(&dashboard_item("A", "dashboard:a", true)).unwrap();
    assert!(
        marked.contains("\"home\":true"),
        "a marked item states it: {marked}"
    );
}
