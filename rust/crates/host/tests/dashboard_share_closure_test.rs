//! `dashboard.share_closure` — the remediation dual of `access_check` (share-closure scope), over the
//! REAL store, the real verbs, and the real caps grammar. No mocks.
//!
//! The live bug this closes: a member opens a team-shared dashboard, the PAGE renders, and every
//! embedded library panel that is still `private` shows "Panel not accessible — isn't shared with
//! you". `access_check` already detected that gap; this verb closes it — for the panels the caller
//! may actually share.
//!
//! Proves the mandatory categories from the scope:
//! - **Capability-deny:** no `mcp:dashboard.share_closure:call` → denied before any read.
//! - **Workspace-isolation:** a ws-B team is never a valid target; a ws-B caller cannot reach a ws-A
//!   dashboard's closure; a non-existent team refuses the WHOLE call with no partial application.
//! - **No-widening (load-bearing):** a not-owned panel is reported `not_owned` and is NOT shared —
//!   asserted by reading the `share` edges after a `dry_run=false` run. The verb is not a grant path.
//! - **Workspace-visible panels are not gaps** / **nested panels are `unchecked`**.
//! - **Dual consistency:** share_closure's gap set == access_check's red panel deps for that team.
//! - **Idempotency + incremental**, and the **end-to-end wall**: the shared-to team member can read
//!   the panel afterwards; a non-member still cannot (the wall did not move).

use lb_assets::list_related;
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::{team_create, Subject};
use lb_host::{
    add_member, dashboard_access_check, dashboard_save, dashboard_share, dashboard_share_closure,
    may_read_panel, panel_save, panel_share, read_panel, Cell, DashboardVisibility, PanelSpec,
    PanelVisibility, ShareClosureDisposition as D, ShareClosureReport,
};
use lb_store::Store;

/// A REAL store (the real SurrealDB in-memory engine — no mocks, CLAUDE §9), isolated per test.
///
/// NOT `Store::open("mem://")`: `open` hands its argument to SurrealKV as a **filesystem path**, so
/// `"mem://"` is a shared on-disk directory rather than an in-memory database — every test that opens
/// it shares (and corrupts) one log. `Store::memory()` is the actual in-memory constructor, and each
/// call is its own isolated instance.
async fn test_store() -> Store {
    Store::memory().await.expect("store opens")
}

/// A principal with exactly `caps` — the real token path (mint → verify), not a hand-built struct.
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

/// The author caps a page/panel owner needs to drive this whole flow. `store:doc/*:write` is what
/// `add_member` (the assets-surface team-membership seam) gates on; `authz.resolve` is what
/// `access_check` requires to preflight for a team (a foreign subject) in the dual-consistency test.
const AUTHOR: &[&str] = &[
    "mcp:dashboard.save:call",
    "mcp:dashboard.share:call",
    "mcp:dashboard.share_closure:call",
    "mcp:dashboard.get:call",
    "mcp:dashboard.access_check:call",
    "mcp:panel.get:call",
    "mcp:panel.save:call",
    "mcp:panel.share:call",
    "mcp:panel.delete:call",
    "mcp:authz.resolve:call",
    "store:doc/*:write",
];

/// A v3 cell referencing a library panel.
fn ref_cell(i: &str, panel_id: &str) -> Cell {
    Cell {
        i: i.into(),
        x: 0,
        y: 0,
        w: 6,
        h: 4,
        v: 3,
        panel_ref: format!("panel:{panel_id}"),
        ..Default::default()
    }
}

/// Save a private library panel owned by `owner`.
async fn save_panel(store: &Store, owner: &Principal, ws: &str, id: &str, title: &str) {
    panel_save(
        store,
        owner,
        ws,
        id,
        title,
        PanelSpec {
            v: 3,
            view: "chart".into(),
            ..Default::default()
        },
        1,
    )
    .await
    .expect("panel saves");
}

/// The teams a panel is `share`d to (the raw S4 edge — the ground truth the no-widening test reads).
async fn share_edges(store: &Store, ws: &str, panel_id: &str) -> Vec<String> {
    list_related(store, ws, "share", panel_id)
        .await
        .expect("share edges read")
}

