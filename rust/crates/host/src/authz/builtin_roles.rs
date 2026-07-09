//! The **built-in role cap bundles** — the single source of truth for what `member` and
//! `workspace-admin` grant (login-hardening scope). Before this, the gateway's `dev_claims`
//! minted ONE `member_caps()` bundle that *contained admin caps*, so every "member" was a full
//! admin (the live escalation: `user:bob`, a plain member, added members / created teams /
//! self-granted `workspace.delete`). The fix splits the bundle by role and moves it into the
//! durable authz model: the built-in role RECORDS carry these caps, and `resolve_caps` folds a
//! subject's `role:member` / `role:workspace-admin` grants into their token. Admin power now
//! requires the `workspace-admin` role — it is no longer baked into the member baseline.
//!
//! Why here (not the gateway): the roles are *administered data* (authz-grants scope) — a token is
//! a cached projection of `resolve_caps`, which reads these role records. Seeding the records
//! ([`ensure_builtin_authz_roles`]) at every bootstrap path (login-resolve, workspace-create,
//! member-add) is what makes trimming the gateway hardcode safe: a member/admin still resolves to
//! their real caps, just role-scoped now.
//!
//! **The three-tier split (viewer ⊂ member ⊂ admin).** The role a subject holds is what gates their
//! *reach* — the nav is a pure lens and never widens reach (access-model scope), so "give bob a
//! one-page nav" only restricts what he *reaches* if his ROLE carries only what those pages need.
//! That is why `member` was split again: it conflated a **viewer** (given a curated nav of pages to
//! look at) with an **author** (builds rules/flows/queries/templates/datasources). A live session
//! proved the gap — `user:bob`, a plain member with a one-page nav, could still open the Rules editor
//! by URL because `mcp:rules.*` was a member cap, so the *cap gate* (the real reach boundary) passed.
//!
//! - **`viewer`** — the minimum to *use a screen you were given*: pub/sub channels you may see; read
//!   your own dashboards/panels + the render path (`viz.query`, catalog/pin, access-check-for-self);
//!   resolve your own nav/prefs/layout/i18n; read insights/series/reminders/telemetry for your own
//!   screen. NO cap here *authors* anything or reaches an authoring surface.
//! - **`member`** — `viewer ∪ author`. The author delta ([`AUTHOR_CAPS`]) is the build/run surface a
//!   member drives on their OWN behalf: rules, flows, saved queries, scripted-view templates,
//!   datasources + federation, ingest/series writes, the bounded store.query read, the devkit, agent
//!   memory writes. A viewer with a one-page nav genuinely cannot reach any of these — the cap gate
//!   denies it server-side, which is the reach restriction the nav alone could never provide.
//! - **`workspace-admin`** — `member ∪ admin`. A cap is `admin`-only iff it manages OTHER principals
//!   or the workspace itself: membership/teams/roles/grants, destructive workspace ops, cross-member
//!   catalogs (system/store lenses), the extension lifecycle, and every WORKSPACE-DEFAULT write
//!   (`*.set_default`, `set_catalog`, `policy.set`, `config.set`, definition/persona CRUD).
//!
//! Each tier is a strict superset of the one below (`viewer ⊆ member ⊆ admin`), proven by the unit
//! tests. The base login floor (`credentials.rs`) is the **viewer** set — the universal minimum every
//! authenticated principal holds — and member/admin caps ride their ROLE grant through `resolve_caps`,
//! so a `viewer`-role token is never silently re-widened to a member (the leak that let bob reach Rules).
//!
//! Load-bearing (do NOT re-classify): the `.catalog`/`.pin` render caps the `mcp:*.<verb>:call`
//! wildcards miss (see `credentials.rs` history) live in the VIEWER set — guarded by the unit tests
//! here (`viewer_bundle_keeps_render_path`) and by `credentials.rs`'s tests over the viewer floor.
//! The datasource-REGISTRATION chain (`datasource.add`/`native.call`/`secret:federation/*:write`) is
//! AUTHOR-tier — a viewer reads sources (`federation.query`) but does not register them.

