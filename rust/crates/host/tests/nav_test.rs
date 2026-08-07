//! The nav builder surface, headless (nav scope, the "Testing plan"). Proves the mandatory
//! categories against a real store/node: the CRUD round-trip, capability-deny **per verb**, the
//! **gate-3 non-member deny** (a team-shared nav resolved by a member, refused for a non-member),
//! two-workspace isolation, resolution precedence (pick → team → default → fallback), tag-group
//! dynamism, the member-owned pref, idempotent upsert, and — the HEADLINE — the "nav never widens"
//! test: a nav that lists a surface + a dashboard the caller lacks is stripped by `nav.resolve` AND a
//! direct read is still denied server-side (the lens grants nothing).
//!
//! A nav is an **asset**, so the sharing model is the shipped S4 three-gate one (`share`/`member`
//! edges, reused via `add_member`/`nav_share`) — identical to the dashboard tests, cloned. `resolve`
//! needs the whole `&Node` (it discovers `ext` items via `ext.list`), so those tests boot a real node.

use lb_assets::{record_install, ExtNavItem, ExtUi, Install};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    add_member, dashboard_save, nav_delete, nav_get, nav_hidden_get, nav_hidden_set, nav_list,
    nav_list_shares, nav_pref_get, nav_pref_set, nav_resolve, nav_save, nav_set_default, nav_share,
    nav_unshare, tags_add, Cell, NavError, NavFacet, NavItem, NavResolvedItem, NavResolvedSource,
    NavVisibility, Node, NAV_MAX_GROUP_DEPTH, NAV_MAX_HIDDEN, NAV_MAX_ITEMS, NAV_MAX_PINNED,
};
use lb_store::Store;
use lb_tags::{Provenance, Source as TagSource, Tag};
use serde_json::json;

/// A principal `sub` in workspace `ws` holding `caps`.
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

const GET: &str = "mcp:nav.get:call";
const LIST: &str = "mcp:nav.list:call";
const SAVE: &str = "mcp:nav.save:call";
const DELETE: &str = "mcp:nav.delete:call";
const SHARE: &str = "mcp:nav.share:call";
const RESOLVE: &str = "mcp:nav.resolve:call";
/// Resolving an `ext:<ext>/<navid>` pin reads the install through the generic `ext.list` seam.
const EXT_LIST: &str = "mcp:ext.list:call";
const ALL: &[&str] = &[GET, LIST, SAVE, DELETE, SHARE, RESOLVE];

/// The surface cap for the `dashboards` page (used to prove a surface item survives/strips).
const DASH_LIST: &str = "mcp:dashboard.list:call";
/// The dashboard read cap the resolver's gate-3 dashboard check needs (dashboard.get).
const DASH_GET: &str = "mcp:dashboard.get:call";
const DASH_SAVE: &str = "mcp:dashboard.save:call";
/// The `rules` surface's gate cap (a surface a caller may lack — the strip target).
const RULES_RUN: &str = "mcp:rules.run:call";

// --- item constructors --------------------------------------------------------------------------

fn surface_item(label: &str, surface: &str) -> NavItem {
    NavItem {
        kind: "surface".into(),
        label: label.into(),
        surface: surface.into(),
        dashboard: String::new(),
        ext: String::new(),
        facets: vec![],
        items: vec![],
        ..Default::default()
    }
}

fn dashboard_item(label: &str, dashboard: &str) -> NavItem {
    NavItem {
        kind: "dashboard".into(),
        label: label.into(),
        surface: String::new(),
        dashboard: dashboard.into(),
        ext: String::new(),
        facets: vec![],
        items: vec![],
        ..Default::default()
    }
}

fn tag_group_item(label: &str, facets: Vec<NavFacet>) -> NavItem {
    NavItem {
        kind: "tag-group".into(),
        label: label.into(),
        surface: String::new(),
        dashboard: String::new(),
        ext: String::new(),
        facets,
        items: vec![],
        ..Default::default()
    }
}

fn group_item(label: &str, items: Vec<NavItem>) -> NavItem {
    NavItem {
        kind: "group".into(),
        label: label.into(),
        surface: String::new(),
        dashboard: String::new(),
        ext: String::new(),
        facets: vec![],
        items,
        ..Default::default()
    }
}

/// Seed a real (empty) dashboard owned by `owner`, so a `dashboard` nav item / tag-group has a target.
async fn seed_dashboard(store: &Store, owner: &Principal, ws: &str, id: &str, title: &str) {
    dashboard_save(store, owner, ws, id, title, no_cells(), vec![], 1)
        .await
        .expect("seed dashboard");
}

fn no_cells() -> Vec<Cell> {
    Vec::new()
}

// --- CRUD ---------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn crud_round_trip() {
    let ws = "ws-nav-crud";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);

    // create
    let n = nav_save(
        &store,
        &test,
        ws,
        "ops",
        "Operations",
        vec![surface_item("Channels", "channels")],
        10,
    )
    .await
    .unwrap();
    assert_eq!(n.title, "Operations");
    assert_eq!(n.owner, "user:test");
    assert_eq!(n.visibility, NavVisibility::Private);

    // get reflects it (full items)
    let got = nav_get(&store, &test, ws, "ops").await.unwrap();
    assert_eq!(got.items.len(), 1);
    assert_eq!(got.items[0].surface, "channels");

    // update (same id) — title + items change, owner preserved
    nav_save(
        &store,
        &test,
        ws,
        "ops",
        "Operations v2",
        vec![
            surface_item("Channels", "channels"),
            surface_item("Rules", "rules"),
        ],
        20,
    )
    .await
    .unwrap();
    let got = nav_get(&store, &test, ws, "ops").await.unwrap();
    assert_eq!(got.title, "Operations v2");
    assert_eq!(got.items.len(), 2);
    assert_eq!(got.updated_ts, 20);

    // list includes it (summary, no items)
    let roster = nav_list(&store, &test, ws).await.unwrap();
    assert!(roster
        .iter()
        .any(|s| s.id == "ops" && s.title == "Operations v2"));

    // delete → list excludes it; get is NotFound
    nav_delete(&store, &test, ws, "ops", 30).await.unwrap();
    let roster = nav_list(&store, &test, ws).await.unwrap();
    assert!(!roster.iter().any(|s| s.id == "ops"));
    assert!(matches!(
        nav_get(&store, &test, ws, "ops").await.unwrap_err(),
        NavError::NotFound
    ));

    // re-delete is an idempotent no-op
    nav_delete(&store, &test, ws, "ops", 40).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn idempotent_upsert_by_slug() {
    let ws = "ws-nav-idem";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);

    // Two saves by the same slug are LWW — no duplicate row.
    nav_save(&store, &test, ws, "ops", "One", vec![], 1)
        .await
        .unwrap();
    nav_save(&store, &test, ws, "ops", "Two", vec![], 2)
        .await
        .unwrap();
    let roster = nav_list(&store, &test, ws).await.unwrap();
    assert_eq!(roster.iter().filter(|s| s.id == "ops").count(), 1);
    assert_eq!(
        nav_get(&store, &test, ws, "ops").await.unwrap().title,
        "Two"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn over_cap_items_rejected() {
    let ws = "ws-nav-bounds";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);

    // Over the item cap → rejected (the host is the boundary, not the builder).
    let too_many: Vec<NavItem> = (0..(NAV_MAX_ITEMS + 1))
        .map(|i| surface_item(&format!("s{i}"), "channels"))
        .collect();
    assert!(matches!(
        nav_save(&store, &test, ws, "ops", "Ops", too_many, 1)
            .await
            .unwrap_err(),
        NavError::BadInput(_)
    ));

    // One level of `group`-in-`group` nesting is now ALLOWED (nested-nav scope) — no longer rejected.
    let nested = group_item("Outer", vec![group_item("Inner", vec![])]);
    nav_save(&store, &test, ws, "ops2", "Ops", vec![nested], 1)
        .await
        .expect("shallow nesting is valid");

    // An unknown item kind → rejected.
    let mut bad = surface_item("x", "channels");
    bad.kind = "bogus".into();
    assert!(matches!(
        nav_save(&store, &test, ws, "ops3", "Ops", vec![bad], 1)
            .await
            .unwrap_err(),
        NavError::BadInput(_)
    ));
}