fn dispo(report: &ShareClosureReport, panel: &str) -> D {
    report
        .panels
        .iter()
        .find(|p| p.panel == format!("panel:{panel}"))
        .unwrap_or_else(|| panic!("panel {panel} in report: {:#?}", report.panels))
        .disposition
}

/// **The edge-identity guard — the test this suite did not have, and needed most.**
///
/// `share_closure` shipped inert: it wrote `share__panel-…__team:ops` while the whole platform keys the
/// S4 graph on the BARE team id (`member__ops__user:bob`, `share__ops-page__ops`). Gate 3 resolves
/// "teams this panel is shared to" → "members of that team", so the prefixed edge dead-ended: nothing
/// is a member of `team:ops`. The verb reported `shared`, the panel flipped to `visibility: team`, and
/// the user's widget stayed "Panel not accessible" (`debugging/dashboard/share-closure-team-prefix-mismatch.md`).
///
/// **The 15 tests that passed could not see it**: they seeded membership with `"team:ops"` AND shared to
/// `"team:ops"`, so both of gate 3's hops matched each other. The fixture agreed with the fixture and
/// disagreed with reality.
///
/// So this asserts the edge SHAPE against the real writers, not against our own convention: the edge
/// `share_closure` writes must be byte-identical to the one `dashboard.share` writes for the same team,
/// and must key on the same string `add_member` used. If a future normalization drifts, this fails here
/// rather than in someone's browser.
#[tokio::test]
async fn writes_the_share_edge_on_the_same_team_id_the_member_and_dashboard_edges_use() {
    let store = test_store().await;
    let ws = "acme";
    let ada = principal("user:ada", ws, AUTHOR);
    team_create(&store, ws, "ops", "Ops").await.expect("team");
    // Seeded exactly as `members.add` does it live: the BARE team id.
    add_member(&store, &ada, ws, "ops", "user:bob")
        .await
        .expect("bob joins ops");

    save_panel(&store, &ada, ws, "cpu", "CPU").await;
    dashboard_save(
        &store,
        &ada,
        ws,
        "ops-page",
        "Ops Page",
        vec![ref_cell("a", "cpu")],
        vec![],
        1,
    )
    .await
    .expect("saves");
    // The reference edge: what the SHIPPED, working verb writes for this same team.
    dashboard_share(
        &store,
        &ada,
        ws,
        "ops-page",
        DashboardVisibility::Team,
        Some("ops"),
        1,
    )
    .await
    .expect("page shares");
    let dashboard_edge = list_related(&store, ws, "share", "ops-page")
        .await
        .expect("dashboard share edges");

    // Drive share_closure with the PREFIXED form — the caller may pass either; the stored edge must
    // still land on the graph's identity.
    dashboard_share_closure(&store, &ada, ws, "ops-page", "team:ops", false, 2)
        .await
        .expect("runs");
    let panel_edge = share_edges(&store, ws, "cpu").await;

    assert_eq!(
        panel_edge, dashboard_edge,
        "share_closure must write the SAME team id dashboard.share writes — a prefixed edge \
         dead-ends at gate 3's member hop and the widget stays 'Panel not accessible'"
    );
    assert_eq!(
        panel_edge,
        vec!["ops".to_string()],
        "the S4 graph keys teams BARE (member__ops__user:bob) — never `team:ops`"
    );

    // And the consequence that actually matters: bob really can read it.
    let bob = principal("user:bob", ws, &["mcp:panel.get:call"]);
    let panel = read_panel(&store, ws, "cpu").await.unwrap().unwrap();
    assert!(
        may_read_panel(&store, &bob, ws, &panel).await.is_ok(),
        "bob (a member of ops, via the REAL add_member edge) must read the shared panel"
    );
}