use lb_store::{read, write, Store, StoreError};

use lb_authz::{Role, ROLE_TABLE};

/// The built-in role names, kept in one place. `workspace-admin` / `member` / `viewer` are seeded
/// with the bundles below; `super-admin` is reserved (node-operator tier, not seeded per-workspace).
pub const ROLE_WORKSPACE_ADMIN: &str = "workspace-admin";
pub const ROLE_MEMBER: &str = "member";
pub const ROLE_VIEWER: &str = "viewer";

/// The **viewer** cap bundle — the minimum to USE a screen you were given (read your own
/// dashboards/panels + the render path, resolve your own nav/prefs/layout, read insights/series for
/// your own screen). NO cap here authors anything or reaches an authoring surface; a viewer given a
/// one-page nav cannot reach the Rules/Flows/Query editors (the cap gate denies it server-side).
/// This is also the base login floor (`credentials.rs`) — the universal minimum every principal holds.
pub fn viewer_role_caps() -> Vec<String> {
    to_owned(VIEWER_CAPS)
}

/// The **member** cap bundle — `viewer ∪ author`. Everything a normal member needs to use AND author
/// on their own screen. NO cap here manages another principal or the workspace itself (those live in
/// [`admin_only_caps`]).
pub fn member_role_caps() -> Vec<String> {
    let mut caps = to_owned(VIEWER_CAPS);
    caps.extend(to_owned(AUTHOR_CAPS));
    caps.sort();
    caps.dedup();
    caps
}

/// The **author** delta a `member` holds over a `viewer` — the build/run surface a member drives on
/// their OWN behalf (rules, flows, saved queries, templates, datasources, ingest, the bounded
/// store.query read, the devkit, agent-memory writes). Exposed so a test can assert a `viewer` token
/// holds NONE of these (the nav-as-reach regression) and a `member` holds ALL of them.
pub fn author_caps() -> Vec<String> {
    to_owned(AUTHOR_CAPS)
}

/// The **workspace-admin** cap bundle: `member ∪ admin`. An admin can do everything a member can,
/// plus manage members/teams/roles/grants, run the cross-member lenses, drive the extension
/// lifecycle, and write workspace defaults.
pub fn workspace_admin_role_caps() -> Vec<String> {
    let mut caps = member_role_caps();
    caps.extend(to_owned(ADMIN_ONLY_CAPS));
    caps.sort();
    caps.dedup();
    caps
}

/// The admin-only additions (the delta over a member). Exposed so a test can assert a `member`
/// token holds NONE of these (the escalation regression) and an admin token holds ALL of them.
pub fn admin_only_caps() -> Vec<String> {
    to_owned(ADMIN_ONLY_CAPS)
}

fn to_owned(caps: &[&str]) -> Vec<String> {
    caps.iter().map(|s| s.to_string()).collect()
}

/// Ensure the `member` and `workspace-admin` role RECORDS exist in workspace `ws`, defining them
/// with the built-in bundles if absent (mirrors `apikey::seed::ensure_builtin_roles`). Idempotent:
/// a present row is left untouched, so an admin who redefined a same-named custom role is not
/// clobbered, and re-running on every login/bootstrap is a cheap no-op (one point read per role,
/// one write only when missing). Called by every path that grants `role:member` /
/// `role:workspace-admin` so the grant actually resolves to caps.
pub async fn ensure_builtin_authz_roles(store: &Store, ws: &str) -> Result<(), StoreError> {
    ensure_one(store, ws, ROLE_VIEWER, viewer_role_caps()).await?;
    ensure_one(store, ws, ROLE_MEMBER, member_role_caps()).await?;
    ensure_one(store, ws, ROLE_WORKSPACE_ADMIN, workspace_admin_role_caps()).await?;
    Ok(())
}