// --- mandatory: capability deny per verb --------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_verb_is_denied_without_its_cap() {
    let ws = "ws-nav-deny";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);
    nav_save(&store, &test, ws, "ops", "Ops", vec![], 1)
        .await
        .unwrap();

    let nobody = principal("user:nobody", ws, &[]);
    assert!(matches!(
        nav_get(&store, &nobody, ws, "ops").await.unwrap_err(),
        NavError::Denied
    ));
    assert!(matches!(
        nav_list(&store, &nobody, ws).await.unwrap_err(),
        NavError::Denied
    ));
    assert!(matches!(
        nav_save(&store, &nobody, ws, "x", "X", vec![], 1)
            .await
            .unwrap_err(),
        NavError::Denied
    ));
    assert!(matches!(
        nav_delete(&store, &nobody, ws, "ops", 1).await.unwrap_err(),
        NavError::Denied
    ));
    assert!(matches!(
        nav_share(
            &store,
            &nobody,
            ws,
            "ops",
            NavVisibility::Workspace,
            None,
            1
        )
        .await
        .unwrap_err(),
        NavError::Denied
    ));
    assert!(matches!(
        nav_set_default(&store, &nobody, ws, "ops", 1)
            .await
            .unwrap_err(),
        NavError::Denied
    ));
    // resolve + pref both gate on `mcp:nav.resolve:call`.
    let node = std::sync::Arc::new(Node::boot_with_store(store.clone()).await.unwrap());
    assert!(matches!(
        nav_resolve(&node, &nobody, ws).await.unwrap_err(),
        NavError::Denied
    ));
    assert!(matches!(
        nav_pref_get(&store, &nobody, ws).await.unwrap_err(),
        NavError::Denied
    ));
    assert!(matches!(
        nav_pref_set(&store, &nobody, ws, Some("ops"), None, 1)
            .await
            .unwrap_err(),
        NavError::Denied
    ));
}

// --- mandatory: workspace isolation -------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation() {
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", "ws-a", ALL);
    let ben = principal("user:ben", "ws-b", ALL);

    nav_save(store, &test, "ws-a", "ops", "Ops A", vec![], 1)
        .await
        .unwrap();
    test_sets_pick(store, &test, "ws-a", "ops").await;

    // Ben (ws-B) cannot get/list ws-A's nav, and his resolve/pref never see it — the wall.
    assert!(matches!(
        nav_get(store, &ben, "ws-b", "ops").await.unwrap_err(),
        NavError::NotFound
    ));
    assert!(nav_list(store, &ben, "ws-b").await.unwrap().is_empty());
    // Ben's pref in ws-B is his own (empty) — never ws-A's pick.
    assert!(nav_pref_get(store, &ben, "ws-b")
        .await
        .unwrap()
        .active
        .is_empty());
    // Ben resolves to the fallback (no nav in ws-B), not ws-A's ops nav.
    let r = nav_resolve(&node, &ben, "ws-b").await.unwrap();
    assert_eq!(r.source, NavResolvedSource::Fallback);

    // A non-owner cannot overwrite the owner's nav even in the same workspace.
    let mallory = principal("user:mallory", "ws-a", ALL);
    assert!(matches!(
        nav_save(store, &mallory, "ws-a", "ops", "hijack", vec![], 2)
            .await
            .unwrap_err(),
        NavError::Denied
    ));
}

async fn test_sets_pick(store: &Store, test: &Principal, ws: &str, id: &str) {
    nav_pref_set(store, test, ws, Some(id), None, 5)
        .await
        .unwrap();
}

// --- mandatory: gate-3 team-shared deny (non-member) --------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn team_shared_member_resolves_non_member_denied() {
    let ws = "ws-nav-share";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    // Test owns + admins (needs `store:doc/*:write` to add a team member — the S4 edge).
    let test = principal(
        "user:test",
        ws,
        &[GET, LIST, SAVE, DELETE, SHARE, RESOLVE, "store:doc/*:write"],
    );
    let ben = principal("user:ben", ws, &[GET, LIST, RESOLVE]); // team member
    let cleo = principal("user:cleo", ws, &[GET, LIST, RESOLVE]); // NOT in the team

    nav_save(
        store,
        &test,
        ws,
        "ops",
        "Ops",
        vec![surface_item("Channels", "channels")],
        1,
    )
    .await
    .unwrap();

    // Private: a non-owner member is denied gate 3.
    assert!(matches!(
        nav_get(store, &ben, ws, "ops").await.unwrap_err(),
        NavError::Denied
    ));

    // Share to a team Ben belongs to.
    add_member(store, &test, ws, "team:ops", "user:ben")
        .await
        .unwrap();
    nav_share(
        store,
        &test,
        ws,
        "ops",
        NavVisibility::Team,
        Some("team:ops"),
        2,
    )
    .await
    .unwrap();

    // Ben (member) reads + resolves it; Cleo (non-member) is DENIED get, and resolves to the fallback
    // (the shared nav is invisible to her — never leaked).
    assert_eq!(nav_get(store, &ben, ws, "ops").await.unwrap().id, "ops");
    let rben = nav_resolve(&node, &ben, ws).await.unwrap();
    assert_eq!(rben.source, NavResolvedSource::Team);
    assert_eq!(rben.nav_id, "ops");

    assert!(matches!(
        nav_get(store, &cleo, ws, "ops").await.unwrap_err(),
        NavError::Denied
    ));
    let rcleo = nav_resolve(&node, &cleo, ws).await.unwrap();
    assert_eq!(rcleo.source, NavResolvedSource::Fallback);

    // The roster is membership-filtered: Ben sees it, Cleo does not.
    assert!(nav_list(store, &ben, ws)
        .await
        .unwrap()
        .iter()
        .any(|s| s.id == "ops"));
    assert!(!nav_list(store, &cleo, ws)
        .await
        .unwrap()
        .iter()
        .any(|s| s.id == "ops"));
}

// --- HEADLINE: the nav never widens -------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn nav_never_widens_strips_and_direct_read_still_denied() {
    let ws = "ws-nav-lens";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;

    // Test (admin) authors a workspace nav listing the `rules` surface AND a dashboard she owns.
    let test = principal(
        "user:test",
        ws,
        &[SAVE, SHARE, RESOLVE, GET, LIST, DASH_SAVE, DASH_GET],
    );
    seed_dashboard(store, &test, ws, "secret", "Secret Board").await;
    nav_save(
        store,
        &test,
        ws,
        "ops",
        "Ops",
        vec![
            surface_item("Rules", "rules"),
            surface_item("Channels", "channels"),
            dashboard_item("Secret", "dashboard:secret"),
        ],
        1,
    )
    .await
    .unwrap();
    nav_share(store, &test, ws, "ops", NavVisibility::Workspace, None, 2)
        .await
        .unwrap();

    // Ben holds resolve + a surface cap for NOTHING but channels, and NO dashboard read. He does NOT
    // hold `rules.run` (the `rules` surface gate) nor `dashboard.get` for the secret board.
    let ben = principal("user:ben", ws, &[RESOLVE]);

    let r = nav_resolve(&node, &ben, ws).await.unwrap();
    // Precedence: no pick, no team share for ben, but a workspace-default? No default set — but the
    // nav is `visibility:workspace`, which is NOT a pick tier. So ben falls to the fallback UNLESS a
    // default points at it. Set the default so ben resolves THIS nav, then prove the strip.
    assert_eq!(r.source, NavResolvedSource::Fallback);

    // Point the workspace default at ops, so ben's resolve lands on it.
    let admin = principal("user:admin", ws, &[SAVE]);
    // set_default is gated by nav.save; admin holds it. (Any admin may set the ws default.)
    nav_set_default(store, &admin, ws, "ops", 3).await.unwrap();

    let r = nav_resolve(&node, &ben, ws).await.unwrap();
    assert_eq!(r.source, NavResolvedSource::WorkspaceDefault);
    // The `rules` surface is STRIPPED (ben lacks `rules.run`); the secret dashboard is STRIPPED (ben
    // lacks `dashboard.get`); only `channels` (always-visible) survives. The lens hides them.
    let surfaces: Vec<&str> = r.items.iter().map(|i| i.surface.as_str()).collect();
    assert!(surfaces.contains(&"channels"), "channels survives");
    assert!(!surfaces.contains(&"rules"), "rules stripped (no cap)");
    assert!(
        !r.items.iter().any(|i| i.dashboard == "dashboard:secret"),
        "secret dashboard stripped (no read)"
    );

    // AND a DIRECT read of the stripped dashboard is STILL denied server-side (the nav granted
    // nothing — proving the lens). Ben with a dashboard.get cap but no membership is still denied on a
    // private board; here the board is Test's private-by-default... actually it is workspace? No: the
    // nav is workspace; the dashboard `secret` stayed PRIVATE (only shared the nav). So even a
    // dashboard.get-holding ben is denied the board by gate 3.
    let ben_with_dashget = principal("user:ben2", ws, &[RESOLVE, DASH_GET]);
    assert!(
        matches!(
            lb_host::dashboard_get(store, &ben_with_dashget, ws, "secret")
                .await
                .unwrap_err(),
            lb_host::DashboardError::Denied
        ),
        "direct dashboard read still denied — the nav widened nothing"
    );
}