/// The live repro, closed end to end: ada's team-shared page embeds her private `cpu` panel; bob (in
/// `ops`) cannot read the panel — that is the "Panel not accessible" widget. share_closure(dry_run)
/// previews `would_share`, the confirm shares it, and bob can now read it. A non-member still cannot:
/// the wall moved for `ops` only.
#[tokio::test]
async fn shares_the_owned_closure_and_the_team_can_read_it() {
    let store = test_store().await;
    let ws = "acme";
    let ada = principal("user:ada", ws, AUTHOR);

    team_create(&store, ws, "ops", "Ops").await.expect("team");
    add_member(&store, &ada, ws, "ops", "user:bob")
        .await
        .expect("bob joins ops");

    save_panel(&store, &ada, ws, "cpu", "CPU").await;
    dashboard_save(
        &store,
        &ada,
        ws,
        "ops-page",
        "Ops Page",
        vec![ref_cell("a", "cpu")],
        vec![],
        1,
    )
    .await
    .expect("dashboard saves");
    dashboard_share(
        &store,
        &ada,
        ws,
        "ops-page",
        DashboardVisibility::Team,
        Some("ops"),
        1,
    )
    .await
    .expect("page shares to ops");

    // The bug: the page is shared, the panel is not — bob's widget is "Panel not accessible".
    let bob = principal("user:bob", ws, &["mcp:panel.get:call"]);
    let panel = read_panel(&store, ws, "cpu").await.unwrap().unwrap();
    assert!(
        may_read_panel(&store, &bob, ws, &panel).await.is_err(),
        "precondition: bob cannot read the private panel (the live repro)"
    );

    // Preview: mutates nothing.
    let preview = dashboard_share_closure(&store, &ada, ws, "ops-page", "team:ops", true, 2)
        .await
        .expect("dry run previews");
    assert_eq!(dispo(&preview, "cpu"), D::WouldShare);
    assert_eq!(preview.share_count(), 1);
    assert!(preview.dry_run);
    assert!(
        share_edges(&store, ws, "cpu").await.is_empty(),
        "a dry run must write NO share edge"
    );
    assert!(
        may_read_panel(&store, &bob, ws, &panel).await.is_err(),
        "a dry run must not move the wall"
    );

    // Confirm: the eligible share happens.
    let applied = dashboard_share_closure(&store, &ada, ws, "ops-page", "team:ops", false, 3)
        .await
        .expect("confirm shares");
    assert_eq!(dispo(&applied, "cpu"), D::Shared);
    assert!(!applied.dry_run);

    // bob's widget renders now — asserted through the REAL gate-3 the render path runs.
    let panel = read_panel(&store, ws, "cpu").await.unwrap().unwrap();
    assert!(
        may_read_panel(&store, &bob, ws, &panel).await.is_ok(),
        "bob (in ops) can now read the panel — the widget renders"
    );

    // ...and the wall did NOT move for anyone else.
    let mallory = principal("user:mallory", ws, &["mcp:panel.get:call"]);
    assert!(
        may_read_panel(&store, &mallory, ws, &panel).await.is_err(),
        "a non-ops member still cannot read it — the wall is unmoved"
    );
}