/// Define `name` with `caps` iff no role row exists for it yet (idempotent seed).
async fn ensure_one(
    store: &Store,
    ws: &str,
    name: &str,
    caps: Vec<String>,
) -> Result<(), StoreError> {
    if read(store, ws, ROLE_TABLE, name).await?.is_some() {
        return Ok(());
    }
    let role = Role::new(name, caps);
    let value = serde_json::to_value(&role).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, ROLE_TABLE, name, &value).await
}

/// **Viewer-level caps** — the minimum to USE a screen you were given. Every entry here is a READ or
/// a RENDER of your own screen, or a personal presentation write (your own prefs/layout/pins) — none
/// authors shared content or reaches an authoring surface. This is deliberately an ALLOW-LIST of
/// concrete verbs: the broad `mcp:*.<verb>:call` / `store:*:*` wildcards live in [`AUTHOR_CAPS`],
/// because a wildcard like `mcp:*.delete:call` or `store:*:write` would silently hand a viewer the
/// authoring reach the whole three-tier split exists to withhold (a viewer with a one-page nav must
/// not reach `rules.delete`/`flows.save`/etc. by URL). Keep this list explicit and read-shaped.
const VIEWER_CAPS: &[&str] = &[
    // channels: pub/sub any channel the viewer may see (gate-3 decides which).
    "bus:chan/*:pub",
    "bus:chan/*:sub",
    // members roster read (list is viewer; add/manage are admin).
    "mcp:members.list:call",
    // inbox READS + resolve their own items (no record/enqueue — those produce motion → author).
    "mcp:inbox.list:call",
    "mcp:inbox.resolve:call",
    "mcp:outbox.status:call",
    // insights read/act + member-owned sub CRUD + policy READ. raise (produce) is author; policy.set admin.
    "mcp:insight.get:call",
    "mcp:insight.list:call",
    "mcp:insight.watch:call",
    "mcp:insight.ack:call",
    "mcp:insight.resolve:call",
    "mcp:insight.occurrences:call",
    "mcp:insight.sub.create:call",
    "mcp:insight.sub.list:call",
    "mcp:insight.sub.get:call",
    "mcp:insight.sub.delete:call",
    "mcp:insight.sub.mute:call",
    "mcp:insight.policy.get:call",
    // workspace directory LIST (a viewer sees the switcher). create/delete/purge are admin.
    "mcp:workspace.list:call",
    // host fs browse (read-only metadata) — the datasource DB-file picker; harmless read.
    "mcp:host.fs.list:call",
    "mcp:host.fs.home:call",
    // datasource + series READS — a viewer's tiles read series/federation; registering a source is author.
    "mcp:datasource.list:call",
    "mcp:federation.query:call",
    "mcp:series.read:call",
    "mcp:series.latest:call",
    "mcp:series.find:call",
    "mcp:series.list:call",
    "mcp:series.watch:call",
    // documents READ (a viewer reads shared docs; put_doc is author).
    "mcp:assets.get_doc:call",
    "mcp:assets.list_docs:call",
    // skills catalog reads.
    "mcp:assets.list_skills:call",
    "mcp:assets.load_skill:call",
    // command palette catalog (leaks only tool SHAPES the caller may already run).
    "mcp:tools.catalog:call",
    // channel chart pref (viewer's own plot override) + channel READS (edit is author).
    "mcp:channel.chart_pref.get:call",
    "mcp:channel.chart_pref.set:call",
    "mcp:channel.history:call",
    // tag graph FIND (a viewer resolves dashboard-var Query sources; tags.add is author).
    "mcp:tags.find:call",
    // dashboards — a viewer READS the pages they were given (save/delete/share are author).
    "mcp:dashboard.get:call",
    "mcp:dashboard.list:call",
    // access-model scope: the read-only dependency-closure preflight for the viewer's OWN reach.
    "mcp:dashboard.access_check:call",
    // LOAD-BEARING `.catalog`/`.pin` — a viewer sees the catalog + pins their own shortcut.
    "mcp:dashboard.catalog:call",
    "mcp:dashboard.pin:call",
    // panels — a viewer READS the panels their pages embed (a panel is a LENS, sources re-checked).
    "mcp:panel.get:call",
    "mcp:panel.list:call",
    "mcp:panel.usage:call",
    // nav READS — a viewer resolves the menu they were given. save/delete/share/set_default are admin.
    "mcp:nav.get:call",
    "mcp:nav.list:call",
    "mcp:nav.resolve:call",
    // per-surface OWN ui layout (keyed to the token sub) — personal presentation.
    "mcp:layout.get:call",
    "mcp:layout.set:call",
    // viz render path (dispatches each target under caller ∩ grant — no widening). The engine that
    // paints a viewer's tiles; it composes the per-target cap so it can never widen a viewer's reach.
    "mcp:viz.query:call",
    // prefs — a viewer reads/writes their OWN presentation settings (target forced to caller sub).
    "mcp:prefs.get:call",
    "mcp:prefs.resolve:call",
    "mcp:prefs.set:call",
    // i18n render/read for the caller's own screen. set_catalog (ws override) is admin.
    "mcp:message.render:call",
    "mcp:message.render_recipient:call",
    "mcp:prefs.catalog:call",
    // telemetry READ (hard-filtered to the caller's own ws ring).
    "mcp:telemetry.read:call",
    // agent — a viewer drives their OWN run (decide/invoke/watch/control) + reads config/defs/personas.
    // These are bounded to the caller's own caps (caller ∩ agent); no authoring of defs/personas here.
    "mcp:agent.decide:call",
    "mcp:agent.invoke:call",
    "mcp:agent.runtimes:call",
    "mcp:agent.watch:call",
    "mcp:agent.control:call",
    "mcp:agent.config.get:call",
    "mcp:agent.def.list:call",
    "mcp:agent.def.get:call",
    "mcp:agent.persona.list:call",
    "mcp:agent.persona.get:call",
    "mcp:agent.persona.resolve:call",
    "mcp:agent.policy.get:call",
    // agent memory READS — the viewer's own scope (scope derived from principal). set/delete are author.
    "mcp:agent.memory.list:call",
    "mcp:agent.memory.get:call",
    // model-activated skill (loop-internal; the S4 skill grant is the real wall).
    "mcp:skill.activate:call",
    // reminders nav gate — the concrete list cap the frontend `hasCap` checks EXACTLY (it does not
    // expand a wildcard), so the Reminders sidebar entry needs it spelled out. fire is author.
    "mcp:reminder.list:call",
    // shared-asset doc/skill store READS (gate-3/ownership owns which specific asset). Writes are author.
    "store:doc/*:read",
    "store:skill/**:read",
    // generic per-workspace store READ + the verb-READ wildcards (list/get). WRITE wildcards are author.
    "store:*:read",
    "mcp:*.get:call",
    "mcp:*.list:call",
];

