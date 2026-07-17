//! `dashboard.share_closure(dashboard, team, dry_run)` — the **remediation** half of the access model
//! (share-closure scope). `dashboard.access_check` DETECTS that a team-shared page's embedded library
//! panels are still private ("Panel not accessible" on every widget); this verb CLOSES that gap, for
//! the panels the caller may actually share, as an explicit capability-bounded action.
//!
//! **It is not a grant path — that is the whole point.** A panel is a first-class asset whose audience
//! is its owner's to choose (`library-panels-scope.md`), so this verb never shares a panel the caller
//! does not own: it REPORTS it as a gap for the owner to close. Auto-sharing on embed / on
//! `dashboard.share` was rejected precisely because "drop a panel on a team page" would silently widen
//! who can read it — the widen-by-reference class of
//! `debugging/auth/member-wildcard-satisfies-admin-cap.md`. The offer is one click; a click it stays.
//!
//! **Nothing here re-implements a wall.** The closure comes from the shared [`closure_panels`]
//! enumeration `access_check` also walks (so the two can never disagree about what the closure is);
//! gate-3 is `panel::may_read_panel` verbatim; the owner check + the S4 `share` edge write are
//! `panel::panel_share` verbatim (this module never calls `relate` itself). What it adds is the
//! per-panel DISPOSITION: the caller-side question "is this gap closable by you?", which access_check
//! (which reasons about the subject's reach) does not answer.
//!
//! **`dry_run` defaults to true** — a plan-only call is safe, the mutation is opt-in (the
//! `federation.migrate` posture). The preview is what the UI shows before asking for a confirm.

use lb_auth::Principal;
use lb_authz::{team_list, Subject};
use lb_mcp::authorize_tool;
use lb_store::Store;
use serde::{Deserialize, Serialize};

use super::authorize::authorize_dashboard;
use super::closure::closure_panels;
use super::error::DashboardError;
use super::store::read_dashboard;
use crate::panel::{may_read_panel, panel_share, read_panel, Panel, Visibility as PanelVisibility};

/// What `share_closure` would do / did to one panel in the closure. Exactly one applies per panel.
///
/// The split between [`NotOwned`](Disposition::NotOwned) and [`NoShareCap`](Disposition::NoShareCap)
/// is deliberate (scope, resolved decision 2): they are different gaps with different human fixes —
/// "ask the owner for their panel" vs. "ask an admin for a capability". Collapsing them would tell the
/// owner of a panel to go ask its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    /// The caller owns it, holds `panel.share`, and the team cannot read it yet — `dry_run=true`.
    WouldShare,
    /// The same, after the write — `dry_run=false`. The team can read it now.
    Shared,
    /// A `share` edge to this team already exists and the panel is team-visible (idempotency).
    AlreadyShared,
    /// The panel is `workspace`-visible: every workspace member can ALREADY read it, so a team share
    /// is a no-op. **Not a gap** — reporting it as one would make the UI nag to "fix" a panel that
    /// needs nothing (`may_read_panel` returns `Ok` for `Workspace` before any team walk).
    AlreadyVisibleWorkspace,
    /// A real gap the caller CANNOT close: they do not own the panel. Never force-shared.
    NotOwned,
    /// The caller owns it but lacks `mcp:panel.share:call` — closable, but not by them, today.
    NoShareCap,
    /// A hop v1 does not walk (nested panel→panel). Neither shared nor called green.
    Unchecked,
}

impl Disposition {
    /// True iff this disposition means the team currently CANNOT read the panel — a real gate-3 gap.
    /// This is the set the dual-consistency test compares against `access_check`'s red panel deps.
    pub fn is_gap(self) -> bool {
        matches!(
            self,
            Disposition::WouldShare | Disposition::NotOwned | Disposition::NoShareCap
        )
    }
}

