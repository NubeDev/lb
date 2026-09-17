//! Pinning a curated menu ROW, `nav:<navid>/<rowid>` (nav-row-pins scope), against a real node:
//! `nav.save` gives every row a stable id (kept / minted / duplicate re-minted / malformed refused),
//! `nav.resolve` echoes it, and a row pin resolves to the row itself — its bound variables, label and
//! folder trail, and for a folder everything inside it — while stripping silently (never touching `nav_pref`) when the row, the nav's
//! readability, the board's readability, or the hidden-set says it cannot render.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    dashboard_save, dashboard_share, nav_hidden_set, nav_pref_get, nav_pref_set, nav_resolve,
    nav_save, nav_share, Cell, DashboardVisibility, NavError, NavItem, NavResolvedItem,
    NavVisibility, Node,
};

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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

const SAVE: &str = "mcp:nav.save:call";
const SHARE: &str = "mcp:nav.share:call";
const RESOLVE: &str = "mcp:nav.resolve:call";
const DASH_GET: &str = "mcp:dashboard.get:call";
const DASH_SAVE: &str = "mcp:dashboard.save:call";
const DASH_SHARE: &str = "mcp:dashboard.share:call";
const AUTHOR: &[&str] = &[SAVE, SHARE, RESOLVE, DASH_GET, DASH_SAVE, DASH_SHARE];

fn vars(site: &str) -> BTreeMap<String, String> {
    BTreeMap::from([("site".to_string(), site.to_string())])
}

fn board(label: &str, dashboard: &str, site: &str) -> NavItem {
    NavItem {
        kind: "dashboard".into(),
        label: label.into(),
        dashboard: format!("dashboard:{dashboard}"),
        vars: vars(site),
        ..Default::default()
    }
}

fn folder(label: &str, overview: Option<&str>, site: &str, items: Vec<NavItem>) -> NavItem {
    NavItem {
        kind: "group".into(),
        label: label.into(),
        dashboard: overview
            .map(|d| format!("dashboard:{d}"))
            .unwrap_or_default(),
        vars: if overview.is_some() {
            vars(site)
        } else {
            BTreeMap::new()
        },
        items,
        ..Default::default()
    }
}

async fn seed_board(node: &Node, owner: &Principal, ws: &str, id: &str) {
    dashboard_save(
        &node.store,
        owner,
        ws,
        id,
        id,
        Vec::<Cell>::new(),
        vec![],
        1,
    )
    .await
    .unwrap();
}

/// The "Sites" menu: Chullora (opens `overview`, bound to chullora) › Energy (bound to chullora), and a
/// board-less "Plain" folder › Water. Returns the saved rows' ids: (chullora, energy, plain).
async fn seed_sites(node: &Node, owner: &Principal, ws: &str) -> (String, String, String) {
    for id in ["overview", "energy", "water"] {
        seed_board(node, owner, ws, id).await;
    }
    let saved = nav_save(
        &node.store,
        owner,
        ws,
        "sites",
        "Sites",
        vec![
            folder(
                "Chullora",
                Some("overview"),
                "chullora",
                vec![board("Energy", "energy", "chullora")],
            ),
            folder("Plain", None, "", vec![board("Water", "water", "chullora")]),
        ],
        1,
    )
    .await
    .unwrap();
    (
        saved.items[0].id.clone(),
        saved.items[0].items[0].id.clone(),
        saved.items[1].id.clone(),
    )
}

async fn pin(node: &Node, who: &Principal, ws: &str, refs: &[String]) {
    nav_pref_set(&node.store, who, ws, None, Some(refs.to_vec()), 5)
        .await
        .unwrap();
}

async fn pinned(node: &Arc<Node>, who: &Principal, ws: &str) -> Vec<NavResolvedItem> {
    nav_resolve(node, who, ws).await.unwrap().pinned
}