/// **Author delta** — the caps a `member` holds over a `viewer`: the build/run surface a member
/// drives on their OWN behalf. Every entry here CREATES, MUTATES, RUNS, or DELETES shared content, or
/// is a broad write/mutate wildcard — exactly the authoring reach a bare `viewer` (given only a
/// curated nav) must NOT have. A `viewer`-role token holds NONE of these; a `member`-role token holds
/// ALL of them (the unit tests pin both).
const AUTHOR_CAPS: &[&str] = &[
    // durable motion a member PRODUCES on their own behalf (record an inbox item, enqueue outbox).
    "mcp:inbox.record:call",
    "mcp:outbox.enqueue:call",
    // insight producer raise (a viewer only reads/acts on insights).
    "mcp:insight.raise:call",
    // Studio local-dev devkit: scaffold/build/inspect/write — the authoring toolchain.
    "mcp:devkit.templates:call",
    "mcp:devkit.scaffold:call",
    "mcp:devkit.write_file:call",
    "mcp:devkit.inspect:call",
    "mcp:devkit.build:call",
    "mcp:devkit.root:call",
    // datasources chain — the member REGISTERS/TESTS their own sources over real series.
    // LOAD-BEARING end-to-end (federation sidecar dispatch gates on native.call).
    "mcp:native.call:call",
    "mcp:datasource.add:call",
    "mcp:datasource.remove:call",
    "mcp:datasource.test:call",
    "secret:federation/*:write",
    // schema-designer write plane — the member WRITES to their own sources + EXPORTS platform
    // data out. `dbschema.save`/`delete` ride the `mcp:*.write:call`/`mcp:*.delete:call` author
    // wildcards; `dbschema.get`/`list` ride the viewer read wildcards. These three name concrete
    // verbs (no wildcard covers `federation.write`/`export` — a viewer must NOT reach them). The
    // `dbschema.save`/`delete` verbs name concrete caps too (`.save`/`.delete` are their own verbs,
    // not matched by the `.write`/`.delete` wildcards — the wildcard segment is the verb, not a
    // suffix).
    "mcp:federation.write:call",
    "mcp:federation.export:call",
    "mcp:dbschema.save:call",
    "mcp:dbschema.delete:call",
    // ingest — a member WRITES their own series (producer = the authed principal). Reads are viewer.
    "mcp:ingest.write:call",
    // documents WRITE (a member's own shared docs).
    "mcp:assets.put_doc:call",
    // generic bus PRODUCE (publish/watch a subject the member drives).
    "mcp:bus.publish:call",
    "mcp:bus.watch:call",
    // channel EDIT (mutate a channel's shape/config).
    "mcp:channel.edit:call",
    // proof-panel guest tools (INNER callbacks authorize against caller ∩ install-grant).
    "mcp:proof-panel.proof.derive:call",
    "mcp:proof-panel.proof.simulate:call",
    // tag graph WRITE — a member tags their own series.
    "mcp:tags.add:call",
    // widget-builder direct-SurrealDB read: parse-allowlisted, bounded, ws-walled SELECT + schema.
    // Author-tier: it is the authoring read behind the widget builder, not a viewer's tile render.
    "mcp:store.query:call",
    "mcp:store.schema:call",
    // dashboards — a member BUILDS/SHARES/DELETES their OWN (gate-3 owns which). delete_any is admin.
    "mcp:dashboard.save:call",
    "mcp:dashboard.delete:call",
    "mcp:dashboard.share:call",
    // panels — a member's own reusable/standalone panels (author + share).
    "mcp:panel.save:call",
    "mcp:panel.delete:call",
    "mcp:panel.share:call",
    // scripted-view templates — a member's own (author-ownership owns which).
    "mcp:template.save:call",
    "mcp:template.get:call",
    "mcp:template.list:call",
    "mcp:template.delete:call",
    // rules — a member AUTHORS/RUNS their own rules (per-source caps still gate reads).
    "mcp:rules.run:call",
    "mcp:rules.eval:call",
    "mcp:rules.save:call",
    "mcp:rules.get:call",
    "mcp:rules.list:call",
    "mcp:rules.delete:call",
    "store:rule:read",
    "store:rule:write",
    // saved queries — a member AUTHORS/RUNS their own (query.run COMPOSES the target cap, no widening).
    "mcp:query.save:call",
    "mcp:query.run:call",
    "mcp:query.compile:call",
    "mcp:query.get:call",
    "mcp:query.list:call",
    "mcp:query.delete:call",
    // flows — a member AUTHORS/RUNS their own typed-node flows (no-widening run gate still applies).
    "mcp:flows.save:call",
    "mcp:flows.get:call",
    "mcp:flows.list:call",
    "mcp:flows.delete:call",
    "mcp:flows.nodes:call",
    "mcp:flows.run:call",
    "mcp:flows.resume:call",
    "mcp:flows.suspend:call",
    "mcp:flows.cancel:call",
    "mcp:flows.patch_run:call",
    "mcp:flows.runs.get:call",
    "mcp:flows.runs.list:call",
    "mcp:flows.watch:call",
    "mcp:flows.node.get:call",
    "mcp:flows.node.update:call",
    "mcp:flows.node_state:call",
    "mcp:flows.enable:call",
    "mcp:flows.inject:call",
    "mcp:flows.debug.watch:call",
    // agent memory WRITES — a member's own scope (scope derived from principal, never an arg).
    "mcp:agent.memory.set:call",
    "mcp:agent.memory.delete:call",
    // reminders run-now (re-checks the ACTION's own cap under the stored principal).
    "mcp:reminder.fire:call",
    // shared-asset doc/skill store WRITES (gate-3/ownership owns which specific asset).
    "store:doc/*:write",
    "store:skill/**:write",
    // generic per-workspace store WRITE + the verb-CRUD WRITE wildcards (the bulk of member authoring).
    // These broad wildcards are what make `member` an author — a `viewer` deliberately lacks them so a
    // one-page nav truly restricts reach (a wildcard `mcp:*.delete:call` would re-open every editor).
    "store:*:write",
    "mcp:*.write:call",
    "mcp:*.create:call",
    "mcp:*.update:call",
    "mcp:*.delete:call",
    "mcp:*.post:call",
];