/// One panel's row in the report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShareClosureItem {
    /// The panel id, `panel:`-prefixed (the form the UI and `access_check`'s `dep` both use).
    pub panel: String,
    /// The panel's title, so the preview can name it without a second read. Empty if unreadable.
    #[serde(default)]
    pub title: String,
    /// The first cell (`Cell.i`) embedding it — lets the UI point at the tile.
    #[serde(default)]
    pub cell: String,
    pub disposition: Disposition,
    /// A one-line, non-secret explanation. For `not_owned` it names the owner (so the UI can say "ask
    /// aidan") — an owner is not a secret: `panel.list`/`get` already expose it to anyone who may read
    /// the panel, and the page's own owner necessarily knows whose panel they embedded.
    pub reason: String,
}

/// The whole preview/result. Panel-centric by design — NOT `AccessReport`'s dep-centric shape (scope,
/// "Report shape"): "what would this write do?" is a different question from "will this page render?".
/// The two are bridged by the dual-consistency test, not by a shared struct.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShareClosureReport {
    pub dashboard: String,
    /// The share target, as the BARE team id (`ops`) — the identity the S4 `member`/`share` edges key
    /// on and what `teams.list` returns, whichever form the caller passed in.
    pub team: String,
    /// True = nothing was mutated (the preview). False = the eligible shares were performed.
    pub dry_run: bool,
    pub panels: Vec<ShareClosureItem>,
}

impl ShareClosureReport {
    /// How many panels this run shared (or would share) — the count the UI's offer copy uses
    /// ("Share the N widgets on this page too?").
    pub fn share_count(&self) -> usize {
        self.panels
            .iter()
            .filter(|p| matches!(p.disposition, Disposition::WouldShare | Disposition::Shared))
            .count()
    }

    /// The panels the team cannot read today — every real gate-3 gap, closable or not.
    pub fn gaps(&self) -> impl Iterator<Item = &ShareClosureItem> {
        self.panels.iter().filter(|p| p.disposition.is_gap())
    }
}

/// Share `dashboard`'s embedded library panels to `team`, as `principal`, at logical time `now`.
///
/// Gate 1+2 (workspace + `mcp:dashboard.share_closure:call`) run first — a caller without the cap is
/// denied before any read (no existence signal, the S4 ordering rule). The dashboard itself is read
/// under the caller's own gate-3 (`dashboard.get`'s wall): you cannot enumerate the closure of a page
/// you cannot see.
///
/// Per panel, exactly one [`Disposition`] — and a share happens ONLY through [`panel_share`], which
/// re-runs the `mcp:panel.share:call` gate and the owner rule itself. `dry_run=true` mutates nothing.
pub async fn dashboard_share_closure(
    store: &Store,
    principal: &Principal,
    ws: &str,
    dashboard_id: &str,
    team: &str,
    dry_run: bool,
    now: u64,
) -> Result<ShareClosureReport, DashboardError> {
    // Gate 1+2 as the CALLER. Before any read.
    authorize_dashboard(principal, ws, "dashboard.share_closure")?;

    let team_id = normalize_team(team)?;

    // The target team must EXIST in this workspace, checked BEFORE any write. `panel_share` alone
    // would happily `relate()` to a bogus/foreign team: a single call's foot-gun (one dangling edge),
    // but a BULK call would scatter a dangling edge across every owned panel in the closure off one
    // typo. Refuse the whole call — no partial application. This is also the ws-isolation wall for the
    // target: a ws-B team is simply absent here, so it can never become a ws-A panel's audience.
    require_team_exists(store, ws, &team_id).await?;

    // Read the dashboard under the CALLER's gate-3 — the closure of a page you cannot see is not
    // yours to enumerate. (A page the caller can see but does not own is fine: they may still own
    // panels on it; the per-panel owner rule is what actually gates each write.)
    let dashboard = read_dashboard(store, ws, dashboard_id)
        .await?
        .filter(|d| !d.deleted)
        .ok_or(DashboardError::NotFound)?;
    super::visibility::may_read_dashboard(store, principal, ws, &dashboard)
        .await
        .map_err(|_| DashboardError::Denied)?;

    // Does the caller hold `panel.share` at all? Asked ONCE (it is per-caller, not per-panel) via the
    // same `authorize_tool` gate `panel_share` runs first — so the report can never disagree with what
    // the write would do.
    let has_share_cap = authorize_tool(principal, ws, "panel.share").is_ok();

    let mut panels: Vec<ShareClosureItem> = Vec::new();
    for panel_ref in closure_panels(&dashboard) {
        let dep = format!("panel:{}", panel_ref.id);
        if panel_ref.unchecked {
            panels.push(item(
                &dep,
                "",
                &panel_ref.cell,
                Disposition::Unchecked,
                "nested panel — not walked in v1; share it directly if it needs this team",
            ));
            continue;
        }
        let Some(panel) = read_panel(store, ws, &panel_ref.id)
            .await?
            .filter(|p| !p.deleted)
        else {
            // A dangling ref is not a share gap — the page is broken for EVERYONE including the
            // owner, and no share fixes it. Report it honestly rather than silently dropping it.
            panels.push(item(
                &dep,
                "",
                &panel_ref.cell,
                Disposition::Unchecked,
                "panel not found (deleted or never existed) — nothing to share",
            ));
            continue;
        };
        let disposition =
            resolve_disposition(store, principal, ws, &team_id, &panel, has_share_cap).await?;
        panels.push(item(
            &dep,
            &panel.title,
            &panel_ref.cell,
            disposition,
            &reason_for(disposition, &panel, &team_id),
        ));
    }

    // The WRITE pass — only after every disposition is settled, and only when not a dry run. Each
    // share goes through `panel_share` verbatim: it re-checks `mcp:panel.share:call` AND the owner
    // rule, so even if a disposition were computed wrongly above, the wall still holds. That is the
    // "not a grant path" guarantee — this module cannot widen anything `panel.share` would refuse.
    if !dry_run {
        for entry in panels.iter_mut() {
            if entry.disposition != Disposition::WouldShare {
                continue;
            }
            let id = entry.panel.trim_start_matches("panel:").to_string();
            panel_share(
                store,
                principal,
                ws,
                &id,
                PanelVisibility::Team,
                Some(&team_id),
                now,
            )
            .await
            .map_err(|_| DashboardError::Denied)?;
            entry.disposition = Disposition::Shared;
            entry.reason = format!("shared with {team_id}");
        }
    }

    Ok(ShareClosureReport {
        dashboard: dashboard_id.to_string(),
        team: team_id,
        dry_run,
        panels,
    })
}

