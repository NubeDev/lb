//! Nav-context built-ins headless (nav-context-builtins scope, "Testing plan") — the typed carriers
//! that let ANY client resolve `${__nav.*}` / `${__page.*}`, against a REAL `mem://` store and node
//! (no mocks, rule 9). lb adds no templating engine: every assertion here is that a template string
//! survives **byte-identical** through save → store → resolve, and that a template naming something
//! the item cannot bind is refused at the door.
//!
//! What this file pins:
//!   - **the projection trap**, through the path that actually bites: a PLAIN HOST nav record written
//!     via `nav.save`, read back through `nav.resolve`. A struct-only change passes a unit round-trip
//!     while the store read silently drops the field — the failure already recorded for
//!     `queryOptions` / `entity` / `heading` on `Dashboard`;
//!   - a `template-group` fan-out giving EVERY generated instance the group's `title_template`;
//!   - the nav-builder write path enforcing the same cap + the same unbindable-reference reject the
//!     extension manifest path does (rule 10 — the ext seam is not the privileged path);
//!   - a pre-field nav record deserializing as `None` (additive, no migration);
//!   - a templated `heading` / `description` round-tripping byte-identical through
//!     `dashboard_save_meta` / `dashboard_get` — the host expanded nothing;
//!   - existing data: a stored string with a bare literal `$` still saves and loads unchanged.

use std::collections::BTreeMap;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    dashboard_get, dashboard_save, dashboard_save_meta, nav_get, nav_pref_set, nav_resolve,
    nav_save, tags_add, Cell, NavError, NavFacet, NavItem, NavResolvedItem, PageMeta, Provenance,
    Store, Tag, TagSource, NAV_MAX_TITLE_TEMPLATE,
};

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
const GET: &str = "mcp:nav.get:call";
const RESOLVE: &str = "mcp:nav.resolve:call";
const DASH_SAVE: &str = "mcp:dashboard.save:call";
const DASH_GET: &str = "mcp:dashboard.get:call";
const TAGS_ADD: &str = "mcp:tags.add:call";
const TAGS_FIND: &str = "mcp:tags.find:call";

/// The template the whole scope exists for: a heading named from the nav context, not from the record.
const HEADING_TEMPLATE: &str = "${__nav.parent.label} · ${__nav.label} — energy meter";

// --- helpers ------------------------------------------------------------------------------------

/// A `dashboard` nav item pinning `title_template` (and the `vars` its references bind against).
fn dashboard_item_with_template(
    label: &str,
    dashboard: &str,
    vars: &[(&str, &str)],
    title_template: Option<&str>,
) -> NavItem {
    NavItem {
        kind: "dashboard".into(),
        label: label.into(),
        dashboard: dashboard.into(),
        vars: vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<BTreeMap<_, _>>(),
        title_template: title_template.map(str::to_string),
        ..Default::default()
    }
}

async fn seed_dashboard(store: &Store, owner: &Principal, ws: &str, id: &str, title: &str) {
    dashboard_save(store, owner, ws, id, title, Vec::<Cell>::new(), vec![], 1)
        .await
        .expect("seed dashboard");
}