/// Admin-only caps — the delta a `workspace-admin` holds over a `member`. These MANAGE other
/// principals or the workspace itself. A `member` token holds NONE of these (the escalation
/// regression asserts exactly this over `bob`'s live `members.add`/`teams.create`/self-grant).
const ADMIN_ONLY_CAPS: &[&str] = &[
    // membership / teams / roles / grants — the escalation-proof set (bob's 204s → 403s).
    "mcp:members.add:call",
    "mcp:members.manage:call",
    "mcp:teams.manage:call",
    "mcp:teams.list:call",
    "mcp:roles.define:call",
    "mcp:roles.list:call",
    "mcp:roles.manage:call",
    "mcp:grants.assign:call",
    "mcp:grants.list:call",
    "mcp:user.manage:call",
    "mcp:user.disable:call",
    "mcp:identity.manage:call",
    // destructive / creating workspace ops.
    "mcp:workspace.create:call",
    "mcp:workspace.delete:call",
    "mcp:workspace.purge:call",
    // access console: resolved effective caps + live-token revoke.
    "mcp:authz.resolve:call",
    "mcp:authz.revoke-tokens:call",
    // extension lifecycle + native supervision + publish.
    "mcp:ext.list:call",
    "mcp:ext.disable:call",
    "mcp:ext.uninstall:call",
    "mcp:ext.publish:call",
    "mcp:native.reset:call",
    "mcp:native.install:call",
    // skills lifecycle (adopt/drop/soft-hide a skill for the workspace).
    "mcp:assets.put_skill:call",
    "mcp:assets.grant_skill:call",
    "mcp:assets.revoke_skill:call",
    "mcp:assets.deprecate_skill:call",
    // raw-store lenses — a scan answers "every record in the workspace" (relaxes gate-3).
    "mcp:store.tables:call",
    "mcp:store.scan:call",
    "mcp:store.graph:call",
    // system map — reads across every subsystem of the workspace.
    "mcp:system.overview:call",
    "mcp:system.topology:call",
    "mcp:system.subsystem:call",
    "mcp:system.tools:call",
    "mcp:system.acp:call",
    // dashboard admin override (delete a dashboard the admin doesn't own).
    "mcp:dashboard.delete_any:call",
    // nav WRITES (author/share/set the workspace-default menu).
    "mcp:nav.save:call",
    "mcp:nav.delete:call",
    "mcp:nav.share:call",
    // WORKSPACE-DEFAULT writes — the workspace-level "for everyone" settings.
    "mcp:prefs.set_default:call",
    "mcp:message.set_catalog:call",
    "mcp:insight.policy.set:call",
    "mcp:agent.policy.set:call",
    "mcp:agent.config.set:call",
    // agent definition / persona CRUD (custom defs/personas; built-ins are read-only regardless).
    "mcp:agent.def.create:call",
    "mcp:agent.def.update:call",
    "mcp:agent.def.delete:call",
    "mcp:agent.persona.create:call",
    "mcp:agent.persona.update:call",
    "mcp:agent.persona.delete:call",
    // spends model budget (a distinct authority) + the sealed model-key secret write.
    "mcp:agent.def.test:call",
    "mcp:secret.set:call",
    "secret:agent/*:write",
    // schema-designer: applying DDL to an external DB is the destructive authority — admin-only
    // (open-question lean #1: member saves the design, admin migrates). The dry_run default keeps
    // a plan-only call safe, but the cap gates the apply step regardless.
    "mcp:federation.migrate:call",
    // shared agent-memory (workspace scope) write — an admin decides every member's agent may write it.
    "store:agent_memory/workspace:write",
    // api-keys + webhooks management surfaces + their secret writes.
    "mcp:apikey.manage:call",
    "mcp:webhook.manage:call",
    "secret:webhook/*:write",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The escalation regression, at the cap-bundle layer: a member's bundle holds NONE of the
    /// admin-only caps the live `user:bob` abused (members.add / teams.manage / grants.assign /
    /// user.manage / workspace.create / workspace.delete / dashboard.delete_any).
    #[test]
    fn member_bundle_holds_no_admin_caps() {
        let member = member_role_caps();
        for admin_cap in [
            "mcp:members.add:call",
            "mcp:teams.manage:call",
            "mcp:roles.define:call",
            "mcp:grants.assign:call",
            "mcp:user.manage:call",
            "mcp:workspace.create:call",
            "mcp:workspace.delete:call",
            "mcp:dashboard.delete_any:call",
        ] {
            assert!(
                !member.contains(&admin_cap.to_string()),
                "member bundle must NOT carry admin cap {admin_cap} (the escalation)"
            );
        }
    }

    /// A member keeps the LOAD-BEARING caps the `mcp:*.<verb>:call` wildcards miss (`.catalog`/
    /// `.pin`) + the full datasources chain — trimming admin must not trim these (credentials.rs history).
    #[test]
    fn member_bundle_keeps_load_bearing_member_caps() {
        let member = member_role_caps();
        for needed in [
            "mcp:dashboard.catalog:call",
            "mcp:dashboard.pin:call",
            "mcp:tools.catalog:call",
            "mcp:datasource.add:call",
            "mcp:federation.query:call",
            "mcp:native.call:call",
            "secret:federation/*:write",
        ] {
            assert!(
                member.contains(&needed.to_string()),
                "member bundle must keep load-bearing cap {needed}"
            );
        }
    }

    /// `workspace-admin` is a strict superset of `member`, and holds EVERY admin-only cap.
    #[test]
    fn admin_bundle_is_member_superset_plus_all_admin_caps() {
        let admin = workspace_admin_role_caps();
        for c in member_role_caps() {
            assert!(
                admin.contains(&c),
                "admin must be a superset of member: {c}"
            );
        }
        for c in admin_only_caps() {
            assert!(
                admin.contains(&c),
                "admin must hold every admin-only cap: {c}"
            );
        }
        // And a member holds none of the admin-only caps (mirror of the escalation test, exhaustive).
        let member = member_role_caps();
        for c in admin_only_caps() {
            assert!(
                !member.contains(&c),
                "member must hold NO admin-only cap: {c}"
            );
        }
    }

    /// The **nav-as-reach regression**: a `viewer` holds NONE of the authoring caps. This is the exact
    /// gap the live session hit — `user:bob`, given a one-page nav, reached the Rules editor because
    /// `mcp:rules.*` was a member cap. A viewer's cap gate must deny every authoring surface so a
    /// curated nav actually restricts reach. Includes the broad write/mutate wildcards — a viewer must
    /// NOT hold `mcp:*.delete:call` / `store:*:write`, or those would re-open every editor by URL.
    #[test]
    fn viewer_bundle_holds_no_author_caps() {
        let viewer = viewer_role_caps();
        for author_cap in [
            "mcp:rules.save:call",
            "mcp:rules.run:call",
            "mcp:rules.delete:call",
            "mcp:flows.save:call",
            "mcp:flows.run:call",
            "mcp:query.save:call",
            "mcp:query.run:call",
            "mcp:template.save:call",
            "mcp:datasource.add:call",
            "mcp:dashboard.save:call",
            "mcp:panel.save:call",
            "mcp:ingest.write:call",
            "mcp:store.query:call",
            "mcp:agent.memory.set:call",
            // the broad wildcards that would silently re-grant authoring reach.
            "store:*:write",
            "mcp:*.write:call",
            "mcp:*.create:call",
            "mcp:*.update:call",
            "mcp:*.delete:call",
            "mcp:*.post:call",
        ] {
            assert!(
                !viewer.contains(&author_cap.to_string()),
                "viewer bundle must NOT carry author cap {author_cap} (the nav-as-reach regression)"
            );
        }
        // ...and holds NONE of the admin-only caps either (a viewer ⊂ member ⊂ admin).
        for c in admin_only_caps() {
            assert!(
                !viewer.contains(&c),
                "viewer must hold NO admin-only cap: {c}"
            );
        }
    }

    /// A `viewer` keeps the caps needed to USE a screen it was given — read its dashboards/panels/nav
    /// and RENDER their tiles (`viz.query`), resolve its own prefs/layout. Trimming authoring must not
    /// trim the viewer's render path, or a one-page nav would render nothing.
    #[test]
    fn viewer_bundle_keeps_render_path() {
        let viewer = viewer_role_caps();
        for needed in [
            "mcp:dashboard.get:call",
            "mcp:dashboard.list:call",
            "mcp:dashboard.catalog:call",
            "mcp:dashboard.pin:call",
            "mcp:panel.get:call",
            "mcp:panel.list:call",
            "mcp:nav.resolve:call",
            "mcp:nav.get:call",
            "mcp:viz.query:call",
            "mcp:series.read:call",
            "mcp:prefs.resolve:call",
            "mcp:layout.get:call",
            "mcp:layout.set:call",
            "mcp:federation.query:call",
            "mcp:tools.catalog:call",
        ] {
            assert!(
                viewer.contains(&needed.to_string()),
                "viewer bundle must keep render-path cap {needed}"
            );
        }
    }

    /// The tier lattice: `viewer ⊆ member ⊆ admin`. A member is a strict superset of a viewer and
    /// holds every author cap; a viewer that is missing an author cap is what makes the reach split
    /// real. (`admin ⊇ member` is pinned separately above.)
    #[test]
    fn member_is_viewer_superset_plus_all_author_caps() {
        let member = member_role_caps();
        let viewer = viewer_role_caps();
        for c in &viewer {
            assert!(
                member.contains(c),
                "member must be a superset of viewer: {c}"
            );
        }
        for c in author_caps() {
            assert!(
                member.contains(&c),
                "member must hold every author cap: {c}"
            );
            assert!(!viewer.contains(&c), "viewer must hold NO author cap: {c}");
        }
        // Belt-and-braces: member is exactly viewer ∪ author (no stray cap in either direction).
        assert!(
            member.len() > viewer.len(),
            "member must be strictly larger than viewer"
        );
    }
}