/// **The load-bearing no-widening test.** ada's page embeds aidan's panel — readable by HER (he shared
/// it to a team she is in) but not shared to the target team. A `dry_run=false` run must report it
/// `not_owned` and write NO share edge to the target — asserted against the raw S4 edges, not the
/// report (the report could lie; the edges cannot). This pins "the verb is not a grant path".
///
/// The fixture is the real shape of this gap, not a contrived one: `dashboard.save`'s
/// `validate_and_strip_refs` requires every `panel_ref` to resolve **under the saver**, so ada could
/// never have embedded a panel she cannot read. The gap arises exactly as the scope's example flow
/// describes — the panel's audience and the page's audience DIVERGE (aidan shared it to `design`, ada
/// is sharing the page to `ops`).
#[tokio::test]
async fn never_shares_a_panel_the_caller_does_not_own() {
    let store = test_store().await;
    let ws = "acme";
    let ada = principal("user:ada", ws, AUTHOR);
    let aidan = principal("user:aidan", ws, AUTHOR);

    team_create(&store, ws, "ops", "Ops").await.expect("team");
    team_create(&store, ws, "design", "Design")
        .await
        .expect("design team");
    add_member(&store, &ada, ws, "ops", "user:bob")
        .await
        .expect("bob joins ops");
    add_member(&store, &ada, ws, "design", "user:ada")
        .await
        .expect("ada is in design");

    save_panel(&store, &ada, ws, "cpu", "CPU").await; // ada's — shareable
    save_panel(&store, &aidan, ws, "aidan", "Aidan's").await; // NOT ada's — a gap
                                                              // aidan shares HIS panel to `design` (ada's team) — so ada may embed it, but its audience is
                                                              // `design`, NOT the `ops` team ada is about to share the page to.
    panel_share(
        &store,
        &aidan,
        ws,
        "aidan",
        PanelVisibility::Team,
        Some("design"),
        1,
    )
    .await
    .expect("aidan shares his panel to design");

    dashboard_save(
        &store,
        &ada,
        ws,
        "ops-page",
        "Ops Page",
        vec![ref_cell("a", "cpu"), ref_cell("b", "aidan")],
        vec![],
        1,
    )
    .await
    .expect("dashboard saves");

    let report = dashboard_share_closure(&store, &ada, ws, "ops-page", "team:ops", false, 2)
        .await
        .expect("runs");

    assert_eq!(dispo(&report, "cpu"), D::Shared, "she owns cpu");
    assert_eq!(
        dispo(&report, "aidan"),
        D::NotOwned,
        "aidan's panel is a gap ada cannot close"
    );

    // The ground truth: the S4 edges.
    assert_eq!(
        share_edges(&store, ws, "cpu").await,
        vec!["ops".to_string()],
        "the owned panel IS shared"
    );
    // The not-owned panel keeps EXACTLY its original audience (`design`) — `ops` was never added.
    // This is the assertion that pins "the verb is not a grant path": ada could read the panel, embed
    // it, and run the verb, and still could not widen it to her team.
    assert_eq!(
        share_edges(&store, ws, "aidan").await,
        vec!["design".to_string()],
        "NO share edge to the target was written for the not-owned panel — not a grant path"
    );

    // And the wall really did not move for aidan's panel.
    let bob = principal("user:bob", ws, &["mcp:panel.get:call"]);
    let aidans = read_panel(&store, ws, "aidan").await.unwrap().unwrap();
    assert!(
        may_read_panel(&store, &bob, ws, &aidans).await.is_err(),
        "bob still cannot read aidan's panel"
    );

    // The report names the owner, so the UI can say "ask aidan".
    let row = report
        .panels
        .iter()
        .find(|p| p.panel == "panel:aidan")
        .unwrap();
    assert!(
        row.reason.contains("user:aidan"),
        "the gap names its owner: {}",
        row.reason
    );
}

/// **Capability-deny (mandatory).** No `mcp:dashboard.share_closure:call` → denied. Holding
/// `panel.share` does not help: the verb's own cap is the gate.
#[tokio::test]
async fn denies_a_caller_without_the_cap() {
    let store = test_store().await;
    let ws = "acme";
    let ada = principal("user:ada", ws, AUTHOR);
    team_create(&store, ws, "ops", "Ops").await.expect("team");
    save_panel(&store, &ada, ws, "cpu", "CPU").await;
    dashboard_save(
        &store,
        &ada,
        ws,
        "ops-page",
        "Ops Page",
        vec![ref_cell("a", "cpu")],
        vec![],
        1,
    )
    .await
    .expect("saves");

    // Every panel cap, but NOT the verb's cap.
    let capless = principal(
        "user:ada",
        ws,
        &["mcp:panel.share:call", "mcp:panel.get:call"],
    );
    let err = dashboard_share_closure(&store, &capless, ws, "ops-page", "team:ops", true, 2).await;
    assert!(
        err.is_err(),
        "denied without mcp:dashboard.share_closure:call"
    );
}

/// **Workspace-isolation (mandatory).** A ws-B caller cannot reach a ws-A dashboard's closure — the
/// record is structurally invisible across the wall.
#[tokio::test]
async fn a_foreign_workspace_caller_cannot_reach_the_closure() {
    let store = test_store().await;
    let ada = principal("user:ada", "acme", AUTHOR);
    team_create(&store, "acme", "ops", "Ops")
        .await
        .expect("team");
    save_panel(&store, &ada, "acme", "cpu", "CPU").await;
    dashboard_save(
        &store,
        &ada,
        "acme",
        "ops-page",
        "Ops Page",
        vec![ref_cell("a", "cpu")],
        vec![],
        1,
    )
    .await
    .expect("saves");

    // A ws-B principal (token scoped to `other`) reaching into ws `acme`.
    let intruder = principal("user:eve", "other", AUTHOR);
    let err =
        dashboard_share_closure(&store, &intruder, "acme", "ops-page", "team:ops", true, 2).await;
    assert!(err.is_err(), "a ws-B caller cannot reach a ws-A closure");
}