fn all_ids(items: &[NavItem], out: &mut Vec<String>) {
    for it in items {
        out.push(it.id.clone());
        all_ids(&it.items, out);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_keeps_mints_and_reminted_duplicate_row_ids() {
    let ws = "ws-rowpin-ids";
    let node = Node::boot().await.unwrap();
    let owner = principal("user:owner", ws, AUTHOR);

    let mut kept = folder("Site", None, "", vec![]);
    kept.id = "site-1".into();
    let mut dup = NavItem {
        kind: "surface".into(),
        surface: "channels".into(),
        ..Default::default()
    };
    dup.id = "site-1".into();
    let fresh = NavItem {
        kind: "surface".into(),
        surface: "inbox".into(),
        ..Default::default()
    };
    kept.items = vec![dup, fresh];

    let saved = nav_save(&node.store, &owner, ws, "m", "M", vec![kept], 1)
        .await
        .unwrap();
    let mut ids = Vec::new();
    all_ids(&saved.items, &mut ids);
    assert_eq!(ids[0], "site-1", "a valid id is kept");
    assert!(
        ids.iter().all(|i| !i.is_empty()),
        "every row gets an id: {ids:?}"
    );
    assert_eq!(
        ids.iter().collect::<HashSet<_>>().len(),
        3,
        "ids are unique: {ids:?}"
    );

    // Saving the returned tree back changes nothing — what the builder loads, it keeps.
    let again = nav_save(&node.store, &owner, ws, "m", "M", saved.items.clone(), 2)
        .await
        .unwrap();
    let mut again_ids = Vec::new();
    all_ids(&again.items, &mut again_ids);
    assert_eq!(again_ids, ids);

    // A malformed id is refused, not replaced.
    let bad = NavItem {
        id: "a/b".into(),
        kind: "surface".into(),
        surface: "inbox".into(),
        ..Default::default()
    };
    assert!(matches!(
        nav_save(&node.store, &owner, ws, "m", "M", vec![bad], 3)
            .await
            .unwrap_err(),
        NavError::BadInput(_)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resolve_echoes_row_ids_at_every_depth() {
    let ws = "ws-rowpin-echo";
    let node = Arc::new(Node::boot().await.unwrap());
    let owner = principal("user:owner", ws, AUTHOR);
    let (chullora, energy, _) = seed_sites(&node, &owner, ws).await;
    nav_pref_set(&node.store, &owner, ws, Some("sites"), None, 2)
        .await
        .unwrap();

    let r = nav_resolve(&node, &owner, ws).await.unwrap();
    assert_eq!(r.items[0].id, chullora);
    assert_eq!(r.items[0].items[0].id, energy);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn row_pin_resolves_as_the_row_binding_label_and_trail() {
    let ws = "ws-rowpin-shape";
    let node = Arc::new(Node::boot().await.unwrap());
    let owner = principal("user:owner", ws, AUTHOR);
    let (chullora, energy, plain) = seed_sites(&node, &owner, ws).await;
    let refs = [
        format!("nav:sites/{energy}"),
        format!("nav:sites/{chullora}"),
        format!("nav:sites/{plain}"),
    ];
    pin(&node, &owner, ws, &refs).await;

    let p = pinned(&node, &owner, ws).await;
    assert_eq!(p.len(), 3, "{p:?}");

    // A bound board keeps its binding, its own label, and the folder it sits in.
    assert_eq!(p[0].kind, "dashboard");
    assert_eq!(p[0].dashboard, "dashboard:energy");
    assert_eq!(p[0].label, "Energy");
    assert_eq!(p[0].vars, vars("chullora"));
    assert_eq!(
        (p[0].nav_id.as_str(), p[0].id.as_str()),
        ("sites", energy.as_str())
    );
    assert_eq!(p[0].trail, vec!["Chullora".to_string()]);

    // A pinned folder is the FOLDER: its own board beside it, and everything inside it — with row ids,
    // so the client can pin (and light) the pages inside the pinned copy too.
    assert_eq!(p[1].kind, "group");
    assert_eq!(p[1].label, "Chullora");
    assert_eq!(p[1].dashboard, "dashboard:overview");
    assert_eq!(p[1].vars, vars("chullora"));
    assert!(p[1].trail.is_empty());
    let inside: Vec<_> = p[1]
        .items
        .iter()
        .map(|c| (c.label.as_str(), c.id.as_str()))
        .collect();
    assert_eq!(inside, vec![("Energy", energy.as_str())]);

    // A folder with no board of its own is pinnable too — it still has its pages.
    assert_eq!(p[2].kind, "group");
    assert_eq!(p[2].label, "Plain");
    assert!(p[2].dashboard.is_empty());
    assert_eq!(p[2].items.len(), 1);
    assert_eq!(p[2].items[0].label, "Water");
    assert_eq!(p[2].nav_id, "sites");
}

/// Hide beats pin INSIDE a pinned folder: a hidden page drops out of the pinned copy, and a folder left
/// with nothing strips entirely — the menu's own `strip_hidden` rule.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pinned_folder_loses_hidden_pages_and_strips_when_empty() {
    let ws = "ws-rowpin-folder-hide";
    let node = Arc::new(Node::boot().await.unwrap());
    let owner = principal("user:owner", ws, AUTHOR);
    let (_, _, plain) = seed_sites(&node, &owner, ws).await;
    pin(&node, &owner, ws, &[format!("nav:sites/{plain}")]).await;
    assert_eq!(pinned(&node, &owner, ws).await[0].items.len(), 1);

    nav_hidden_set(&node.store, &owner, ws, vec!["dashboard:water".into()], 6)
        .await
        .unwrap();
    assert!(pinned(&node, &owner, ws).await.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn row_pin_strips_silently_and_restores_free() {
    let ws = "ws-rowpin-strip";
    let node = Arc::new(Node::boot().await.unwrap());
    let owner = principal("user:owner", ws, AUTHOR);
    let (_, energy, _) = seed_sites(&node, &owner, ws).await;
    let viewer = principal("user:viewer", ws, &[RESOLVE, DASH_GET]);
    let refs = vec![format!("nav:sites/{energy}")];
    pin(&node, &viewer, ws, &refs).await;

    // The nav is private to its owner → the viewer cannot read it → stripped.
    assert!(pinned(&node, &viewer, ws).await.is_empty());

    // Shared to the workspace, but the board is still private → the row resolves to nothing.
    nav_share(
        &node.store,
        &owner,
        ws,
        "sites",
        NavVisibility::Workspace,
        None,
        3,
    )
    .await
    .unwrap();
    assert!(pinned(&node, &viewer, ws).await.is_empty());

    // The board shared too → the pin renders, with no write to the viewer's pref in between.
    dashboard_share(
        &node.store,
        &owner,
        ws,
        "energy",
        DashboardVisibility::Workspace,
        None,
        4,
    )
    .await
    .unwrap();
    assert_eq!(pinned(&node, &viewer, ws).await.len(), 1);

    // Hide beats pin: hiding the row's underlying board strips it.
    nav_hidden_set(&node.store, &owner, ws, vec!["dashboard:energy".into()], 6)
        .await
        .unwrap();
    assert!(pinned(&node, &viewer, ws).await.is_empty());
    nav_hidden_set(&node.store, &owner, ws, vec![], 7)
        .await
        .unwrap();
    assert_eq!(pinned(&node, &viewer, ws).await.len(), 1);

    // The author deletes the row → stripped; a malformed ref never faults the menu.
    nav_save(&node.store, &owner, ws, "sites", "Sites", vec![], 8)
        .await
        .unwrap();
    pin(
        &node,
        &viewer,
        ws,
        &[refs[0].clone(), "nav:sites".into(), "nav:/x".into()],
    )
    .await;
    assert!(pinned(&node, &viewer, ws).await.is_empty());
    assert_eq!(
        nav_pref_get(&node.store, &viewer, ws)
            .await
            .unwrap()
            .pinned
            .len(),
        3,
        "a strip never mutates the stored pins"
    );
}