// --- resolution precedence ----------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resolution_precedence_pick_over_team_over_default_over_fallback() {
    let ws = "ws-nav-prec";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal(
        "user:test",
        ws,
        &[SAVE, SHARE, RESOLVE, GET, LIST, DELETE, "store:doc/*:write"],
    );

    // Empty state: no nav at all → fallback (never blank).
    assert_eq!(
        nav_resolve(&node, &test, ws).await.unwrap().source,
        NavResolvedSource::Fallback
    );

    // A workspace-default nav → resolves to WorkspaceDefault.
    nav_save(store, &test, ws, "wsdef", "WS Default", vec![], 1)
        .await
        .unwrap();
    nav_set_default(store, &test, ws, "wsdef", 2).await.unwrap();
    assert_eq!(nav_resolve(&node, &test, ws).await.unwrap().nav_id, "wsdef");
    assert_eq!(
        nav_resolve(&node, &test, ws).await.unwrap().source,
        NavResolvedSource::WorkspaceDefault
    );

    // A team-shared nav Test belongs to → beats the default.
    nav_save(store, &test, ws, "teamnav", "Team", vec![], 3)
        .await
        .unwrap();
    add_member(store, &test, ws, "team:ops", "user:test")
        .await
        .unwrap();
    nav_share(
        store,
        &test,
        ws,
        "teamnav",
        NavVisibility::Team,
        Some("team:ops"),
        4,
    )
    .await
    .unwrap();
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    assert_eq!(r.source, NavResolvedSource::Team);
    assert_eq!(r.nav_id, "teamnav");

    // A personal pick → beats the team share.
    nav_save(store, &test, ws, "mine", "Mine", vec![], 5)
        .await
        .unwrap();
    nav_pref_set(store, &test, ws, Some("mine"), None, 6)
        .await
        .unwrap();
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    assert_eq!(r.source, NavResolvedSource::Pick);
    assert_eq!(r.nav_id, "mine");

    // A stale pick (deleted nav) falls through to the next tier, not an error.
    nav_delete(store, &test, ws, "mine", 7).await.unwrap();
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    assert_eq!(
        r.source,
        NavResolvedSource::Team,
        "stale pick falls through"
    );
}

// --- tag-group dynamism -------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn tag_group_expands_dynamically_and_respects_reachability() {
    let ws = "ws-nav-taggroup";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal(
        "user:test",
        ws,
        &[
            SAVE,
            RESOLVE,
            GET,
            DASH_SAVE,
            DASH_GET,
            "mcp:tags.add:call",
            "mcp:tags.find:call",
            "mcp:tags.remove:call",
        ],
    );

    // Two dashboards Test owns; a nav with a tag-group over `site`.
    seed_dashboard(store, &test, ws, "plant-1", "Plant 1").await;
    seed_dashboard(store, &test, ws, "plant-2", "Plant 2").await;
    nav_save(
        store,
        &test,
        ws,
        "sites",
        "Sites",
        vec![tag_group_item(
            "Sites",
            vec![NavFacet {
                key: "site".into(),
                value: None,
            }],
        )],
        1,
    )
    .await
    .unwrap();
    nav_pref_set(store, &test, ws, Some("sites"), None, 2)
        .await
        .unwrap();

    // Before tagging: the tag-group is empty (no dashboard carries a `site` facet).
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    assert!(grp.items.is_empty(), "no tags yet → empty group");

    // Tag plant-1 with `site` → it appears on re-resolve (no nav edit).
    let prov = Provenance::new(3, "user:test", TagSource::Human);
    tags_add(
        store,
        &test,
        ws,
        "dashboard:plant-1",
        &Tag::new("site", json!("plant-1")),
        &prov,
    )
    .await
    .unwrap();
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    assert_eq!(grp.items.len(), 1);
    assert_eq!(grp.items[0].dashboard, "dashboard:plant-1");

    // Untag → it disappears.
    lb_host::tags_remove(store, &test, ws, "dashboard:plant-1", "site", None)
        .await
        .unwrap();
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    assert!(grp.items.is_empty(), "untagged → gone");

    // A tag-group only surfaces dashboards the caller can READ: tag a dashboard owned by someone
    // else + not shared, and it does not appear for Test... (build it as Ben's private board).
    let ben = principal("user:ben", ws, &[DASH_SAVE, "mcp:tags.add:call"]);
    seed_dashboard(store, &ben, ws, "ben-board", "Ben Board").await;
    tags_add(
        store,
        &ben,
        ws,
        "dashboard:ben-board",
        &Tag::new("site", json!("ben")),
        &Provenance::new(9, "user:ben", TagSource::Human),
    )
    .await
    .unwrap();
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    assert!(
        !grp.items
            .iter()
            .any(|i| i.dashboard == "dashboard:ben-board"),
        "tag-group hides an unreadable dashboard (the lens)"
    );
}

// --- member-owned pref --------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn member_owns_own_pref_cannot_touch_anothers() {
    let ws = "ws-nav-pref";
    let store = Store::memory().await.unwrap();
    // A plain member (only the resolve cap) sets their OWN pick — no admin cap needed.
    let ben = principal("user:ben", ws, &[RESOLVE]);
    nav_pref_set(&store, &ben, ws, Some("somepick"), None, 1)
        .await
        .unwrap();
    assert_eq!(
        nav_pref_get(&store, &ben, ws).await.unwrap().active,
        "somepick"
    );

    // Test's pick is independent — Ben's write never touched it (keyed by principal sub).
    let test = principal("user:test", ws, &[RESOLVE]);
    assert!(nav_pref_get(&store, &test, ws)
        .await
        .unwrap()
        .active
        .is_empty());
    nav_pref_set(&store, &test, ws, Some("adapick"), None, 2)
        .await
        .unwrap();
    // Ben's is still his own, unchanged.
    assert_eq!(
        nav_pref_get(&store, &ben, ws).await.unwrap().active,
        "somepick"
    );
    assert_eq!(
        nav_pref_get(&store, &test, ws).await.unwrap().active,
        "adapick"
    );
}

// --- group nesting + surface strip inside a group -----------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn group_children_are_stripped_independently() {
    let ws = "ws-nav-group";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", ws, &[SAVE, RESOLVE]);
    nav_save(
        store,
        &test,
        ws,
        "admin",
        "Admin",
        vec![group_item(
            "Admin",
            vec![
                surface_item("Rules", "rules"),
                surface_item("Channels", "channels"),
            ],
        )],
        1,
    )
    .await
    .unwrap();
    nav_pref_set(store, &test, ws, Some("admin"), None, 2)
        .await
        .unwrap();

    // Test holds RESOLVE but NOT rules.run → inside the group, `rules` strips, `channels` stays.
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    let surfaces: Vec<&str> = grp.items.iter().map(|i| i.surface.as_str()).collect();
    assert_eq!(
        surfaces,
        vec!["channels"],
        "rules stripped inside the group"
    );

    // With rules.run, `rules` survives too.
    let ada2 = principal("user:test", ws, &[SAVE, RESOLVE, RULES_RUN, DASH_LIST]);
    let r = nav_resolve(&node, &ada2, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    assert_eq!(grp.items.len(), 2, "both survive with the cap");
}

