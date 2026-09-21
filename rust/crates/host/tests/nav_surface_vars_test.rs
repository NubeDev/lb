//! A `surface` entry's `vars` — the page's query parameters (nav-surface-vars scope).
//!
//! What this file pins: the parameters survive BOTH doors — `nav.get` (the record) and `nav.resolve`
//! (the rendered payload). The resolver used to build every surface entry with an empty `vars`, so a
//! saved parameter reached the store and silently never reached the menu.

use std::collections::BTreeMap;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{nav_get, nav_pref_set, nav_resolve, nav_save, NavItem};

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

fn surface_item(label: &str, surface: &str, vars: &[(&str, &str)]) -> NavItem {
    NavItem {
        kind: "surface".into(),
        label: label.into(),
        surface: surface.into(),
        vars: vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn surface_vars_survive_both_doors() {
    let ws = "ws-nav-surface-vars";
    let node = Arc::new(lb_host::Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", ws, &[SAVE, GET, RESOLVE]);

    nav_save(
        store,
        &test,
        ws,
        "ops",
        "Ops",
        vec![
            surface_item(
                "Critical",
                "inbox",
                &[("severity", "critical"), ("tag.site", "chullora")],
            ),
            surface_item("Inbox", "inbox", &[]),
        ],
        10,
    )
    .await
    .unwrap();

    let want: BTreeMap<String, String> = [("severity", "critical"), ("tag.site", "chullora")]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let got = nav_get(store, &test, ws, "ops").await.unwrap();
    assert_eq!(
        got.items[0].vars, want,
        "the store read keeps the parameters"
    );

    nav_pref_set(store, &test, ws, Some("ops"), None, 11)
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
    assert_eq!(
        by("Critical").vars,
        want,
        "ResolvedItem relays the parameters"
    );
    assert!(
        by("Inbox").vars.is_empty(),
        "and only on the entry that has them"
    );
}