/// **Workspace-isolation, the bulk-specific edge.** A target team that does not exist in the caller's
/// workspace (a typo, or a ws-B team name) refuses the WHOLE call before any write — asserted by
/// finding no share edge on the owned panel afterwards. No partial application, no dangling edges.
#[tokio::test]
async fn a_nonexistent_team_target_refuses_the_whole_call_with_no_partial_writes() {
    let store = test_store().await;
    let ws = "acme";
    let ada = principal("user:ada", ws, AUTHOR);

    // The team exists in ws-B, NOT in ws-A — it must never become a ws-A panel's audience.
    team_create(&store, "other", "ops", "Ops (ws-B)")
        .await
        .expect("ws-B team");

    save_panel(&store, &ada, ws, "cpu", "CPU").await;
    save_panel(&store, &ada, ws, "mem", "Mem").await;
    dashboard_save(
        &store,
        &ada,
        ws,
        "ops-page",
        "Ops Page",
        vec![ref_cell("a", "cpu"), ref_cell("b", "mem")],
        vec![],
        1,
    )
    .await
    .expect("saves");

    let err = dashboard_share_closure(&store, &ada, ws, "ops-page", "team:ops", false, 2).await;
    assert!(err.is_err(), "a team absent from THIS workspace is refused");

    assert!(
        share_edges(&store, ws, "cpu").await.is_empty()
            && share_edges(&store, ws, "mem").await.is_empty(),
        "NO panel got a dangling edge — the call refused before any write (no partial application)"
    );
}

/// **Sharing ADDS an audience, it never replaces one.** A panel already shared to `design` that also
/// gets shared to `ops` must keep BOTH: the `share` edge is additive and `panel_share` writes the
/// `team` tier it already had. This pins the other direction of "the wall does not move" — the verb
/// must not silently NARROW a panel's existing audience as a side effect of widening it to a new team
/// (the mirror of the no-widening rule, and just as much a surprise to the panel's owner).
#[tokio::test]
async fn sharing_to_a_second_team_keeps_the_first_team_s_access() {
    let store = test_store().await;
    let ws = "acme";
    let ada = principal("user:ada", ws, AUTHOR);
    team_create(&store, ws, "ops", "Ops").await.expect("team");
    team_create(&store, ws, "design", "Design")
        .await
        .expect("team");
    add_member(&store, &ada, ws, "ops", "user:bob")
        .await
        .expect("bob joins ops");
    add_member(&store, &ada, ws, "design", "user:dee")
        .await
        .expect("dee joins design");

    save_panel(&store, &ada, ws, "cpu", "CPU").await;
    panel_share(
        &store,
        &ada,
        ws,
        "cpu",
        PanelVisibility::Team,
        Some("design"),
        1,
    )
    .await
    .expect("cpu is shared to design first");

    dashboard_save(
        &store,
        &ada,
        ws,
        "ops-page",
        "Ops Page",
        vec![ref_cell("a", "cpu")],
        vec![],
        1,
    )
    .await
    .expect("saves");

    // Now share the same panel's closure to a DIFFERENT team.
    let report = dashboard_share_closure(&store, &ada, ws, "ops-page", "team:ops", false, 2)
        .await
        .expect("runs");
    assert_eq!(dispo(&report, "cpu"), D::Shared);

    // Both audiences survive — asserted on the edges AND through the live gate-3 for a member of each.
    let mut edges = share_edges(&store, ws, "cpu").await;
    edges.sort();
    assert_eq!(
        edges,
        vec!["design".to_string(), "ops".to_string()],
        "sharing to ops must ADD an audience, not replace design's"
    );
    let panel = read_panel(&store, ws, "cpu").await.unwrap().unwrap();
    let dee = principal("user:dee", ws, &["mcp:panel.get:call"]);
    let bob = principal("user:bob", ws, &["mcp:panel.get:call"]);
    assert!(
        may_read_panel(&store, &dee, ws, &panel).await.is_ok(),
        "design (the ORIGINAL audience) must not lose access"
    );
    assert!(
        may_read_panel(&store, &bob, ws, &panel).await.is_ok(),
        "ops (the new audience) gains access"
    );
}

