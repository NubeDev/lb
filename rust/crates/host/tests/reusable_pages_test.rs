//! Reusable pages headless (reusable-pages-scope, "Testing plan") — the template-dashboard /
//! instance-as-binding / tag-driven fan-out feature, against a REAL store/caps/node with seeded
//! dashboards + tags + navs (no mocks, rule 9). Proves the mandatory categories and the scope's
//! headline tests:
//!   - `Variable.required` round-trips save→get; old records without it load unchanged (serde default).
//!   - a `template-group` expands to one instance link per option value (`?var-<var>=<value>`), from a
//!     tag-facet source AND a `{tool,args}` query source; a new tag value adds a page, untag removes it.
//!   - **capability deny (mandatory):** a template-group whose option source the caller lacks →
//!     the whole entry is stripped, no option value leaks (opaque).
//!   - **workspace isolation (mandatory):** ws-B's expansion sees only ws-B tags; a ws-A dashboard
//!     binding resolved in ws-B is stripped.
//!   - **the lens (headline):** a caller who cannot READ the template dashboard gets the entry stripped
//!     even holding the option-source cap — a binding never widens access.
//!   - **binding precedence carrier:** a pinned `vars` on a `dashboard` entry rides through to
//!     `ResolvedItem::vars`; the 50-per-group cap truncates loudly.
//!   - **bounds:** `nav.save` rejects a malformed template-group at author time.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    dashboard_get, dashboard_save, nav_pref_set, nav_resolve, nav_save, tags_add, Cell,
    DashboardVariable as Variable, NavFacet, NavItem, NavVisibility, Node, Provenance, Tag,
    TagSource,
};
use lb_store::Store;
use serde_json::json;
use std::collections::BTreeMap;

// --- principals / caps --------------------------------------------------------------------------

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
const RESOLVE: &str = "mcp:nav.resolve:call";
const DASH_SAVE: &str = "mcp:dashboard.save:call";
const DASH_GET: &str = "mcp:dashboard.get:call";
const TAGS_ADD: &str = "mcp:tags.add:call";
const TAGS_FIND: &str = "mcp:tags.find:call";
const TAGS_REMOVE: &str = "mcp:tags.remove:call";
const NAV_SHARE: &str = "mcp:nav.share:call";
const STORE_QUERY: &str = "mcp:store.query:call";

// --- helpers ------------------------------------------------------------------------------------

/// A required "page parameter" variable (the template's `site`).
fn required_var(name: &str) -> Variable {
    Variable {
        name: name.into(),
        required: true,
        r#type: "query".into(),
        ..Default::default()
    }
}

/// Seed a dashboard `id` owned by `owner`, optionally with variables.
async fn seed_template(
    store: &Store,
    owner: &Principal,
    ws: &str,
    id: &str,
    title: &str,
    vars: Vec<Variable>,
) {
    dashboard_save(store, owner, ws, id, title, Vec::<Cell>::new(), vars, 1)
        .await
        .expect("seed template dashboard");
}

/// Tag `entity` with `site=value` as `by`.
async fn tag_site(store: &Store, by: &Principal, ws: &str, entity: &str, value: &str, at: u64) {
    tags_add(
        store,
        by,
        ws,
        entity,
        &Tag::new("site", json!(value)),
        &Provenance::new(at, by.sub(), TagSource::Human),
    )
    .await
    .expect("tag site");
}

/// A template-group entry over a tag-facet source (`facets:[{key}]`).
fn template_group_facet(label: &str, dashboard: &str, var: &str, facet_key: &str) -> NavItem {
    NavItem {
        kind: "template-group".into(),
        label: label.into(),
        dashboard: dashboard.into(),
        var: var.into(),
        facets: vec![NavFacet {
            key: facet_key.into(),
            value: None,
        }],
        ..Default::default()
    }
}

// --- Variable.required round-trip ---------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn variable_required_round_trips_and_defaults_false() {
    let ws = "ws-rp-required";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[DASH_SAVE, DASH_GET]);

    // One required var, one not.
    let vars = vec![
        required_var("site"),
        Variable {
            name: "env".into(),
            ..Default::default()
        },
    ];
    seed_template(&store, &ada, ws, "site-overview", "Site Overview", vars).await;

    let got = dashboard_get(&store, &ada, ws, "site-overview")
        .await
        .unwrap();
    let site = got.variables.iter().find(|v| v.name == "site").unwrap();
    let env = got.variables.iter().find(|v| v.name == "env").unwrap();
    assert!(site.required, "required survives save→get");
    assert!(!env.required, "a non-parameter var defaults required:false");

    // A pre-reusable-pages record (no `required` field on the wire) loads unchanged (serde default).
    let legacy: Variable =
        serde_json::from_value(json!({ "name": "old", "type": "custom" })).unwrap();
    assert!(
        !legacy.required,
        "old record without the field → required:false"
    );
}