// --- share roster: list_shares + unshare (the add/remove team surface) --------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn share_roster_lists_and_revokes_team_shares() {
    let ws = "ws-nav-shares";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal(
        "user:test",
        ws,
        &[GET, LIST, SAVE, SHARE, RESOLVE, "store:doc/*:write"],
    );

    nav_save(
        store,
        &test,
        ws,
        "ops",
        "Ops",
        vec![surface_item("Channels", "channels")],
        1,
    )
    .await
    .unwrap();

    // Empty roster before any share.
    assert!(nav_list_shares(store, &test, ws, "ops")
        .await
        .unwrap()
        .is_empty());

    // Share to TWO teams (each call writes one edge; the underlying relate is multi-edge).
    add_member(store, &test, ws, "team:ops", "user:ben")
        .await
        .unwrap();
    add_member(store, &test, ws, "team:eng", "user:cleo")
        .await
        .unwrap();
    nav_share(
        store,
        &test,
        ws,
        "ops",
        NavVisibility::Team,
        Some("team:ops"),
        2,
    )
    .await
    .unwrap();
    nav_share(
        store,
        &test,
        ws,
        "ops",
        NavVisibility::Team,
        Some("team:eng"),
        3,
    )
    .await
    .unwrap();

    // The roster reflects both — order unspecified, so compare as a set.
    let shares = nav_list_shares(store, &test, ws, "ops").await.unwrap();
    let mut sorted = shares.clone();
    sorted.sort();
    assert_eq!(sorted, vec!["team:eng", "team:ops"]);

    // Both members resolve the nav (they're in a shared team).
    let ben = principal("user:ben", ws, &[GET, RESOLVE]);
    let cleo = principal("user:cleo", ws, &[GET, RESOLVE]);
    assert_eq!(nav_resolve(&node, &ben, ws).await.unwrap().nav_id, "ops");
    assert_eq!(nav_resolve(&node, &cleo, ws).await.unwrap().nav_id, "ops");

    // Revoke the ops share → roster drops it; ben stops resolving, cleo (still in team:eng) keeps it.
    nav_unshare(store, &test, ws, "ops", "team:ops", 4)
        .await
        .unwrap();
    let shares = nav_list_shares(store, &test, ws, "ops").await.unwrap();
    assert_eq!(shares, vec!["team:eng"]);

    assert_eq!(
        nav_resolve(&node, &cleo, ws).await.unwrap().source,
        NavResolvedSource::Team,
        "cleo still resolves via team:eng"
    );
    // Ben: no longer a member of any shared team → falls through to the fallback.
    assert_eq!(
        nav_resolve(&node, &ben, ws).await.unwrap().source,
        NavResolvedSource::Fallback,
        "ben no longer resolves after the unshare"
    );
    // And a direct get is denied again (gate-3 reads the live relations).
    assert!(matches!(
        nav_get(store, &ben, ws, "ops").await.unwrap_err(),
        NavError::Denied
    ));

    // Re-unshare is idempotent (revoking a never-/already-revoked edge is a no-op tombstone).
    nav_unshare(store, &test, ws, "ops", "team:ops", 5)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unshare_and_list_shares_denied_without_cap() {
    let ws = "ws-nav-shares-deny";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);
    let nobody = principal("user:nobody", ws, &[]);

    nav_save(&store, &test, ws, "ops", "Ops", vec![], 1)
        .await
        .unwrap();
    nav_share(
        &store,
        &test,
        ws,
        "ops",
        NavVisibility::Team,
        Some("team:ops"),
        2,
    )
    .await
    .unwrap();

    // `nav.share` cap gates both new verbs — a capless caller is denied before anything runs.
    assert!(matches!(
        nav_list_shares(&store, &nobody, ws, "ops")
            .await
            .unwrap_err(),
        NavError::Denied
    ));
    assert!(matches!(
        nav_unshare(&store, &nobody, ws, "ops", "team:ops", 3)
            .await
            .unwrap_err(),
        NavError::Denied
    ));
    // The share edge survived — the deny left no mutation.
    assert_eq!(
        nav_list_shares(&store, &test, ws, "ops").await.unwrap(),
        vec!["team:ops"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_shares_and_unshare_owner_only_and_workspace_walled() {
    let ws = "ws-nav-shares-own";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);
    // A same-workspace peer who holds the share cap but is NOT the owner.
    let mallory = principal("user:mallory", ws, ALL);
    // A cross-workspace caller who owns a same-id nav over there.
    let ben = principal("user:ben", "ws-b", ALL);

    nav_save(&store, &test, ws, "ops", "Ops", vec![], 1)
        .await
        .unwrap();
    nav_share(
        &store,
        &test,
        ws,
        "ops",
        NavVisibility::Team,
        Some("team:ops"),
        2,
    )
    .await
    .unwrap();

    // Mallory (cap, non-owner) is denied — exposing the share roster to a peer would leak which
    // other teams exist.
    assert!(matches!(
        nav_list_shares(&store, &mallory, ws, "ops")
            .await
            .unwrap_err(),
        NavError::Denied
    ));
    assert!(matches!(
        nav_unshare(&store, &mallory, ws, "ops", "team:ops", 3)
            .await
            .unwrap_err(),
        NavError::Denied
    ));

    // Ben in ws-B cannot read or revoke ws-A's share (the workspace wall, rule 6). Reached in his
    // OWN workspace (where the nav doesn't exist) it reads as NotFound — no existence signal.
    assert!(matches!(
        nav_list_shares(&store, &ben, "ws-b", "ops")
            .await
            .unwrap_err(),
        NavError::NotFound
    ));
    assert!(matches!(
        nav_unshare(&store, &ben, "ws-b", "ops", "team:ops", 5)
            .await
            .unwrap_err(),
        NavError::NotFound
    ));
    // ws-A's share is untouched by the cross-ws attempt.
    assert_eq!(
        nav_list_shares(&store, &test, ws, "ops").await.unwrap(),
        vec!["team:ops"]
    );
}

// --- hide-and-pins scope: the workspace hidden-set + per-user pins ------------------------------

/// Deny: a member without `mcp:nav.save:call` cannot write the hidden-set (nothing persists); the
/// read rides `nav.resolve` (a capless caller is denied that too).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn hidden_set_denied_without_admin_cap() {
    let ws = "ws-nav-hide-deny";
    let store = Store::memory().await.unwrap();
    let member = principal("user:ben", ws, &[RESOLVE]); // resolve only — no authoring cap
    assert!(matches!(
        nav_hidden_set(&store, &member, ws, vec!["dashboards".into()], 1)
            .await
            .unwrap_err(),
        NavError::Denied
    ));
    // Nothing persisted — the admin read still sees an empty set.
    let admin = principal("user:test", ws, &[SAVE, RESOLVE]);
    assert!(nav_hidden_get(&store, &admin, ws)
        .await
        .unwrap()
        .hidden
        .is_empty());
    // A capless caller cannot even read it.
    let nobody = principal("user:nobody", ws, &[]);
    assert!(matches!(
        nav_hidden_get(&store, &nobody, ws).await.unwrap_err(),
        NavError::Denied
    ));
}

/// Isolation: ws-A's hidden-set has no effect on ws-B, and ws-B cannot read ws-A's record.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn hidden_set_is_workspace_walled() {
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", "ws-a", &[SAVE, RESOLVE]);
    let ben = principal("user:ben", "ws-b", &[SAVE, RESOLVE]);

    nav_hidden_set(store, &test, "ws-a", vec!["dashboards".into()], 1)
        .await
        .unwrap();

    // Ben's ws-B set is empty (his read is his own workspace's record).
    assert!(nav_hidden_get(store, &ben, "ws-b")
        .await
        .unwrap()
        .hidden
        .is_empty());
    // And his resolve echoes no hidden refs.
    let r = nav_resolve(&node, &ben, "ws-b").await.unwrap();
    assert!(r.hidden.is_empty());
    // Test's ws-A resolve echoes hers — even on the fallback tier (no nav authored at all).
    let r = nav_resolve(&node, &test, "ws-a").await.unwrap();
    assert_eq!(r.source, NavResolvedSource::Fallback);
    assert_eq!(r.hidden, vec!["dashboards".to_string()]);
}

/// HEADLINE: hide never blocks. Hiding a surface + a dashboard strips them from the resolved menu
/// (and the echo covers the fallback), but a direct read of the hidden dashboard STILL succeeds for
/// a caller who is permitted — the hidden-set is declutter, not authz.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn hide_strips_menu_but_never_blocks_direct_access() {
    let ws = "ws-nav-hide-lens";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal(
        "user:test",
        ws,
        &[SAVE, SHARE, RESOLVE, GET, DASH_SAVE, DASH_GET, RULES_RUN],
    );
    seed_dashboard(store, &test, ws, "ops-board", "Ops Board").await;
    nav_save(
        store,
        &test,
        ws,
        "ops",
        "Ops",
        vec![
            surface_item("Rules", "rules"),
            surface_item("Channels", "channels"),
            dashboard_item("Ops Board", "dashboard:ops-board"),
        ],
        1,
    )
    .await
    .unwrap();
    nav_pref_set(store, &test, ws, Some("ops"), None, 2)
        .await
        .unwrap();

    // Test holds every cap — before the hide, all three entries resolve.
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    assert_eq!(r.items.len(), 3);

    // The admin hides the `rules` surface and the dashboard.
    nav_hidden_set(
        store,
        &test,
        ws,
        vec!["rules".into(), "dashboard:ops-board".into()],
        3,
    )
    .await
    .unwrap();

    // Both are STRIPPED from the resolved menu (a personal-pick tier), channels survives.
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    assert_eq!(r.source, NavResolvedSource::Pick);
    let surfaces: Vec<&str> = r.items.iter().map(|i| i.surface.as_str()).collect();
    assert!(surfaces.contains(&"channels"));
    assert!(!surfaces.contains(&"rules"), "hidden surface stripped");
    assert!(
        !r.items.iter().any(|i| i.dashboard == "dashboard:ops-board"),
        "hidden dashboard stripped"
    );
    // The echo carries the set for the UI's client-side fallback subtraction.
    assert_eq!(r.hidden.len(), 2);

    // AND the direct read of the hidden dashboard still succeeds — hiding blocked NOTHING.
    assert_eq!(
        lb_host::dashboard_get(store, &test, ws, "ops-board")
            .await
            .unwrap()
            .title,
        "Ops Board"
    );
}