/// A `workspace`-visible panel is **not a gap**: every member can already read it, so a team share is
/// a no-op. Reporting it as a gap would make the UI nag to "fix" a panel that needs nothing.
#[tokio::test]
async fn a_workspace_visible_panel_is_not_a_gap() {
    let store = test_store().await;
    let ws = "acme";
    let ada = principal("user:ada", ws, AUTHOR);
    team_create(&store, ws, "ops", "Ops").await.expect("team");

    save_panel(&store, &ada, ws, "cpu", "CPU").await;
    panel_share(&store, &ada, ws, "cpu", PanelVisibility::Workspace, None, 1)
        .await
        .expect("panel goes workspace-visible");

    dashboard_save(
        &store,
        &ada,
        ws,
        "ops-page",
        "Ops Page",
        vec![ref_cell("a", "cpu")],
        vec![],
        1,
    )
    .await
    .expect("saves");

    let report = dashboard_share_closure(&store, &ada, ws, "ops-page", "team:ops", false, 2)
        .await
        .expect("runs");
    assert_eq!(dispo(&report, "cpu"), D::AlreadyVisibleWorkspace);
    assert_eq!(
        report.share_count(),
        0,
        "nothing to share — no offer to make"
    );
    assert!(
        report.gaps().count() == 0,
        "a workspace-visible panel is not a gap"
    );
    assert!(
        share_edges(&store, ws, "cpu").await.is_empty(),
        "no pointless team edge written for an already-workspace-visible panel"
    );
}

/// **Idempotency + incremental.** Re-running shares nothing new; adding a panel and re-running shares
/// only the new one.
#[tokio::test]
async fn is_idempotent_and_shares_only_the_newly_added_panel() {
    let store = test_store().await;
    let ws = "acme";
    let ada = principal("user:ada", ws, AUTHOR);
    team_create(&store, ws, "ops", "Ops").await.expect("team");
    add_member(&store, &ada, ws, "ops", "user:bob")
        .await
        .expect("bob joins");

    save_panel(&store, &ada, ws, "cpu", "CPU").await;
    dashboard_save(
        &store,
        &ada,
        ws,
        "ops-page",
        "Ops Page",
        vec![ref_cell("a", "cpu")],
        vec![],
        1,
    )
    .await
    .expect("saves");

    let first = dashboard_share_closure(&store, &ada, ws, "ops-page", "team:ops", false, 2)
        .await
        .expect("first run");
    assert_eq!(first.share_count(), 1);

    // Re-run: nothing new.
    let second = dashboard_share_closure(&store, &ada, ws, "ops-page", "team:ops", false, 3)
        .await
        .expect("second run");
    assert_eq!(dispo(&second, "cpu"), D::AlreadyShared);
    assert_eq!(second.share_count(), 0, "re-running shares nothing");

    // Add a panel; only the new one shares.
    save_panel(&store, &ada, ws, "mem", "Mem").await;
    dashboard_save(
        &store,
        &ada,
        ws,
        "ops-page",
        "Ops Page",
        vec![ref_cell("a", "cpu"), ref_cell("b", "mem")],
        vec![],
        4,
    )
    .await
    .expect("re-saves with the new panel");

    let third = dashboard_share_closure(&store, &ada, ws, "ops-page", "team:ops", false, 5)
        .await
        .expect("third run");
    assert_eq!(dispo(&third, "cpu"), D::AlreadyShared);
    assert_eq!(dispo(&third, "mem"), D::Shared);
    assert_eq!(third.share_count(), 1, "only the newly added panel shares");
}