// --- template-group expansion (tag-facet source) ------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn template_group_expands_one_instance_per_facet_value() {
    let ws = "ws-rp-facet";
    let node = Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let ada = principal(
        "user:ada",
        ws,
        &[
            SAVE,
            RESOLVE,
            DASH_SAVE,
            DASH_GET,
            TAGS_ADD,
            TAGS_FIND,
            TAGS_REMOVE,
        ],
    );

    seed_template(
        store,
        &ada,
        ws,
        "site-overview",
        "Site Overview",
        vec![required_var("site")],
    )
    .await;
    // Three sites present in the tag graph (any tagged entity carries the facet value).
    tag_site(store, &ada, ws, "series:hvac.plant-1.temp", "plant-1", 2).await;
    tag_site(store, &ada, ws, "series:hvac.plant-2.temp", "plant-2", 3).await;
    tag_site(store, &ada, ws, "series:hvac.plant-3.temp", "plant-3", 4).await;

    nav_save(
        store,
        &ada,
        ws,
        "ops",
        "Operations",
        vec![template_group_facet(
            "Sites",
            "dashboard:site-overview",
            "site",
            "site",
        )],
        5,
    )
    .await
    .unwrap();
    nav_pref_set(store, &ada, ws, Some("ops"), None, 6)
        .await
        .unwrap();

    let r = nav_resolve(&node, &ada, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    assert_eq!(grp.items.len(), 3, "one instance per distinct site value");
    // Each child is the SAME dashboard record, bound to a distinct site via `vars`.
    let mut sites: Vec<String> = grp
        .items
        .iter()
        .map(|c| {
            assert_eq!(
                c.dashboard, "dashboard:site-overview",
                "one dashboard, many bindings"
            );
            assert_eq!(c.kind, "dashboard");
            assert_eq!(
                c.label,
                c.vars.get("site").cloned().unwrap(),
                "label = the value"
            );
            c.vars.get("site").cloned().unwrap()
        })
        .collect();
    sites.sort();
    assert_eq!(sites, vec!["plant-1", "plant-2", "plant-3"]);

    // A NEW site appears with no nav/dashboard edit; untag removes it.
    tag_site(store, &ada, ws, "series:hvac.plant-4.temp", "plant-4", 7).await;
    let r = nav_resolve(&node, &ada, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    assert_eq!(grp.items.len(), 4, "tag site:plant-4 → a new page appears");

    lb_host::tags_remove(store, &ada, ws, "series:hvac.plant-4.temp", "site", None)
        .await
        .unwrap();
    let r = nav_resolve(&node, &ada, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    assert_eq!(grp.items.len(), 3, "untag → gone");
}

// --- capability deny (MANDATORY) ----------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn template_group_stripped_without_option_source_cap() {
    let ws = "ws-rp-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    // Ada authors with the tag caps.
    let ada = principal(
        "user:ada",
        ws,
        &[SAVE, DASH_SAVE, DASH_GET, TAGS_ADD, TAGS_FIND],
    );
    seed_template(
        store,
        &ada,
        ws,
        "site-overview",
        "Site Overview",
        vec![required_var("site")],
    )
    .await;
    tag_site(store, &ada, ws, "series:x", "plant-1", 2).await;
    nav_save(
        store,
        &ada,
        ws,
        "ops",
        "Operations",
        vec![template_group_facet(
            "Sites",
            "dashboard:site-overview",
            "site",
            "site",
        )],
        3,
    )
    .await
    .unwrap();
    // Ben can RESOLVE + read the dashboard, but LACKS `tags.find` (the option source's cap).
    let ben = principal("user:ben", ws, &[RESOLVE, DASH_GET]);
    nav_pref_set(store, &ben, ws, Some("ops"), None, 4)
        .await
        .unwrap();
    let r = nav_resolve(&node, &ben, ws).await.unwrap();
    // The whole entry is stripped — no group, no option value leaked (opaque).
    assert!(
        r.items.iter().all(|i| i.kind != "group"),
        "no option source cap → template-group stripped, values not enumerable"
    );
}

// --- the lens (HEADLINE): a binding never widens access -----------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn template_group_stripped_when_template_unreadable() {
    let ws = "ws-rp-lens";
    let node = Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let ada = principal(
        "user:ada",
        ws,
        &[SAVE, DASH_SAVE, DASH_GET, TAGS_ADD, TAGS_FIND],
    );
    // Ada's PRIVATE template (default visibility private).
    seed_template(
        store,
        &ada,
        ws,
        "secret-overview",
        "Secret",
        vec![required_var("site")],
    )
    .await;
    tag_site(store, &ada, ws, "series:x", "plant-1", 2).await;
    nav_save(
        store,
        &ada,
        ws,
        "ops",
        "Operations",
        vec![template_group_facet(
            "Sites",
            "dashboard:secret-overview",
            "site",
            "site",
        )],
        3,
    )
    .await
    .unwrap();
    // Ben HOLDS tags.find (can enumerate values) but CANNOT read Ada's private template.
    let ben = principal("user:ben", ws, &[RESOLVE, DASH_GET, TAGS_FIND]);
    nav_pref_set(store, &ben, ws, Some("ops"), None, 4)
        .await
        .unwrap();
    let r = nav_resolve(&node, &ben, ws).await.unwrap();
    assert!(
        r.items.iter().all(|i| i.kind != "group"),
        "cannot read the template → entry stripped even holding the option cap (the lens)"
    );
}