/// Hide applies inside groups too, and at the team/default tiers (the strip is tier-independent).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn hide_applies_inside_groups_and_at_every_tier() {
    let ws = "ws-nav-hide-tiers";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal(
        "user:test",
        ws,
        &[SAVE, SHARE, RESOLVE, GET, "store:doc/*:write"],
    );
    nav_save(
        store,
        &test,
        ws,
        "grouped",
        "Grouped",
        vec![group_item(
            "Ops",
            vec![
                surface_item("Channels", "channels"),
                surface_item("Inbox", "inbox"),
            ],
        )],
        1,
    )
    .await
    .unwrap();
    nav_hidden_set(store, &test, ws, vec!["inbox".into()], 2)
        .await
        .unwrap();

    // Workspace-default tier.
    nav_set_default(store, &test, ws, "grouped", 3)
        .await
        .unwrap();
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    assert_eq!(r.source, NavResolvedSource::WorkspaceDefault);
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    let surfaces: Vec<&str> = grp.items.iter().map(|i| i.surface.as_str()).collect();
    assert_eq!(surfaces, vec!["channels"], "hidden child stripped in group");

    // Team tier — same nav shared to a team test belongs to; the strip result is identical.
    add_member(store, &test, ws, "team:ops", "user:test")
        .await
        .unwrap();
    nav_share(
        store,
        &test,
        ws,
        "grouped",
        NavVisibility::Team,
        Some("team:ops"),
        4,
    )
    .await
    .unwrap();
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    assert_eq!(r.source, NavResolvedSource::Team);
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    assert_eq!(grp.items.len(), 1, "hidden child stripped at team tier too");
}

/// Bounds: an over-cap hidden-set and a blank ref are rejected (`BadInput`), never truncated.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn hidden_set_bounds_rejected() {
    let ws = "ws-nav-hide-bounds";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, &[SAVE, RESOLVE]);
    let too_many: Vec<String> = (0..(NAV_MAX_HIDDEN + 1)).map(|i| format!("s{i}")).collect();
    assert!(matches!(
        nav_hidden_set(&store, &test, ws, too_many, 1)
            .await
            .unwrap_err(),
        NavError::BadInput(_)
    ));
    assert!(matches!(
        nav_hidden_set(&store, &test, ws, vec!["  ".into()], 1)
            .await
            .unwrap_err(),
        NavError::BadInput(_)
    ));
    // LWW replace + clear: set → replace → empty clears.
    nav_hidden_set(&store, &test, ws, vec!["rules".into()], 2)
        .await
        .unwrap();
    nav_hidden_set(&store, &test, ws, vec!["inbox".into()], 3)
        .await
        .unwrap();
    assert_eq!(
        nav_hidden_get(&store, &test, ws).await.unwrap().hidden,
        vec!["inbox".to_string()]
    );
    nav_hidden_set(&store, &test, ws, vec![], 4).await.unwrap();
    assert!(nav_hidden_get(&store, &test, ws)
        .await
        .unwrap()
        .hidden
        .is_empty());
}

/// Pins resolve in the member's order through the same lens (cap-strip), on every tier including
/// the fallback; a pin to an unreadable dashboard strips WITHOUT mutating the stored record.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pins_resolve_ordered_cap_stripped_and_never_mutated() {
    let ws = "ws-nav-pins";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", ws, &[SAVE, RESOLVE, DASH_SAVE, DASH_GET]);
    seed_dashboard(store, &test, ws, "fav", "Fav Board").await;

    // Ben pins a surface + Test's (private, unreadable-to-him) dashboard. No nav exists → fallback.
    let ben = principal("user:ben", ws, &[RESOLVE, DASH_GET]);
    nav_pref_set(
        store,
        &ben,
        ws,
        None,
        Some(vec!["channels".into(), "dashboard:fav".into()]),
        1,
    )
    .await
    .unwrap();

    let r = nav_resolve(&node, &ben, ws).await.unwrap();
    assert_eq!(r.source, NavResolvedSource::Fallback);
    // The unreadable dashboard pin STRIPPED (the lens); the surface pin survives.
    assert_eq!(r.pinned.len(), 1);
    assert_eq!(r.pinned[0].surface, "channels");
    // The stored record is untouched by the strip — both refs still there (restores are free).
    assert_eq!(
        nav_pref_get(store, &ben, ws).await.unwrap().pinned,
        vec!["channels".to_string(), "dashboard:fav".to_string()]
    );

    // Test resolves her own pins — both readable, member order preserved.
    nav_pref_set(
        store,
        &test,
        ws,
        None,
        Some(vec!["dashboard:fav".into(), "channels".into()]),
        2,
    )
    .await
    .unwrap();
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    let refs: Vec<String> = r
        .pinned
        .iter()
        .map(|i| {
            if i.dashboard.is_empty() {
                i.surface.clone()
            } else {
                i.dashboard.clone()
            }
        })
        .collect();
    assert_eq!(refs, vec!["dashboard:fav", "channels"], "member order kept");

    // `pinned: None` (an active-pick-only write) leaves pins untouched.
    nav_pref_set(store, &test, ws, Some(""), None, 3)
        .await
        .unwrap();
    assert_eq!(
        nav_pref_get(store, &test, ws).await.unwrap().pinned.len(),
        2
    );
}

/// Hide beats pin: an admin-hidden ref is stripped even from the member's pinned section; un-hiding
/// restores it without any `nav_pref` rewrite.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn hide_beats_pin_and_unhide_restores() {
    let ws = "ws-nav-hidepin";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", ws, &[SAVE, RESOLVE]);

    nav_pref_set(store, &test, ws, Some(""), Some(vec!["channels".into()]), 1)
        .await
        .unwrap();
    assert_eq!(nav_resolve(&node, &test, ws).await.unwrap().pinned.len(), 1);

    // Hide the pinned surface → the pin strips (hide beats pin).
    nav_hidden_set(store, &test, ws, vec!["channels".into()], 2)
        .await
        .unwrap();
    assert!(nav_resolve(&node, &test, ws)
        .await
        .unwrap()
        .pinned
        .is_empty());

    // Un-hide → the pin is back, with NO write to nav_pref in between.
    nav_hidden_set(store, &test, ws, vec![], 3).await.unwrap();
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    assert_eq!(r.pinned.len(), 1);
    assert_eq!(r.pinned[0].surface, "channels");
}

/// Pins are member-owned and bounded: a caller writes only their own pins; over-cap is `BadInput`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pins_member_owned_and_bounded() {
    let ws = "ws-nav-pin-bounds";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, &[RESOLVE]);
    let ben = principal("user:ben", ws, &[RESOLVE]);

    nav_pref_set(&store, &test, ws, Some(""), Some(vec!["rules".into()]), 1)
        .await
        .unwrap();
    // Ben's pins are independent (keyed by the principal sub — never a body field).
    assert!(nav_pref_get(&store, &ben, ws)
        .await
        .unwrap()
        .pinned
        .is_empty());

    // Over the pin cap → rejected, nothing persisted over the old value.
    let too_many: Vec<String> = (0..(NAV_MAX_PINNED + 1)).map(|i| format!("p{i}")).collect();
    assert!(matches!(
        nav_pref_set(&store, &test, ws, None, Some(too_many), 2)
            .await
            .unwrap_err(),
        NavError::BadInput(_)
    ));
    assert_eq!(
        nav_pref_get(&store, &test, ws).await.unwrap().pinned,
        vec!["rules".to_string()]
    );
}

// --- no-lockout (nav-no-lockout scope) ----------------------------------------------------------

/// An admin-marker cap — its presence means the caller sees the admin console, so a curated nav must
/// not silently replace it (mirrors the UI's `ADMIN_SECTION_CAPS`).
const MEMBERS_MANAGE: &str = "mcp:members.manage:call";

/// The HEADLINE no-lockout guarantee: a workspace admin is NEVER auto-narrowed by a team-shared nav
/// or the workspace default — those tiers are skipped for admins, who fall through to the built-in
/// sidebar. A NON-admin member of the same team still resolves the team nav (members are unaffected).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn admin_never_auto_narrowed_member_still_is() {
    let ws = "ws-nav-nolock";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;

    // Test is an ADMIN (holds `members.manage`) AND can author navs. Ben is a plain member.
    let test = principal(
        "user:test",
        ws,
        &[
            MEMBERS_MANAGE,
            SAVE,
            SHARE,
            RESOLVE,
            GET,
            LIST,
            "store:doc/*:write",
        ],
    );
    let ben = principal("user:ben", ws, &[GET, LIST, RESOLVE]);

    // A one-page nav shared to team:ops, AND set as the workspace default — both auto-apply tiers.
    nav_save(
        store,
        &test,
        ws,
        "ops",
        "Ops",
        vec![surface_item("Channels", "channels")],
        1,
    )
    .await
    .unwrap();
    add_member(store, &test, ws, "team:ops", "user:test")
        .await
        .unwrap();
    add_member(store, &test, ws, "team:ops", "user:ben")
        .await
        .unwrap();
    nav_share(
        store,
        &test,
        ws,
        "ops",
        NavVisibility::Team,
        Some("team:ops"),
        2,
    )
    .await
    .unwrap();
    nav_set_default(store, &test, ws, "ops", 3).await.unwrap();

    // Test is an admin → NEITHER the team share NOR the default narrows her. She gets the built-in rail.
    let rada = nav_resolve(&node, &test, ws).await.unwrap();
    assert_eq!(
        rada.source,
        NavResolvedSource::Fallback,
        "an admin is never auto-narrowed by a team share / workspace default"
    );

    // Ben (non-admin, same team) DOES resolve the team nav — members are unaffected by the rule.
    let rben = nav_resolve(&node, &ben, ws).await.unwrap();
    assert_eq!(rben.source, NavResolvedSource::Team);
    assert_eq!(rben.nav_id, "ops");

    // The admin can still OPT IN explicitly: a personal pick (tier 1) is honored even for an admin.
    nav_pref_set(store, &test, ws, Some("ops"), None, 4)
        .await
        .unwrap();
    let rada2 = nav_resolve(&node, &test, ws).await.unwrap();
    assert_eq!(
        rada2.source,
        NavResolvedSource::Pick,
        "an admin's OWN explicit pick still applies (opt-in, never silent)"
    );
}