/// Decide one panel's disposition for `team`, from the caller's standpoint.
///
/// Order matters: "the team can already read it" wins over "you don't own it", because a not-owned
/// panel that is already workspace-visible is NOT a gap the owner needs to close — reporting it as
/// `not_owned` would send the user to ask for something they already have.
async fn resolve_disposition(
    store: &Store,
    principal: &Principal,
    ws: &str,
    team_id: &str,
    panel: &Panel,
    has_share_cap: bool,
) -> Result<Disposition, DashboardError> {
    // Workspace-visible: every member can already read it (gate-3 returns Ok before any team walk).
    if panel.visibility == PanelVisibility::Workspace {
        return Ok(Disposition::AlreadyVisibleWorkspace);
    }
    // Can the TEAM already read it? Asked through the live gate-3 predicate itself (`may_read_panel`)
    // against a synthetic principal carrying the team's identity — the same fn the render path runs,
    // so this can never drift from what the viewer actually experiences.
    if team_can_read(store, ws, team_id, panel).await? {
        return Ok(Disposition::AlreadyShared);
    }
    // A real gap. Is it closable by THIS caller? Ownership first — it is the wall `panel_share`
    // enforces and the one a capability cannot buy your way past.
    if panel.owner != principal.owner_sub() {
        return Ok(Disposition::NotOwned);
    }
    if !has_share_cap {
        return Ok(Disposition::NoShareCap);
    }
    Ok(Disposition::WouldShare)
}