/// Find a resolved item by label anywhere in the tree.
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
async fn title_template_survives_the_store_read_on_a_plain_host_nav_record() {
    // The single highest-probability bug in the scope. A plain HOST nav item (no extension, no
    // template group) is written through `nav.save` and read back through BOTH doors — `nav.get`
    // (the record) and `nav.resolve` (the rendered payload). A struct-only change passes neither.
    let ws = "ws-navctx-projection";
    let node = Arc::new(lb_host::Node::boot().await.unwrap());
    let store = &node.store;
    let ada = principal("user:ada", ws, &[SAVE, GET, RESOLVE, DASH_SAVE, DASH_GET]);
    seed_dashboard(store, &ada, ws, "meter", "Socomec COUNTIS P44").await;

    nav_save(
        store,
        &ada,
        ws,
        "ops",
        "Operations",
        vec![dashboard_item_with_template(
            "test",
            "dashboard:meter",
            &[("network", "socomec-rtu"), ("device", "test")],
            Some(HEADING_TEMPLATE),
        )],
        10,
    )
    .await
    .unwrap();

    // Door 1 — the record read.
    let got = nav_get(store, &ada, ws, "ops").await.unwrap();
    assert_eq!(
        got.items[0].title_template.as_deref(),
        Some(HEADING_TEMPLATE),
        "the store read must not drop the field (the projection trap)"
    );

    // Door 2 — the resolved payload the client actually renders from.
    nav_pref_set(store, &ada, ws, Some("ops"), None, 11)
        .await
        .unwrap();
    let resolved = nav_resolve(&node, &ada, ws).await.unwrap();
    let item = find(&resolved.items, "test").expect("the item resolved");
    assert_eq!(
        item.title_template.as_deref(),
        Some(HEADING_TEMPLATE),
        "ResolvedItem relays the template verbatim, beside `vars`"
    );
    assert_eq!(
        item.vars.get("device").map(String::as_str),
        Some("test"),
        "the binding still rides through unchanged"
    );

    // And the wire spelling the consumer reads.
    let json = serde_json::to_value(item).unwrap();
    assert_eq!(json["titleTemplate"], serde_json::json!(HEADING_TEMPLATE));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_item_without_a_template_resolves_to_none_and_omits_the_key() {
    // Additive, no migration: every pre-field record and every item that pins nothing reads exactly
    // as it did before this change, and the payload an old client sees is byte-identical.
    let ws = "ws-navctx-absent";
    let node = Arc::new(lb_host::Node::boot().await.unwrap());
    let store = &node.store;
    let ada = principal("user:ada", ws, &[SAVE, GET, RESOLVE, DASH_SAVE, DASH_GET]);
    seed_dashboard(store, &ada, ws, "plain", "Plain Board").await;
    nav_save(
        store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![dashboard_item_with_template(
            "Plain",
            "dashboard:plain",
            &[],
            None,
        )],
        1,
    )
    .await
    .unwrap();
    nav_pref_set(store, &ada, ws, Some("ops"), None, 2)
        .await
        .unwrap();

    let resolved = nav_resolve(&node, &ada, ws).await.unwrap();
    let item = find(&resolved.items, "Plain").expect("resolved");
    assert_eq!(item.title_template, None);
    let json = serde_json::to_value(item).unwrap();
    assert!(json.get("titleTemplate").is_none(), "absent ⇒ omitted");

    // A record written BEFORE the field existed deserializes the same way — no migration.
    let legacy: NavItem =
        serde_json::from_str(r#"{"kind":"dashboard","label":"Old","dashboard":"dashboard:x"}"#)
            .unwrap();
    assert_eq!(legacy.title_template, None);
}

// --- the template-group fan-out -----------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn template_group_fan_out_gives_every_instance_the_title_template() {
    // ONE template board, many bindings — so the override has to reach every generated instance, or
    // the fan-out is exactly the case the scope exists to fix (every page titled the same thing).
    let ws = "ws-navctx-fanout";
    let node = Arc::new(lb_host::Node::boot().await.unwrap());
    let store = &node.store;
    let ada = principal(
        "user:ada",
        ws,
        &[SAVE, RESOLVE, DASH_SAVE, DASH_GET, TAGS_ADD, TAGS_FIND],
    );
    seed_dashboard(store, &ada, ws, "site-overview", "Site Overview").await;
    for (entity, value, at) in [
        ("series:hvac.plant-1.temp", "plant-1", 2u64),
        ("series:hvac.plant-2.temp", "plant-2", 3),
    ] {
        tags_add(
            store,
            &ada,
            ws,
            entity,
            &Tag::new("site", serde_json::json!(value)),
            &Provenance::new(at, ada.sub(), TagSource::Human),
        )
        .await
        .unwrap();
    }

    // The group binds `site`, so `${site}` is bindable on its own template.
    let group = NavItem {
        kind: "template-group".into(),
        label: "Sites".into(),
        dashboard: "dashboard:site-overview".into(),
        var: "site".into(),
        facets: vec![NavFacet {
            key: "site".into(),
            value: None,
        }],
        title_template: Some("${site} — ${__nav.parent.label}".into()),
        ..Default::default()
    };
    nav_save(store, &ada, ws, "ops", "Ops", vec![group], 5)
        .await
        .unwrap();
    nav_pref_set(store, &ada, ws, Some("ops"), None, 6)
        .await
        .unwrap();

    let resolved = nav_resolve(&node, &ada, ws).await.unwrap();
    let grp = resolved.items.iter().find(|i| i.kind == "group").unwrap();
    assert_eq!(grp.items.len(), 2, "one instance per option value");
    assert_eq!(
        grp.title_template.as_deref(),
        Some("${site} — ${__nav.parent.label}"),
        "the group node keeps the authored template"
    );
    for child in &grp.items {
        assert_eq!(
            child.title_template.as_deref(),
            Some("${site} — ${__nav.parent.label}"),
            "every generated instance names itself from its own binding"
        );
        assert!(
            child.vars.contains_key("site"),
            "and carries the binding the template resolves against"
        );
    }
}

// --- the nav-builder write path is validated identically (rule 10) ------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn nav_save_caps_and_validates_the_template_like_the_manifest_path() {
    let ws = "ws-navctx-validate";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[SAVE, GET]);

    // Over the cap → BadInput, nothing persists.
    let over = "x".repeat(NAV_MAX_TITLE_TEMPLATE + 1);
    let err = nav_save(
        &store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![dashboard_item_with_template(
            "T",
            "dashboard:x",
            &[],
            Some(&over),
        )],
        1,
    )
    .await
    .expect_err("over-cap template is refused");
    assert!(matches!(err, NavError::BadInput(_)));

    // An unbindable reference → BadInput NAMING the offender (the same verdict the manifest gives).
    let err = nav_save(
        &store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![dashboard_item_with_template(
            "T",
            "dashboard:x",
            &[("network", "n")],
            Some("${network} / ${device}"),
        )],
        2,
    )
    .await
    .expect_err("unbindable reference is refused");
    let NavError::BadInput(msg) = err else {
        panic!("expected BadInput");
    };
    assert!(msg.contains("device"), "names the offender: {msg}");

    // A bound reference and a built-in both pass — and `__nav.*` IS allowed in `title_template`.
    nav_save(
        &store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![dashboard_item_with_template(
            "T",
            "dashboard:x",
            &[("network", "n")],
            Some("${network} · ${__nav.label} · ${__page.ext}"),
        )],
        3,
    )
    .await
    .expect("bound names + built-ins are accepted");
    assert!(nav_get(&store, &ada, ws, "ops").await.unwrap().items[0]
        .title_template
        .is_some());
}

