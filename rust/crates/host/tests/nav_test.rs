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

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    add_member, dashboard_save, nav_delete, nav_get, nav_list, nav_list_shares, nav_pref_get,
    nav_pref_set, nav_resolve, nav_save, nav_set_default, nav_share, nav_unshare, tags_add, Cell,
    NavError, NavFacet, NavItem, NavResolvedSource, NavVisibility, Node, NAV_MAX_ITEMS,
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
    let ada = principal("user:ada", ws, ALL);

    // create
    let n = nav_save(
        &store,
        &ada,
        ws,
        "ops",
        "Operations",
        vec![surface_item("Channels", "channels")],
        10,
    )
    .await
    .unwrap();
    assert_eq!(n.title, "Operations");
    assert_eq!(n.owner, "user:ada");
    assert_eq!(n.visibility, NavVisibility::Private);

    // get reflects it (full items)
    let got = nav_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(got.items.len(), 1);
    assert_eq!(got.items[0].surface, "channels");

    // update (same id) — title + items change, owner preserved
    nav_save(
        &store,
        &ada,
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
    let got = nav_get(&store, &ada, ws, "ops").await.unwrap();
    assert_eq!(got.title, "Operations v2");
    assert_eq!(got.items.len(), 2);
    assert_eq!(got.updated_ts, 20);

    // list includes it (summary, no items)
    let roster = nav_list(&store, &ada, ws).await.unwrap();
    assert!(roster
        .iter()
        .any(|s| s.id == "ops" && s.title == "Operations v2"));

    // delete → list excludes it; get is NotFound
    nav_delete(&store, &ada, ws, "ops", 30).await.unwrap();
    let roster = nav_list(&store, &ada, ws).await.unwrap();
    assert!(!roster.iter().any(|s| s.id == "ops"));
    assert!(matches!(
        nav_get(&store, &ada, ws, "ops").await.unwrap_err(),
        NavError::NotFound
    ));

    // re-delete is an idempotent no-op
    nav_delete(&store, &ada, ws, "ops", 40).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn idempotent_upsert_by_slug() {
    let ws = "ws-nav-idem";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    // Two saves by the same slug are LWW — no duplicate row.
    nav_save(&store, &ada, ws, "ops", "One", vec![], 1)
        .await
        .unwrap();
    nav_save(&store, &ada, ws, "ops", "Two", vec![], 2)
        .await
        .unwrap();
    let roster = nav_list(&store, &ada, ws).await.unwrap();
    assert_eq!(roster.iter().filter(|s| s.id == "ops").count(), 1);
    assert_eq!(nav_get(&store, &ada, ws, "ops").await.unwrap().title, "Two");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn over_cap_items_rejected() {
    let ws = "ws-nav-bounds";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    // Over the item cap → rejected (the host is the boundary, not the builder).
    let too_many: Vec<NavItem> = (0..(NAV_MAX_ITEMS + 1))
        .map(|i| surface_item(&format!("s{i}"), "channels"))
        .collect();
    assert!(matches!(
        nav_save(&store, &ada, ws, "ops", "Ops", too_many, 1)
            .await
            .unwrap_err(),
        NavError::BadInput(_)
    ));

    // A nested `group` inside a `group` → rejected (one nesting level only).
    let nested = group_item("Outer", vec![group_item("Inner", vec![])]);
    assert!(matches!(
        nav_save(&store, &ada, ws, "ops2", "Ops", vec![nested], 1)
            .await
            .unwrap_err(),
        NavError::BadInput(_)
    ));

    // An unknown item kind → rejected.
    let mut bad = surface_item("x", "channels");
    bad.kind = "bogus".into();
    assert!(matches!(
        nav_save(&store, &ada, ws, "ops3", "Ops", vec![bad], 1)
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
    let ada = principal("user:ada", ws, ALL);
    nav_save(&store, &ada, ws, "ops", "Ops", vec![], 1)
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
        nav_pref_set(&store, &nobody, ws, "ops", 1)
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
    let ada = principal("user:ada", "ws-a", ALL);
    let ben = principal("user:ben", "ws-b", ALL);

    nav_save(store, &ada, "ws-a", "ops", "Ops A", vec![], 1)
        .await
        .unwrap();
    ada_sets_pick(store, &ada, "ws-a", "ops").await;

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

async fn ada_sets_pick(store: &Store, ada: &Principal, ws: &str, id: &str) {
    nav_pref_set(store, ada, ws, id, 5).await.unwrap();
}

// --- mandatory: gate-3 team-shared deny (non-member) --------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn team_shared_member_resolves_non_member_denied() {
    let ws = "ws-nav-share";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    // Ada owns + admins (needs `store:doc/*:write` to add a team member — the S4 edge).
    let ada = principal(
        "user:ada",
        ws,
        &[GET, LIST, SAVE, DELETE, SHARE, RESOLVE, "store:doc/*:write"],
    );
    let ben = principal("user:ben", ws, &[GET, LIST, RESOLVE]); // team member
    let cleo = principal("user:cleo", ws, &[GET, LIST, RESOLVE]); // NOT in the team

    nav_save(
        store,
        &ada,
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
    add_member(store, &ada, ws, "team:ops", "user:ben")
        .await
        .unwrap();
    nav_share(
        store,
        &ada,
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

    // Ada (admin) authors a workspace nav listing the `rules` surface AND a dashboard she owns.
    let ada = principal(
        "user:ada",
        ws,
        &[SAVE, SHARE, RESOLVE, GET, LIST, DASH_SAVE, DASH_GET],
    );
    seed_dashboard(store, &ada, ws, "secret", "Secret Board").await;
    nav_save(
        store,
        &ada,
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
    nav_share(store, &ada, ws, "ops", NavVisibility::Workspace, None, 2)
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
    // private board; here the board is Ada's private-by-default... actually it is workspace? No: the
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
    let ada = principal(
        "user:ada",
        ws,
        &[SAVE, SHARE, RESOLVE, GET, LIST, DELETE, "store:doc/*:write"],
    );

    // Empty state: no nav at all → fallback (never blank).
    assert_eq!(
        nav_resolve(&node, &ada, ws).await.unwrap().source,
        NavResolvedSource::Fallback
    );

    // A workspace-default nav → resolves to WorkspaceDefault.
    nav_save(store, &ada, ws, "wsdef", "WS Default", vec![], 1)
        .await
        .unwrap();
    nav_set_default(store, &ada, ws, "wsdef", 2).await.unwrap();
    assert_eq!(nav_resolve(&node, &ada, ws).await.unwrap().nav_id, "wsdef");
    assert_eq!(
        nav_resolve(&node, &ada, ws).await.unwrap().source,
        NavResolvedSource::WorkspaceDefault
    );

    // A team-shared nav Ada belongs to → beats the default.
    nav_save(store, &ada, ws, "teamnav", "Team", vec![], 3)
        .await
        .unwrap();
    add_member(store, &ada, ws, "team:ops", "user:ada")
        .await
        .unwrap();
    nav_share(
        store,
        &ada,
        ws,
        "teamnav",
        NavVisibility::Team,
        Some("team:ops"),
        4,
    )
    .await
    .unwrap();
    let r = nav_resolve(&node, &ada, ws).await.unwrap();
    assert_eq!(r.source, NavResolvedSource::Team);
    assert_eq!(r.nav_id, "teamnav");

    // A personal pick → beats the team share.
    nav_save(store, &ada, ws, "mine", "Mine", vec![], 5)
        .await
        .unwrap();
    nav_pref_set(store, &ada, ws, "mine", 6).await.unwrap();
    let r = nav_resolve(&node, &ada, ws).await.unwrap();
    assert_eq!(r.source, NavResolvedSource::Pick);
    assert_eq!(r.nav_id, "mine");

    // A stale pick (deleted nav) falls through to the next tier, not an error.
    nav_delete(store, &ada, ws, "mine", 7).await.unwrap();
    let r = nav_resolve(&node, &ada, ws).await.unwrap();
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
    let ada = principal(
        "user:ada",
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

    // Two dashboards Ada owns; a nav with a tag-group over `site`.
    seed_dashboard(store, &ada, ws, "plant-1", "Plant 1").await;
    seed_dashboard(store, &ada, ws, "plant-2", "Plant 2").await;
    nav_save(
        store,
        &ada,
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
    nav_pref_set(store, &ada, ws, "sites", 2).await.unwrap();

    // Before tagging: the tag-group is empty (no dashboard carries a `site` facet).
    let r = nav_resolve(&node, &ada, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    assert!(grp.items.is_empty(), "no tags yet → empty group");

    // Tag plant-1 with `site` → it appears on re-resolve (no nav edit).
    let prov = Provenance::new(3, "user:ada", TagSource::Human);
    tags_add(
        store,
        &ada,
        ws,
        "dashboard:plant-1",
        &Tag::new("site", json!("plant-1")),
        &prov,
    )
    .await
    .unwrap();
    let r = nav_resolve(&node, &ada, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    assert_eq!(grp.items.len(), 1);
    assert_eq!(grp.items[0].dashboard, "dashboard:plant-1");

    // Untag → it disappears.
    lb_host::tags_remove(store, &ada, ws, "dashboard:plant-1", "site", None)
        .await
        .unwrap();
    let r = nav_resolve(&node, &ada, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    assert!(grp.items.is_empty(), "untagged → gone");

    // A tag-group only surfaces dashboards the caller can READ: tag a dashboard owned by someone
    // else + not shared, and it does not appear for Ada... (build it as Ben's private board).
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
    let r = nav_resolve(&node, &ada, ws).await.unwrap();
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
    nav_pref_set(&store, &ben, ws, "somepick", 1).await.unwrap();
    assert_eq!(
        nav_pref_get(&store, &ben, ws).await.unwrap().active,
        "somepick"
    );

    // Ada's pick is independent — Ben's write never touched it (keyed by principal sub).
    let ada = principal("user:ada", ws, &[RESOLVE]);
    assert!(nav_pref_get(&store, &ada, ws)
        .await
        .unwrap()
        .active
        .is_empty());
    nav_pref_set(&store, &ada, ws, "adapick", 2).await.unwrap();
    // Ben's is still his own, unchanged.
    assert_eq!(
        nav_pref_get(&store, &ben, ws).await.unwrap().active,
        "somepick"
    );
    assert_eq!(
        nav_pref_get(&store, &ada, ws).await.unwrap().active,
        "adapick"
    );
}

// --- group nesting + surface strip inside a group -----------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn group_children_are_stripped_independently() {
    let ws = "ws-nav-group";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let store = &node.store;
    let ada = principal("user:ada", ws, &[SAVE, RESOLVE]);
    nav_save(
        store,
        &ada,
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
    nav_pref_set(store, &ada, ws, "admin", 2).await.unwrap();

    // Ada holds RESOLVE but NOT rules.run → inside the group, `rules` strips, `channels` stays.
    let r = nav_resolve(&node, &ada, ws).await.unwrap();
    let grp = r.items.iter().find(|i| i.kind == "group").unwrap();
    let surfaces: Vec<&str> = grp.items.iter().map(|i| i.surface.as_str()).collect();
    assert_eq!(
        surfaces,
        vec!["channels"],
        "rules stripped inside the group"
    );

    // With rules.run, `rules` survives too.
    let ada2 = principal("user:ada", ws, &[SAVE, RESOLVE, RULES_RUN, DASH_LIST]);
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
    let ada = principal(
        "user:ada",
        ws,
        &[GET, LIST, SAVE, SHARE, RESOLVE, "store:doc/*:write"],
    );

    nav_save(
        store,
        &ada,
        ws,
        "ops",
        "Ops",
        vec![surface_item("Channels", "channels")],
        1,
    )
    .await
    .unwrap();

    // Empty roster before any share.
    assert!(nav_list_shares(store, &ada, ws, "ops")
        .await
        .unwrap()
        .is_empty());

    // Share to TWO teams (each call writes one edge; the underlying relate is multi-edge).
    add_member(store, &ada, ws, "team:ops", "user:ben")
        .await
        .unwrap();
    add_member(store, &ada, ws, "team:eng", "user:cleo")
        .await
        .unwrap();
    nav_share(
        store,
        &ada,
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
        &ada,
        ws,
        "ops",
        NavVisibility::Team,
        Some("team:eng"),
        3,
    )
    .await
    .unwrap();

    // The roster reflects both — order unspecified, so compare as a set.
    let shares = nav_list_shares(store, &ada, ws, "ops").await.unwrap();
    let mut sorted = shares.clone();
    sorted.sort();
    assert_eq!(sorted, vec!["team:eng", "team:ops"]);

    // Both members resolve the nav (they're in a shared team).
    let ben = principal("user:ben", ws, &[GET, RESOLVE]);
    let cleo = principal("user:cleo", ws, &[GET, RESOLVE]);
    assert_eq!(nav_resolve(&node, &ben, ws).await.unwrap().nav_id, "ops");
    assert_eq!(nav_resolve(&node, &cleo, ws).await.unwrap().nav_id, "ops");

    // Revoke the ops share → roster drops it; ben stops resolving, cleo (still in team:eng) keeps it.
    nav_unshare(store, &ada, ws, "ops", "team:ops", 4)
        .await
        .unwrap();
    let shares = nav_list_shares(store, &ada, ws, "ops").await.unwrap();
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
    nav_unshare(store, &ada, ws, "ops", "team:ops", 5)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unshare_and_list_shares_denied_without_cap() {
    let ws = "ws-nav-shares-deny";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);
    let nobody = principal("user:nobody", ws, &[]);

    nav_save(&store, &ada, ws, "ops", "Ops", vec![], 1)
        .await
        .unwrap();
    nav_share(
        &store,
        &ada,
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
        nav_list_shares(&store, &ada, ws, "ops").await.unwrap(),
        vec!["team:ops"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_shares_and_unshare_owner_only_and_workspace_walled() {
    let ws = "ws-nav-shares-own";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);
    // A same-workspace peer who holds the share cap but is NOT the owner.
    let mallory = principal("user:mallory", ws, ALL);
    // A cross-workspace caller who owns a same-id nav over there.
    let ben = principal("user:ben", "ws-b", ALL);

    nav_save(&store, &ada, ws, "ops", "Ops", vec![], 1)
        .await
        .unwrap();
    nav_share(
        &store,
        &ada,
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
        nav_list_shares(&store, &ada, ws, "ops").await.unwrap(),
        vec!["team:ops"]
    );
}