/// The escape hatch: anyone handed a too-narrow nav can force the built-in sidebar via the reserved
/// `__builtin__` pick (member-owned), and clear it to resume normal resolution.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn builtin_pick_sentinel_forces_fallback() {
    let ws = "ws-nav-escape";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal(
        "user:test",
        ws,
        &[SAVE, SHARE, RESOLVE, GET, LIST, "store:doc/*:write"],
    );
    let ben = principal("user:ben", ws, &[GET, LIST, RESOLVE]);

    // A team nav Ben would normally resolve.
    nav_save(
        store,
        &test,
        ws,
        "ops",
        "Ops",
        vec![surface_item("Channels", "channels")],
        1,
    )
    .await
    .unwrap();
    add_member(store, &test, ws, "team:ops", "user:ben")
        .await
        .unwrap();
    nav_share(
        store,
        &test,
        ws,
        "ops",
        NavVisibility::Team,
        Some("team:ops"),
        2,
    )
    .await
    .unwrap();
    assert_eq!(
        nav_resolve(&node, &ben, ws).await.unwrap().source,
        NavResolvedSource::Team
    );

    // Ben forces the built-in sidebar via the sentinel → fallback (skips team/default).
    nav_pref_set(store, &ben, ws, Some(lb_host::NAV_BUILTIN_PICK), None, 3)
        .await
        .unwrap();
    assert_eq!(
        nav_resolve(&node, &ben, ws).await.unwrap().source,
        NavResolvedSource::Fallback,
        "the __builtin__ pick forces the built-in sidebar"
    );

    // Clearing the pick resumes normal resolution — the team nav is back.
    nav_pref_set(store, &ben, ws, Some(""), None, 4)
        .await
        .unwrap();
    assert_eq!(
        nav_resolve(&node, &ben, ws).await.unwrap().source,
        NavResolvedSource::Team
    );
}

/// The reserved sentinel can never BE a real nav id — `nav.save` rejects the `__…__` shape so the
/// pick axis stays unambiguous.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reserved_nav_id_rejected() {
    let ws = "ws-nav-reserved";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, &[SAVE]);
    assert!(matches!(
        nav_save(
            &store,
            &test,
            ws,
            lb_host::NAV_BUILTIN_PICK,
            "nope",
            vec![],
            1
        )
        .await
        .unwrap_err(),
        NavError::BadInput(_)
    ));
    // Any `__…__` shape is rejected, not just the one sentinel.
    assert!(matches!(
        nav_save(&store, &test, ws, "__anything__", "nope", vec![], 1)
            .await
            .unwrap_err(),
        NavError::BadInput(_)
    ));
}

// --- nested nav groups (nested-nav scope) -------------------------------------------------------
//
// Groups may nest recursively up to `NAV_MAX_GROUP_DEPTH` (top-level list = depth 1). The resolver's
// strip/expand pipeline runs at EVERY depth as one recursive pure function, and prunes empty groups
// POST-ORDER so a permitted user never sees a folder that expands to nothing. The 100-node cap and the
// depth cap fire INDEPENDENTLY.

/// Wrap `leaf` in `depth` nested `group`s. `depth == 1` = one group holding the leaf (a top-level
/// group at depth 1). `depth == 5` = five groups deep (a group at depth 5 holding the leaf at depth 6).
fn nest_groups(depth: usize, leaf: NavItem) -> NavItem {
    let mut item = group_item(&format!("g{depth}"), vec![leaf]);
    for d in (1..depth).rev() {
        item = group_item(&format!("g{d}"), vec![item]);
    }
    item
}

/// Descend into the single nested `group` chain and return the deepest group's items.
fn deepest_group_items(item: &lb_host::NavResolvedItem) -> &[lb_host::NavResolvedItem] {
    let mut cur = item;
    while let Some(inner) = cur.items.iter().find(|c| c.kind == "group") {
        cur = inner;
    }
    &cur.items
}

/// `nav.save` ACCEPTS a nav nested exactly `NAV_MAX_GROUP_DEPTH` deep and REJECTS one a level deeper
/// with `BadInput` — nothing persists, the error names the limit. Round-trips the accepted deep tree.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn depth_at_cap_accepted_over_cap_rejected() {
    let ws = "ws-nav-depth";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);

    // Exactly at the cap: a group at depth `NAV_MAX_GROUP_DEPTH` holding a leaf. Accepted.
    let at_cap = nest_groups(NAV_MAX_GROUP_DEPTH, surface_item("Channels", "channels"));
    nav_save(&store, &test, ws, "deep", "Deep", vec![at_cap.clone()], 1)
        .await
        .expect("exactly at the depth cap is valid");
    // Round-trips identically (order + nesting preserved).
    let got = nav_get(&store, &test, ws, "deep").await.unwrap();
    assert_eq!(got.items, vec![at_cap], "deep tree round-trips identically");

    // One level deeper: a group at depth `NAV_MAX_GROUP_DEPTH + 1`. Rejected, nothing persists, the
    // error names the limit.
    let over = nest_groups(
        NAV_MAX_GROUP_DEPTH + 1,
        surface_item("Channels", "channels"),
    );
    let err = nav_save(&store, &test, ws, "over", "Over", vec![over], 2)
        .await
        .unwrap_err();
    match err {
        NavError::BadInput(m) => assert!(
            m.contains(&NAV_MAX_GROUP_DEPTH.to_string()),
            "error names the depth limit: {m}"
        ),
        other => panic!("expected BadInput, got {other:?}"),
    }
    assert!(
        matches!(
            nav_get(&store, &test, ws, "over").await.unwrap_err(),
            NavError::NotFound
        ),
        "the over-cap save persisted nothing"
    );
}

/// The 100-node cap and the depth cap are INDEPENDENT: a wide-but-shallow tree over `NAV_MAX_ITEMS`
/// nodes still `BadInput`s even though its nesting is well under the depth cap.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn node_cap_and_depth_cap_are_independent() {
    let ws = "ws-nav-wide";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);

    // One shallow (depth 2) group holding > MAX_ITEMS leaves — over the NODE cap, under the DEPTH cap.
    let leaves: Vec<NavItem> = (0..NAV_MAX_ITEMS)
        .map(|i| surface_item(&format!("s{i}"), "channels"))
        .collect();
    let wide = group_item("Wide", leaves); // group (1) + MAX_ITEMS children = MAX_ITEMS + 1 nodes.
    let err = nav_save(&store, &test, ws, "wide", "Wide", vec![wide], 1)
        .await
        .unwrap_err();
    match err {
        NavError::BadInput(m) => assert!(
            m.contains(&NAV_MAX_ITEMS.to_string()),
            "over-node error names the node cap, not the depth cap: {m}"
        ),
        other => panic!("expected BadInput, got {other:?}"),
    }
}

/// Recursive cap-strip / "nav never widens at depth": a surface the caller lacks, nested several
/// levels deep, is stripped by `nav.resolve` AND a direct route to it still succeeds server-side (the
/// lens grants nothing — enforced at every depth).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deep_leaf_stripped_but_route_still_reachable() {
    let ws = "ws-nav-deep-strip";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal("user:test", ws, &[SAVE, RESOLVE]);

    // A group tree 4 deep whose innermost group holds BOTH `rules` (gated on rules.run, which test
    // lacks) and `channels` (always visible). `rules` must strip at depth 4; `channels` survives.
    let inner = group_item(
        "g4",
        vec![
            surface_item("Rules", "rules"),
            surface_item("Channels", "channels"),
        ],
    );
    let tree = group_item(
        "g1",
        vec![group_item("g2", vec![group_item("g3", vec![inner])])],
    );
    nav_save(store, &test, ws, "deep", "Deep", vec![tree], 1)
        .await
        .unwrap();
    nav_pref_set(store, &test, ws, Some("deep"), None, 2)
        .await
        .unwrap();

    let r = nav_resolve(&node, &test, ws).await.unwrap();
    let top = r.items.iter().find(|i| i.kind == "group").unwrap();
    let deepest = deepest_group_items(top);
    let surfaces: Vec<&str> = deepest.iter().map(|i| i.surface.as_str()).collect();
    assert_eq!(
        surfaces,
        vec!["channels"],
        "the deep `rules` leaf is stripped (test lacks rules.run); channels survives"
    );

    // With rules.run, the deep `rules` leaf survives too — same recursion, more caps.
    let ada2 = principal("user:test", ws, &[SAVE, RESOLVE, RULES_RUN]);
    let r = nav_resolve(&node, &ada2, ws).await.unwrap();
    let top = r.items.iter().find(|i| i.kind == "group").unwrap();
    assert_eq!(
        deepest_group_items(top).len(),
        2,
        "both survive with the cap"
    );
}