/// **Dual consistency — the anti-drift guarantee.** The panels `share_closure(dry_run=true)` reports
/// as gate-3 gaps are EXACTLY the panels `access_check` reports red for that same team. The read and
/// the write agree about the closure, across their two deliberately different report shapes.
#[tokio::test]
async fn share_closure_gaps_match_access_check_red_panels() {
    let store = test_store().await;
    let ws = "acme";
    let ada = principal("user:ada", ws, AUTHOR);
    let aidan = principal("user:aidan", ws, AUTHOR);
    team_create(&store, ws, "ops", "Ops").await.expect("team");
    team_create(&store, ws, "design", "Design")
        .await
        .expect("design team");
    add_member(&store, &ada, ws, "ops", "user:bob")
        .await
        .expect("bob joins");
    add_member(&store, &ada, ws, "design", "user:ada")
        .await
        .expect("ada is in design");

    // A deliberately mixed closure: owned+private (a closable gap), not-owned (an unclosable gap —
    // shared to ada's `design` team so she could embed it, but not to the `ops` target),
    // workspace-visible (not a gap), and already-shared-to-ops (not a gap).
    save_panel(&store, &ada, ws, "cpu", "CPU").await;
    save_panel(&store, &aidan, ws, "aidan", "Aidan's").await;
    panel_share(
        &store,
        &aidan,
        ws,
        "aidan",
        PanelVisibility::Team,
        Some("design"),
        1,
    )
    .await
    .expect("aidan shares to design, not ops");
    save_panel(&store, &ada, ws, "wide", "Wide").await;
    panel_share(
        &store,
        &ada,
        ws,
        "wide",
        PanelVisibility::Workspace,
        None,
        1,
    )
    .await
    .expect("wide goes workspace");
    save_panel(&store, &ada, ws, "done", "Done").await;
    panel_share(
        &store,
        &ada,
        ws,
        "done",
        PanelVisibility::Team,
        Some("ops"),
        1,
    )
    .await
    .expect("done is already shared");

    dashboard_save(
        &store,
        &ada,
        ws,
        "ops-page",
        "Ops Page",
        vec![
            ref_cell("a", "cpu"),
            ref_cell("b", "aidan"),
            ref_cell("c", "wide"),
            ref_cell("d", "done"),
        ],
        vec![],
        1,
    )
    .await
    .expect("saves");

    // The write's view of the gaps.
    let report = dashboard_share_closure(&store, &ada, ws, "ops-page", "team:ops", true, 2)
        .await
        .expect("preview");
    let mut share_closure_gaps: Vec<String> = report.gaps().map(|p| p.panel.clone()).collect();
    share_closure_gaps.sort();

    // The read's view of the same closure, for the same team.
    let team = Subject::parse("team:ops").unwrap();
    let access = dashboard_access_check(&store, &ada, ws, "ops-page", &team)
        .await
        .expect("access_check");
    let mut access_check_red_panels: Vec<String> = access
        .dependencies
        .iter()
        .filter(|d| d.kind == lb_host::DepKind::Panel && !d.ok && !d.unchecked)
        .map(|d| d.dep.clone())
        .collect();
    access_check_red_panels.sort();

    assert_eq!(
        share_closure_gaps, access_check_red_panels,
        "the remediation and the detection must agree about the closure's gaps"
    );
    // Sanity: the fixture really does contain both a closable and an unclosable gap.
    assert_eq!(
        share_closure_gaps,
        vec!["panel:aidan".to_string(), "panel:cpu".to_string()]
    );
}

/// A panel the caller OWNS but lacks `mcp:panel.share:call` for reports `no_share_cap` — distinct from
/// `not_owned`, because the human fix is different (ask an admin for a cap vs. ask a person for their
/// panel). And nothing is shared.
#[tokio::test]
async fn an_owner_without_the_share_cap_reports_no_share_cap_and_shares_nothing() {
    let store = test_store().await;
    let ws = "acme";
    let ada = principal("user:ada", ws, AUTHOR);
    team_create(&store, ws, "ops", "Ops").await.expect("team");
    save_panel(&store, &ada, ws, "cpu", "CPU").await;
    dashboard_save(
        &store,
        &ada,
        ws,
        "ops-page",
        "Ops Page",
        vec![ref_cell("a", "cpu")],
        vec![],
        1,
    )
    .await
    .expect("saves");

    // Same human, same owned panel — but no `panel.share` cap.
    let ada_no_share = principal(
        "user:ada",
        ws,
        &[
            "mcp:dashboard.share_closure:call",
            "mcp:dashboard.get:call",
            "mcp:panel.get:call",
        ],
    );
    let report =
        dashboard_share_closure(&store, &ada_no_share, ws, "ops-page", "team:ops", false, 2)
            .await
            .expect("runs");
    assert_eq!(dispo(&report, "cpu"), D::NoShareCap);
    assert!(
        share_edges(&store, ws, "cpu").await.is_empty(),
        "no cap, no share"
    );
}