// --- heading / description are TEMPLATE STRINGS the host stores RAW ------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_templated_heading_round_trips_byte_identical() {
    // §G2/§G5: the host expands nothing. Save a heading and a description that are pure templates and
    // read them back — any server-side interpolation, trimming or escaping shows up here.
    let ws = "ws-navctx-heading";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[DASH_SAVE, DASH_GET]);

    let meta = PageMeta {
        heading: Some(HEADING_TEMPLATE.to_string()),
        description: Some("Generated by ${__page.ext} for ${__nav.path}".to_string()),
        ..Default::default()
    };
    dashboard_save_meta(
        &store,
        &ada,
        ws,
        "meter",
        "Socomec COUNTIS P44",
        meta,
        Vec::<Cell>::new(),
        vec![],
        1,
    )
    .await
    .unwrap();

    let got = dashboard_get(&store, &ada, ws, "meter").await.unwrap();
    assert_eq!(got.heading, HEADING_TEMPLATE, "stored RAW, never resolved");
    assert_eq!(
        got.description,
        "Generated by ${__page.ext} for ${__nav.path}"
    );

    // A heading of ONLY a reference is still just a string to the host.
    dashboard_save_meta(
        &store,
        &ada,
        ws,
        "meter",
        "Socomec COUNTIS P44",
        PageMeta {
            heading: Some("${__page.title}".into()),
            ..Default::default()
        },
        Vec::<Cell>::new(),
        vec![],
        2,
    )
    .await
    .unwrap();
    assert_eq!(
        dashboard_get(&store, &ada, ws, "meter")
            .await
            .unwrap()
            .heading,
        "${__page.title}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn existing_data_with_a_bare_literal_dollar_is_untouched() {
    // The retroactivity guard on the DATA side: declaring these fields templates must not
    // invalidate what is already stored, and the grammar has no escape for a literal `$`.
    let ws = "ws-navctx-legacy";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, &[DASH_SAVE, DASH_GET]);

    dashboard_save_meta(
        &store,
        &ada,
        ws,
        "tariff",
        "Tariff",
        PageMeta {
            heading: Some("Cost $USD per kWh".into()),
            description: Some("Spot price in $/kWh".into()),
            ..Default::default()
        },
        Vec::<Cell>::new(),
        vec![],
        1,
    )
    .await
    .expect("a literal `$` must still save — no validator rejects stored prose");

    let got = dashboard_get(&store, &ada, ws, "tariff").await.unwrap();
    assert_eq!(got.heading, "Cost $USD per kWh");
    assert_eq!(got.description, "Spot price in $/kWh");
}