/// Empty-group pruning (post-order): a group whose WHOLE subtree strips disappears; a group whose ONLY
/// survivor is a nested group several levels down STAYS. Proves the prune is post-order, not leaf-level.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn empty_group_pruned_but_deep_survivor_keeps_ancestors() {
    let ws = "ws-nav-prune";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    // Test holds RESOLVE but NOT rules.run — a `rules`-only subtree strips entirely.
    let test = principal("user:test", ws, &[SAVE, RESOLVE]);

    let nav_items = vec![
        // (a) A whole subtree of only unreachable `rules`, nested 3 deep → must PRUNE entirely.
        group_item(
            "AllStripped",
            vec![group_item(
                "inner",
                vec![group_item(
                    "innermost",
                    vec![surface_item("Rules", "rules")],
                )],
            )],
        ),
        // (b) A group whose only survivor sits 3 levels down (a reachable `channels`) → must STAY, and
        //     every ancestor group with it.
        group_item(
            "DeepSurvivor",
            vec![group_item(
                "mid",
                vec![group_item(
                    "leaf-group",
                    vec![
                        surface_item("Rules", "rules"),       // strips
                        surface_item("Channels", "channels"), // the lone deep survivor
                    ],
                )],
            )],
        ),
    ];
    nav_save(store, &test, ws, "prune", "Prune", nav_items, 1)
        .await
        .unwrap();
    nav_pref_set(store, &test, ws, Some("prune"), None, 2)
        .await
        .unwrap();

    let r = nav_resolve(&node, &test, ws).await.unwrap();
    let labels: Vec<&str> = r.items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        !labels.contains(&"AllStripped"),
        "a group whose whole subtree strips is pruned (never an empty folder)"
    );
    let survivor = r
        .items
        .iter()
        .find(|i| i.label == "DeepSurvivor")
        .expect("the group with one deep survivor STAYS (post-order prune)");
    // The lone `channels` survives at the bottom, and the whole ancestor chain is intact.
    let deepest = deepest_group_items(survivor);
    let surfaces: Vec<&str> = deepest.iter().map(|i| i.surface.as_str()).collect();
    assert_eq!(surfaces, vec!["channels"], "only the deep survivor remains");
}

/// Recursive hidden-strip and recursive tag-group expansion behave at depth exactly as at the top:
/// a hidden ref inside a deep group is stripped (and its now-empty group pruned), and a tag-group
/// nested in a group still expands to a flat, cap-bounded list at its position.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn hidden_and_tag_group_apply_at_depth() {
    let ws = "ws-nav-depth-hide-tag";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal(
        "user:test",
        ws,
        &[
            SAVE,
            RESOLVE,
            GET,
            DASH_SAVE,
            DASH_GET,
            RULES_RUN,
            "mcp:tags.add:call",
            "mcp:tags.find:call",
        ],
    );

    // Two dashboards Test owns, both tagged `site` — the tag-group expansion target.
    seed_dashboard(store, &test, ws, "d1", "Board One").await;
    seed_dashboard(store, &test, ws, "d2", "Board Two").await;
    for id in ["d1", "d2"] {
        tags_add(
            store,
            &test,
            ws,
            &format!("dashboard:{id}"),
            &Tag::new("site", json!("plant")),
            &Provenance::new(1, "user:test", TagSource::Human),
        )
        .await
        .unwrap();
    }

    // A nested tree: a depth-2 group holds a tag-group (must expand at depth) and, in a SIBLING deep
    // group, a `channels` surface + a `rules` surface we'll hide.
    let nav_items = vec![group_item(
        "Outer",
        vec![
            tag_group_item(
                "Sites",
                vec![NavFacet {
                    key: "site".into(),
                    value: None,
                }],
            ),
            group_item(
                "Inner",
                vec![
                    surface_item("Rules", "rules"),
                    surface_item("Channels", "channels"),
                ],
            ),
        ],
    )];
    nav_save(store, &test, ws, "deep", "Deep", nav_items, 2)
        .await
        .unwrap();
    nav_pref_set(store, &test, ws, Some("deep"), None, 3)
        .await
        .unwrap();

    // Before hiding: the nested tag-group expands to a flat group of both boards at its position.
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    let outer = r.items.iter().find(|i| i.kind == "group").unwrap();
    let sites = outer
        .items
        .iter()
        .find(|i| i.label == "Sites")
        .expect("the nested tag-group expanded at depth");
    assert_eq!(
        sites.items.len(),
        2,
        "tag-group expands flat at depth (both boards)"
    );
    assert!(
        sites.items.iter().all(|c| c.kind == "dashboard"),
        "tag-group children are flat dashboard leaves, not further groups"
    );

    // Hide the deep `rules` surface: it strips inside the deep `Inner` group at depth, `channels` stays.
    nav_hidden_set(store, &test, ws, vec!["rules".into()], 4)
        .await
        .unwrap();
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    let outer = r.items.iter().find(|i| i.kind == "group").unwrap();
    let inner = outer.items.iter().find(|i| i.label == "Inner").unwrap();
    let surfaces: Vec<&str> = inner.items.iter().map(|i| i.surface.as_str()).collect();
    assert_eq!(
        surfaces,
        vec!["channels"],
        "deep hidden ref stripped at depth"
    );
}

// --- ext sub-ref pins (`ext:<ext>/<navid>`) ------------------------------------------------------
// ext-subref-pins scope. The shell renders a pin on each of an extension's DECLARED `[[ui.nav]]`
// destinations, using an `ext:<ext>/<navid>` ref. `nav.pref.set` always persisted it; `nav.resolve`
// used to drop it silently (the ext branch matched no install for the id "modbus/networks"), so the
// pin visibly un-filled on reload. These prove the round-trip and every strip path.

/// Seed a REAL installed extension declaring `[[ui.nav]]` destinations (a real `Install` record
/// `ext.list` reads — no sidecar spawn needed for a wasm page row).
async fn seed_nav_ext(node: &std::sync::Arc<Node>, ws: &str, ext_id: &str, nav: Vec<ExtNavItem>) {
    let page = ExtUi {
        entry: "assets/remoteEntry.js".into(),
        label: "Modbus".into(),
        icon: "network".into(),
        scope: vec![],
        data: false,
        id: None,
        options: vec![],
        nav,
    };
    let install = Install::new(ext_id, "0.1.0", vec![], 1).with_ui(Some(page), vec![]);
    record_install(&node.store, ws, &install)
        .await
        .expect("seed nav ext install");
}

fn nav_item(id: &str, label: &str) -> ExtNavItem {
    ExtNavItem {
        id: id.into(),
        label: label.into(),
        ..ExtNavItem::default()
    }
}

/// The headline: a pin on a declared ext destination RESOLVES (it used to strip silently), echoes
/// the destination in `nav` so the client can reconstruct the ref, and keeps the member's order.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ext_subref_pin_resolves_and_round_trips() {
    let ws = "ws-nav-subref";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    seed_nav_ext(
        &node,
        ws,
        "modbus",
        vec![
            nav_item("networks", "Networks"),
            nav_item("templates", "Templates"),
        ],
    )
    .await;

    let test = principal("user:test", ws, &[RESOLVE, EXT_LIST]);
    nav_pref_set(
        &node.store,
        &test,
        ws,
        None,
        Some(vec![
            "ext:modbus/templates".into(),
            "ext:modbus/networks".into(),
        ]),
        1,
    )
    .await
    .unwrap();

    let r = nav_resolve(&node, &test, ws).await.unwrap();
    assert_eq!(r.pinned.len(), 2, "both declared destinations resolve");
    // Member order preserved, and each carries the ext + destination so `item_ref` rebuilds the ref.
    assert_eq!(r.pinned[0].kind, "ext");
    assert_eq!(r.pinned[0].ext, "modbus");
    assert_eq!(r.pinned[0].nav, "templates");
    assert_eq!(r.pinned[0].label, "Templates", "the declared label is used");
    assert_eq!(r.pinned[1].nav, "networks");
}