/// Can members of `team_id` read `panel` today **by virtue of the team**? Runs the REAL gate-3
/// [`may_read_panel`] against a synthetic principal standing in for a team member, rather than
/// re-reading the `share` edges here — a second edge walk would be a parallel implementation of the
/// visibility rule (rule 9) and would drift the moment `may_read_panel` learns a new tier.
///
/// **The probe must be a NON-OWNER member.** `may_read_panel` short-circuits `Ok` for the panel's
/// owner before it ever looks at a share edge, so probing as an owner-who-is-also-a-member would
/// answer "yes, readable" for a panel that is shared with nobody — masking a real gap as
/// `already_shared` and silently skipping the share the team actually needs. (Caught by the
/// `page_visibility_gates_the_closure…` test, where the panel's owner is himself in the target team.)
/// We ask as someone for whom the ONLY possible path to `Ok` is the team share.
async fn team_can_read(
    store: &Store,
    ws: &str,
    team_id: &str,
    panel: &Panel,
) -> Result<bool, DashboardError> {
    match team_member_probe(store, ws, team_id, &panel.owner).await? {
        // A non-owner member exists — ask gate-3 as them: the honest live answer, since the only way
        // they can read it is the share we are asking about.
        Some(member) => {
            let p = Principal::routed(member, ws, Vec::new());
            Ok(may_read_panel(store, &p, ws, panel).await.is_ok())
        }
        // No non-owner member to ask (an empty team, or one whose only member owns the panel): no
        // principal exists for whom gate-3 would isolate the share edge. Ask the share-edge question
        // directly — the same edge `may_read_panel`'s team branch consults.
        None => Ok(panel_is_shared_to(store, ws, team_id, panel).await?),
    }
}

/// One member of `team_id` that is NOT `owner` — the probe identity [`team_can_read`] asks gate-3 as.
/// Skipping the owner is what keeps the probe honest (see [`team_can_read`]).
///
/// `team_id` is the BARE id (`normalize_team`), which is what the `member` edge keys on live
/// (`member__ops__user:bob`). The `team:`-prefixed form is tolerated as a fallback because both shapes
/// exist in the wild — the same tolerance `access_check::gate3_identity` applies.
async fn team_member_probe(
    store: &Store,
    ws: &str,
    team_id: &str,
    owner: &str,
) -> Result<Option<String>, DashboardError> {
    for key in [team_id.to_string(), format!("team:{team_id}")] {
        if let Some(m) = lb_assets::list_related(store, ws, "member", &key)
            .await?
            .into_iter()
            .find(|m| m != owner)
        {
            return Ok(Some(m));
        }
    }
    Ok(None)
}

/// Is `panel` `share`d to `team_id`? Used ONLY for the empty-team case, where no member exists to run
/// gate-3 as. Reads the same `share` edge `may_read_panel` reads.
async fn panel_is_shared_to(
    store: &Store,
    ws: &str,
    team_id: &str,
    panel: &Panel,
) -> Result<bool, DashboardError> {
    if panel.visibility != PanelVisibility::Team {
        return Ok(false);
    }
    Ok(lb_assets::list_related(store, ws, "share", &panel.id)
        .await?
        .iter()
        .any(|t| t == team_id))
}

/// Normalize the `team` arg to the **BARE** team id (`ops`) — the identity the S4 edges actually key
/// on. A `team:ops` handle is accepted and unwrapped; anything else (a `user:`/`role:` handle, an
/// empty name) is refused, since the `share` edge only models a team audience.
///
/// **The bare form is not a preference — it is the platform's identity for a team, and getting this
/// wrong silently breaks sharing.** `members.add`/`add_member` write `team -[member]-> user` with the
/// team string **verbatim**, `dashboard.share`/`panel.share` write `asset -[share]-> team` verbatim,
/// and `teams.list` returns `{"team":"ops"}`. Live, the edges read:
///
/// ```text
/// member__ops__user:bob          <- membership, bare
/// share__ops-page__ops           <- the page's share, bare  => bob reads the page
/// share__panel-…__team:ops       <- WRONG: what a `team:`-normalizing verb writes
/// ```
///
/// Gate 3 resolves "teams this asset is shared to" and then "members of THAT team", so a `team:ops`
/// share edge dead-ends: nothing is a member of `team:ops` — the members are under `ops`. The panel
/// stays unreadable while the verb cheerfully reports `shared`. That is precisely the bug this
/// normalization once caused (`debugging/dashboard/share-closure-team-prefix-mismatch.md`): a
/// self-consistent unit test (which used `team:ops` for BOTH the membership and the share) agreed with
/// itself and disagreed with the live system.
///
/// So: unwrap to bare, and let the edge writers stay verbatim as they are for every other verb.
fn normalize_team(team: &str) -> Result<String, DashboardError> {
    let team = team.trim();
    if team.is_empty() {
        return Err(DashboardError::BadInput("empty team".into()));
    }
    if !team.contains(':') {
        return Ok(team.to_string());
    }
    match Subject::parse(team) {
        Some(Subject::Team(name)) => Ok(name),
        _ => Err(DashboardError::BadInput(format!(
            "bad team `{team}` — expected a team, e.g. `ops` or `team:ops`"
        ))),
    }
}