/// A caller who can SEE a page but does not own it may still share the panels they own on it (the
/// per-panel owner rule is the real wall, not page ownership) — and a page they cannot see at all is
/// not theirs to enumerate.
#[tokio::test]
async fn page_visibility_gates_the_closure_but_panel_ownership_gates_each_share() {
    let store = test_store().await;
    let ws = "acme";
    let ada = principal("user:ada", ws, AUTHOR);
    let aidan = principal("user:aidan", ws, AUTHOR);
    team_create(&store, ws, "ops", "Ops").await.expect("team");
    add_member(&store, &ada, ws, "ops", "user:aidan")
        .await
        .expect("aidan joins ops");

    // ada's page (shared to ops, so aidan can SEE it) embeds AIDAN's panel. He shares it to `design`
    // — ada's team — so she may embed it (`validate_and_strip_refs` requires the ref resolve under
    // the saver); its audience is still not `ops`, so it remains a real gap only HE can close.
    team_create(&store, ws, "design", "Design")
        .await
        .expect("design team");
    add_member(&store, &ada, ws, "design", "user:ada")
        .await
        .expect("ada is in design");
    save_panel(&store, &aidan, ws, "aidan", "Aidan's").await;
    panel_share(
        &store,
        &aidan,
        ws,
        "aidan",
        PanelVisibility::Team,
        Some("design"),
        1,
    )
    .await
    .expect("aidan shares to design so ada can embed it");
    dashboard_save(
        &store,
        &ada,
        ws,
        "ops-page",
        "Ops Page",
        vec![ref_cell("a", "aidan")],
        vec![],
        1,
    )
    .await
    .expect("saves");
    dashboard_share(
        &store,
        &ada,
        ws,
        "ops-page",
        DashboardVisibility::Team,
        Some("ops"),
        1,
    )
    .await
    .expect("shares page");

    // aidan doesn't own the PAGE but owns the PANEL — he can close the gap.
    let report = dashboard_share_closure(&store, &aidan, ws, "ops-page", "team:ops", false, 2)
        .await
        .expect("a page-viewer who owns the panel may share it");
    assert_eq!(dispo(&report, "aidan"), D::Shared);

    // A stranger who cannot see the page at all cannot enumerate its closure.
    let stranger = principal("user:mallory", ws, AUTHOR);
    assert!(
        dashboard_share_closure(&store, &stranger, ws, "ops-page", "team:ops", true, 3)
            .await
            .is_err(),
        "a page you cannot read is not a closure you may enumerate"
    );
}

/// A dangling `panel_ref` (the panel was deleted) is reported `unchecked`, not silently dropped —
/// the page is broken for everyone and no share fixes it, so the report must say so honestly rather
/// than claim the closure is fully shared.
#[tokio::test]
async fn a_dangling_panel_ref_is_reported_not_silently_dropped() {
    let store = test_store().await;
    let ws = "acme";
    let ada = principal("user:ada", ws, AUTHOR);
    team_create(&store, ws, "ops", "Ops").await.expect("team");

    // `dashboard.save` validates every ref resolves under the saver, so a page can never be BUILT
    // around a missing panel. The dangling ref arises the real way: the panel is deleted out from
    // under a page that already embedded it ("validate at write, tolerate at read").
    save_panel(&store, &ada, ws, "ghost", "Ghost").await;
    dashboard_save(
        &store,
        &ada,
        ws,
        "ops-page",
        "Ops Page",
        vec![ref_cell("a", "ghost")],
        vec![],
        1,
    )
    .await
    .expect("saves with a live ref");
    lb_host::panel_delete(&store, &ada, ws, "ghost", true, 2)
        .await
        .expect("panel is deleted out from under the page");

    let report = dashboard_share_closure(&store, &ada, ws, "ops-page", "team:ops", true, 3)
        .await
        .expect("runs");
    assert_eq!(
        dispo(&report, "ghost"),
        D::Unchecked,
        "a dangling ref is reported, not dropped"
    );
    assert_eq!(report.share_count(), 0);
}