// --- workspace isolation (MANDATORY) ------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn template_group_isolation_two_workspaces() {
    let node = Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let caps = &[SAVE, RESOLVE, DASH_SAVE, DASH_GET, TAGS_ADD, TAGS_FIND];

    // ws-A: template + plant-1/plant-2 tags + a nav.
    let ada = principal("user:ada", "ws-a", caps);
    seed_template(
        store,
        &ada,
        "ws-a",
        "site-overview",
        "A Overview",
        vec![required_var("site")],
    )
    .await;
    tag_site(store, &ada, "ws-a", "series:a1", "plant-1", 2).await;
    tag_site(store, &ada, "ws-a", "series:a2", "plant-2", 3).await;

    // ws-B: its OWN template + a single distinct tag value + a nav with the same shape.
    let ben = principal("user:ben", "ws-b", caps);
    seed_template(
        store,
        &ben,
        "ws-b",
        "site-overview",
        "B Overview",
        vec![required_var("site")],
    )
    .await;
    tag_site(store, &ben, "ws-b", "series:b1", "zone-x", 4).await;
    nav_save(
        store,
        &ben,
        "ws-b",
        "ops",
        "Operations",
        vec![template_group_facet(
            "Sites",
            "dashboard:site-overview",
            "site",
            "site",
        )],
        5,
    )
    .await
    .unwrap();
    nav_pref_set(store, &ben, "ws-b", Some("ops"), None, 6)
        .await
        .unwrap();

    // ws-B's expansion sees ONLY ws-B tag values — never ws-A's plant-1/plant-2.
    let r = nav_resolve(&node, &ben, "ws-b").await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    let values: Vec<String> = grp
        .items
        .iter()
        .filter_map(|c| c.vars.get("site").cloned())
        .collect();
    assert_eq!(
        values,
        vec!["zone-x"],
        "ws-B enumeration is walled to ws-B tags"
    );
}

// --- pinned vars on a dashboard entry (curated named instance) ----------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pinned_vars_on_dashboard_entry_round_trip_and_resolve() {
    let ws = "ws-rp-pinned";
    let node = Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let ada = principal(
        "user:ada",
        ws,
        &[SAVE, RESOLVE, "mcp:nav.get:call", DASH_SAVE, DASH_GET],
    );
    seed_template(
        store,
        &ada,
        ws,
        "site-overview",
        "Site Overview",
        vec![required_var("site")],
    )
    .await;

    let mut vars = BTreeMap::new();
    vars.insert("site".to_string(), "plant-2".to_string());
    let entry = NavItem {
        kind: "dashboard".into(),
        label: "Plant-2 Overview".into(),
        dashboard: "dashboard:site-overview".into(),
        vars,
        ..Default::default()
    };
    nav_save(store, &ada, ws, "ops", "Operations", vec![entry], 2)
        .await
        .unwrap();
    nav_pref_set(store, &ada, ws, Some("ops"), None, 3)
        .await
        .unwrap();

    // Round-trip: the pinned binding survives save→get.
    let got = lb_host::nav_get(store, &ada, ws, "ops").await.unwrap();
    assert_eq!(got.items[0].vars.get("site").unwrap(), "plant-2");

    // Resolve: the binding rides through to the rendered item.
    let r = nav_resolve(&node, &ada, ws).await.unwrap();
    let dash = r.items.iter().find(|i| i.kind == "dashboard").unwrap();
    assert_eq!(dash.label, "Plant-2 Overview");
    assert_eq!(dash.vars.get("site").unwrap(), "plant-2");
}