/// The target team must exist in THIS workspace, checked through [`team_list`] — the SAME read
/// `teams.list` serves, whose `kind`-equality filter already excludes tombstoned teams. Asking the
/// existing listing (rather than point-reading the row and re-deriving "is it deleted?") means this
/// check can never disagree with what the platform considers a live team.
///
/// `team_list` is workspace-namespaced, so this doubles as the target's isolation wall: a ws-B team is
/// simply not in ws-A's listing and can never become a ws-A panel's audience.
async fn require_team_exists(store: &Store, ws: &str, team_id: &str) -> Result<(), DashboardError> {
    let exists = team_list(store, ws)
        .await?
        .iter()
        .any(|t| t.team == team_id);
    if !exists {
        return Err(DashboardError::BadInput(format!(
            "team `{team_id}` does not exist in this workspace"
        )));
    }
    Ok(())
}

fn item(
    panel: &str,
    title: &str,
    cell: &str,
    disposition: Disposition,
    reason: &str,
) -> ShareClosureItem {
    ShareClosureItem {
        panel: panel.to_string(),
        title: title.to_string(),
        cell: cell.to_string(),
        disposition,
        reason: reason.to_string(),
    }
}

/// The one-line explanation per disposition. `not_owned` names the owner so the UI can say "ask aidan"
/// — the product answer to the gap this verb deliberately refuses to close.
fn reason_for(disposition: Disposition, panel: &Panel, team_id: &str) -> String {
    match disposition {
        Disposition::WouldShare => format!("you own it — would share with {team_id}"),
        Disposition::Shared => format!("shared with {team_id}"),
        Disposition::AlreadyShared => format!("already shared with {team_id}"),
        Disposition::AlreadyVisibleWorkspace => {
            "already visible to the whole workspace — no share needed".to_string()
        }
        Disposition::NotOwned => format!(
            "owned by {} — only they can share it; ask them, or unlink the panel",
            panel.owner
        ),
        Disposition::NoShareCap => {
            "you own it but lack the panel-share capability — ask an admin".to_string()
        }
        Disposition::Unchecked => "not walked in v1".to_string(),
    }
}

/// The `dashboard.share_closure` descriptor — a real arg schema so a model advertised the verb can
/// FORM the call, and so the plan-then-confirm two-step is legible without reading the source.
pub fn share_closure_descriptor() -> lb_mcp::ToolDescriptor {
    lb_mcp::ToolDescriptor {
        emits_external: false,
        name: "dashboard.share_closure".to_string(),
        title: "Share a dashboard's embedded library panels with a team".to_string(),
        group: "dashboard".to_string(),
        input_schema: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "dashboard": { "type": "string", "x-lb": { "label": "Dashboard id" } },
                "team": { "type": "string", "x-lb": { "label": "Team", "description": "The share target, e.g. 'ops' or 'team:ops' — must exist in this workspace" } },
                "dry_run": { "type": "boolean", "x-lb": { "label": "Dry run", "description": "DEFAULT TRUE — preview only, mutates nothing. Pass false to perform the shares." } },
                "now": { "type": "integer", "x-lb": { "label": "Timestamp", "description": "Logical time of the share — unix epoch seconds" } }
            },
            "required": ["dashboard", "team", "now"]
        })),
        result: None,
    }
}