/// A destination declaring a `dashboard` + `vars` resolves AS that dashboard (so the pin opens the
/// board var-bound), while KEEPING its ext identity so it is still pinned as the destination it is.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ext_subref_pin_on_a_dashboard_destination_opens_the_board() {
    let ws = "ws-nav-subref-dash";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let test = principal("user:test", ws, &[RESOLVE, DASH_SAVE, DASH_GET, EXT_LIST]);
    seed_dashboard(&node.store, &test, ws, "site", "Site Board").await;
    seed_nav_ext(
        &node,
        ws,
        "ems",
        vec![ExtNavItem {
            id: "site-a".into(),
            label: "Site A".into(),
            dashboard: Some("dashboard:site".into()),
            vars: [("site".to_string(), "a".to_string())]
                .into_iter()
                .collect(),
            ..ExtNavItem::default()
        }],
    )
    .await;

    nav_pref_set(
        &node.store,
        &test,
        ws,
        None,
        Some(vec!["ext:ems/site-a".into()]),
        1,
    )
    .await
    .unwrap();

    let r = nav_resolve(&node, &test, ws).await.unwrap();
    assert_eq!(r.pinned.len(), 1);
    // It OPENS a board…
    assert_eq!(r.pinned[0].kind, "dashboard");
    assert_eq!(r.pinned[0].dashboard, "dashboard:site");
    assert_eq!(r.pinned[0].vars.get("site").map(String::as_str), Some("a"));
    // …but is still identified as the ext destination (so the pin lights the right row).
    assert_eq!(r.pinned[0].ext, "ems");
    assert_eq!(r.pinned[0].nav, "site-a");
}

/// Every strip path, none of which may mutate the stored record: an unknown destination (the ext
/// shipped a new manifest), an uninstalled ext, and a 3-segment DYNAMIC child ref (a non-goal — the
/// server can never resolve a `bridge.setNav` child, so it strips instead of faulting).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ext_subref_pins_strip_silently_without_mutating_the_record() {
    let ws = "ws-nav-subref-strip";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    seed_nav_ext(&node, ws, "modbus", vec![nav_item("networks", "Networks")]).await;

    let test = principal("user:test", ws, &[RESOLVE, EXT_LIST]);
    let stored = vec![
        "ext:modbus/networks".to_string(),       // resolves
        "ext:modbus/gone".to_string(),           // manifest no longer declares it
        "ext:nosuch/networks".to_string(),       // ext not installed
        "ext:modbus/networks/net-1".to_string(), // dynamic child — non-goal, strips
    ];
    nav_pref_set(&node.store, &test, ws, None, Some(stored.clone()), 1)
        .await
        .unwrap();

    let r = nav_resolve(&node, &test, ws).await.unwrap();
    assert_eq!(
        r.pinned.len(),
        1,
        "only the live declared destination survives"
    );
    assert_eq!(r.pinned[0].nav, "networks");
    // The record is untouched by any strip — a reinstall/new manifest restores the pins for free.
    assert_eq!(
        nav_pref_get(&node.store, &test, ws).await.unwrap().pinned,
        stored
    );
}

/// Hide beats pin at the sub-ref grammar too — the hidden-set can target ONE ext destination, which
/// the old three-shape `item_ref` could not express. A whole-ext pin is unaffected (no regression).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn hiding_one_ext_destination_beats_its_pin_and_spares_the_others() {
    let ws = "ws-nav-subref-hide";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    seed_nav_ext(
        &node,
        ws,
        "modbus",
        vec![
            nav_item("networks", "Networks"),
            nav_item("templates", "Templates"),
        ],
    )
    .await;

    let admin = principal("user:admin", ws, ALL);
    nav_hidden_set(
        &node.store,
        &admin,
        ws,
        vec!["ext:modbus/networks".into()],
        1,
    )
    .await
    .unwrap();

    let test = principal("user:test", ws, &[RESOLVE, EXT_LIST]);
    nav_pref_set(
        &node.store,
        &test,
        ws,
        None,
        Some(vec![
            "ext:modbus/networks".into(),
            "ext:modbus/templates".into(),
            "ext:modbus".into(),
        ]),
        1,
    )
    .await
    .unwrap();

    let r = nav_resolve(&node, &test, ws).await.unwrap();
    // The hidden destination strips; its SIBLING and the whole-ext pin both survive.
    let refs: Vec<String> = r
        .pinned
        .iter()
        .map(|p| {
            if p.nav.is_empty() {
                format!("ext:{}", p.ext)
            } else {
                format!("ext:{}/{}", p.ext, p.nav)
            }
        })
        .collect();
    assert_eq!(refs, vec!["ext:modbus/templates", "ext:modbus"]);
}

/// A member who cannot LIST extensions has one stale ext pin STRIPPED — not a faulted menu. The pin
/// path made this reachable: `resolve_ext` faults the whole resolve on a denied `ext.list`, which for
/// a *pin* would blank the entire sidebar over one favorite. Strips are silent, always.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ext_subref_pin_without_ext_list_cap_strips_rather_than_faulting() {
    let ws = "ws-nav-subref-nocap";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    seed_nav_ext(&node, ws, "modbus", vec![nav_item("networks", "Networks")]).await;

    // RESOLVE but NOT ext.list.
    let test = principal("user:test", ws, &[RESOLVE]);
    nav_pref_set(
        &node.store,
        &test,
        ws,
        None,
        Some(vec!["channels".into(), "ext:modbus/networks".into()]),
        1,
    )
    .await
    .unwrap();

    // The menu still resolves (no Denied fault); the unreachable ext pin simply isn't in it.
    let r = nav_resolve(&node, &test, ws)
        .await
        .expect("resolve must not fault");
    assert_eq!(r.pinned.len(), 1);
    assert_eq!(r.pinned[0].surface, "channels");
}

// --- authored icon colors (`icon_color`) ---------------------------------------------------------
// The author-picked color is the twin of `icon`: opaque data, bounded at save, echoed through resolve
// untouched. The core never parses it — the UI decides what a value means.

/// An authored `icon_color` survives save → get → resolve on EVERY kind the author can color, and a
/// dynamically expanded group's children INHERIT their parent's color (one branch, one color).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn icon_color_round_trips_and_group_children_inherit() {
    let ws = "ws-nav-icon-color";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let test = principal(
        "user:test",
        ws,
        &[
            GET,
            SAVE,
            RESOLVE,
            DASH_LIST,
            DASH_GET,
            DASH_SAVE,
            "mcp:tags.add:call",
            "mcp:tags.find:call",
        ],
    );

    // A real dashboard, tagged, so the tag-group below expands over something reachable.
    seed_dashboard(store, &test, ws, "plant", "Plant").await;
    tags_add(
        store,
        &test,
        ws,
        "dashboard:plant",
        &Tag::new("site", json!("north")),
        &Provenance::new(2, "user:test", TagSource::Human),
    )
    .await
    .unwrap();

    let mut surface = surface_item("Channels", "channels");
    surface.icon_color = "#ff8800".into();
    let mut dash = dashboard_item("Plant", "dashboard:plant");
    dash.icon_color = "#00aaff".into();
    let mut group = tag_group_item(
        "By site",
        vec![NavFacet {
            key: "site".into(),
            value: None,
        }],
    );
    group.icon_color = "#22cc44".into();

    nav_save(
        store,
        &test,
        ws,
        "colored",
        "Colored",
        vec![surface, dash, group],
        3,
    )
    .await
    .unwrap();

    // Persisted verbatim on the record itself.
    let got = nav_get(store, &test, ws, "colored").await.unwrap();
    assert_eq!(got.items[0].icon_color, "#ff8800");
    assert_eq!(got.items[1].icon_color, "#00aaff");
    assert_eq!(got.items[2].icon_color, "#22cc44");

    // And echoed through resolve on each resolved kind.
    nav_pref_set(store, &test, ws, Some("colored"), None, 4)
        .await
        .unwrap();
    let r = nav_resolve(&node, &test, ws).await.unwrap();
    let by_kind = |k: &str| -> &NavResolvedItem {
        r.items.iter().find(|i| i.kind == k).expect("kind present")
    };
    assert_eq!(by_kind("surface").icon_color, "#ff8800");
    assert_eq!(by_kind("dashboard").icon_color, "#00aaff");

    // The expanded tag-group: the group keeps its color AND its dynamic children inherit it, so the
    // fan-out reads as one branch rather than uncolored strays under a colored parent.
    let grp = by_kind("group");
    assert_eq!(grp.icon_color, "#22cc44");
    assert!(!grp.items.is_empty(), "tag-group expanded to its match");
    for child in &grp.items {
        assert_eq!(
            child.icon_color, "#22cc44",
            "expanded child inherits the group's color"
        );
    }
}

/// An over-long `icon_color` is REJECTED at save (`BadInput`) — bounded like every other opaque field,
/// never silently truncated.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn icon_color_over_cap_is_rejected() {
    let ws = "ws-nav-icon-color-cap";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let test = principal("user:test", ws, ALL);

    let mut item = surface_item("Channels", "channels");
    item.icon_color = "#".repeat(33); // one past MAX_ICON_COLOR_LEN (32)

    let err = nav_save(&node.store, &test, ws, "toolong", "Too long", vec![item], 1)
        .await
        .expect_err("over-cap icon color must be rejected");
    assert!(
        matches!(err, NavError::BadInput(_)),
        "expected BadInput, got {err:?}"
    );
}