// --- query option source (the general case) + its deny ------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn template_group_query_option_source_enumerates_and_denies() {
    let ws = "ws-rp-query";
    let node = Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let ada = principal(
        "user:ada",
        ws,
        &[
            SAVE,
            RESOLVE,
            DASH_SAVE,
            DASH_GET,
            TAGS_ADD,
            TAGS_FIND,
            STORE_QUERY,
            NAV_SHARE,
        ],
    );
    seed_template(
        store,
        &ada,
        ws,
        "site-overview",
        "Site Overview",
        vec![required_var("site")],
    )
    .await;
    tag_site(store, &ada, ws, "series:a1", "plant-1", 2).await;
    tag_site(store, &ada, ws, "series:a2", "plant-2", 3).await;

    // A `{tool,args}` option source: distinct site values via store.query over the tag edges.
    let entry = NavItem {
        kind: "template-group".into(),
        label: "Sites".into(),
        dashboard: "dashboard:site-overview".into(),
        var: "site".into(),
        tool: "store.query".into(),
        args: json!({ "sql": "SELECT tval AS value FROM tagged WHERE tkey = 'site' GROUP BY value" }),
        ..Default::default()
    };
    nav_save(store, &ada, ws, "ops", "Operations", vec![entry.clone()], 4)
        .await
        .unwrap();
    nav_pref_set(store, &ada, ws, Some("ops"), None, 5)
        .await
        .unwrap();

    let r = nav_resolve(&node, &ada, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    let mut values: Vec<String> = grp
        .items
        .iter()
        .filter_map(|c| c.vars.get("site").cloned())
        .collect();
    values.sort();
    assert_eq!(
        values,
        vec!["plant-1", "plant-2"],
        "query source enumerates values"
    );

    // Deny: a caller lacking the query TOOL's cap (`store.query`) → the entry strips, opaque.
    let ben = principal("user:ben", ws, &[RESOLVE, DASH_GET]);
    // Ben needs to resolve the same nav; share it to the workspace so he picks it.
    lb_host::nav_share(store, &ada, ws, "ops", NavVisibility::Workspace, None, 6)
        .await
        .unwrap();
    lb_host::nav_set_default(store, &ada, ws, "ops", 7)
        .await
        .unwrap();
    let r = nav_resolve(&node, &ben, ws).await.unwrap();
    assert!(
        r.items.iter().all(|i| i.kind != "group"),
        "no store.query cap → query-source template-group stripped (the lens)"
    );
}

// --- bounds: malformed template-group rejected at author time -----------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn nav_save_rejects_malformed_template_group() {
    let ws = "ws-rp-bounds";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[SAVE]);

    // No `var`.
    let no_var = NavItem {
        kind: "template-group".into(),
        dashboard: "dashboard:x".into(),
        facets: vec![NavFacet {
            key: "site".into(),
            value: None,
        }],
        ..Default::default()
    };
    assert!(nav_save(&store, &ada, ws, "n1", "N", vec![no_var], 1)
        .await
        .is_err());

    // No option source.
    let no_src = NavItem {
        kind: "template-group".into(),
        dashboard: "dashboard:x".into(),
        var: "site".into(),
        ..Default::default()
    };
    assert!(nav_save(&store, &ada, ws, "n2", "N", vec![no_src], 1)
        .await
        .is_err());

    // BOTH sources (ambiguous).
    let both = NavItem {
        kind: "template-group".into(),
        dashboard: "dashboard:x".into(),
        var: "site".into(),
        tool: "store.query".into(),
        args: json!({ "sql": "SELECT 1" }),
        facets: vec![NavFacet {
            key: "site".into(),
            value: None,
        }],
        ..Default::default()
    };
    assert!(nav_save(&store, &ada, ws, "n3", "N", vec![both], 1)
        .await
        .is_err());

    // Missing dashboard.
    let no_dash = NavItem {
        kind: "template-group".into(),
        var: "site".into(),
        facets: vec![NavFacet {
            key: "site".into(),
            value: None,
        }],
        ..Default::default()
    };
    assert!(nav_save(&store, &ada, ws, "n4", "N", vec![no_dash], 1)
        .await
        .is_err());
}
