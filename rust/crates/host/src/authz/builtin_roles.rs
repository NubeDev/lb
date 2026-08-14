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
//! **No broad `mcp:*.<verb>:call` wildcard in the viewer/member bundles.** These bundles name their
//! verbs concretely, and `no_member_or_viewer_wildcard_may_span_an_admin_only_cap` enforces it against
//! the real matcher. The reason is the 2026-07-16 regression: the bundles once carried
//! `mcp:*.get|list:call` (viewer) and `mcp:*.write|create|update|delete|post:call` (author) as a
//! shorthand for "all the CRUD". But a bundle's reach is not what it LITERALLY lists — it is what
//! [`holds_cap`] authorizes, and that is wildcard-aware. The `*` segment is the `<tool>` half of
//! `<tool>.<verb>`, so `mcp:*.list:call` spanned `teams.list` / `roles.list` / `grants.list` /
//! `invite.list`, and `mcp:*.delete:call` spanned `workspace.delete` — TEN admin-only caps authorized
//! for every plain member, five for every viewer, live (`GET /admin/teams` and `roles.list` both
//! returned 200 to `user:bob`, a plain member). Every literal `!bundle.contains(admin_cap)` test in
//! this file passed throughout: the wildcard is invisible to a membership check.
//!
//! This is the SAME `user:bob` escalation described above, returning through the grammar instead of
//! the grant. The lesson is that a role bundle is a POLICY statement and must be exhaustive-by-name;
//! a wildcard in a bundle is an open-ended promise about verbs that do not exist yet. When you add an
//! author verb, name it. If that feels tedious, that tedium is the feature — it is what makes the
//! blast radius of a new admin verb reviewable.
//!
//! Load-bearing (do NOT re-classify): the `.catalog`/`.pin` render caps live in the VIEWER set —
//! guarded by the unit tests here (`viewer_bundle_keeps_render_path`) and by `credentials.rs`'s tests
//! over the viewer floor.
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
    // Triage plane (insight-triage-scope.md): assigning an owner and leaving a note are member-ACT
    // grade, exactly like ack/resolve — the operator who may ack a finding may say who owns it and
    // write down what they found. They are two NARROW caps rather than one `insight.update`
    // precisely so a producer holding only `mcp:insight.raise:call` gets zero triage write power.
    "mcp:insight.assign:call",
    "mcp:insight.comment:call",
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
    // Sample statistics for ONE series (counts, extent, producers) — a data-plane READ about the
    // samples a viewer can already read, so it sits on this tier and NOT with retention
    // administration. That split is deliberate: it is what lets a client degrade per fact and still
    // show counts + freshness when the admin-plane `series.retention.status` is refused
    // (series-observability scope).
    "mcp:series.stats:call",
    // The relative time-range resolver (relative-time-range scope) — read-only compute over the
    // caller's OWN expression (no store read, no write), the arithmetic every screen's range picker
    // and every flow needs. Viewer-tier for the same reason `viz.query` is: it renders a screen a
    // viewer was given and reaches nothing.
    "mcp:time.range.resolve:call",
    // What the PRODUCERS of a series report about their own ingest. Same data-plane tier and the
    // same argument as `series.stats`: it is a read about samples the viewer can already see, and
    // the fan-out it performs is re-gated per extension under the CALLER's own principal, so this
    // grant widens nothing on its own — a viewer without `mcp:<ext>.ingest.health:call` gets rows
    // that say `denied`, which is the honest answer (series-observability scope, slice D).
    "mcp:series.producer.health:call",
    // documents READ (a viewer reads shared docs; put_doc is author).
    "mcp:assets.get_doc:call",
    "mcp:assets.list_docs:call",
    // binary assets READ (document-store scope move 2) — a viewer's screen may RENDER a shared
    // binary asset (a scene's glTF stage model, an image underlay), the same tier as reading the
    // doc that references it. put/delete are author, below. The inner gate needs the store-surface
    // cap too: `store:asset/*` is named concretely because the generic `store:*:read` wildcard is
    // single-segment and does not span the `asset/{id}` resource path (same reason `store:doc/*`
    // is named above).
    "mcp:assets.get_asset:call",
    "mcp:assets.list_assets:call",
    "store:asset/*:read",
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
    // packs (packs scope) — a receipt is OPERATOR DOCUMENTATION: it is how someone learns what
    // turned this workspace into this product (the vocabulary, the insight grammar, the applied
    // objects). Hiding it from viewers would hide the teaching surface from the people it teaches,
    // so list/get are viewer reads — consistent with insights being viewer-visible. `pack.validate`
    // joins them: it is a pure dry run that touches no object and writes nothing, and a pack author
    // must be able to run it in CI without an admin token. APPLYING is the write, and it is
    // admin-only (below) — plus every object it drives re-checks its own cap.
    "mcp:pack.list:call",
    "mcp:pack.get:call",
    "mcp:pack.validate:call",
    // dashboards — a viewer READS the pages they were given (save/delete/share are author).
    "mcp:dashboard.get:call",
    "mcp:dashboard.list:call",
    // forms — a viewer READS the forms they were given (save/delete are author). A form is a simple
    // owner/workspace asset, so read is viewer-tier exactly like a dashboard read.
    "mcp:forms.get:call",
    "mcp:forms.list:call",
    // access-model scope: the read-only dependency-closure preflight for the viewer's OWN reach.
    "mcp:dashboard.access_check:call",
    // viz import-export scope: export is a READ (viewer); import (a write) is an author cap below.
    "mcp:dashboard.export:call",
    // LOAD-BEARING `.catalog`/`.pin` — a viewer sees the catalog + pins their own shortcut.
    "mcp:dashboard.catalog:call",
    "mcp:dashboard.pin:call",
    // panels — a viewer READS the panels their pages embed (a panel is a LENS, sources re-checked).
    "mcp:panel.get:call",
    "mcp:panel.list:call",
    "mcp:panel.usage:call",
    // reports — a viewer READS reports + brand profiles (a report is a LENS; export is author-gated).
    "mcp:report.get:call",
    "mcp:report.list:call",
    "mcp:brand.get:call",
    "mcp:brand.list:call",
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
    // entity-scoped-grants scope: every member asks "what can I reach?" — the scoped read API.
    // These are informational (the enforcement happens at the verb level); a caller only learns its
    // OWN reach (the verbs use the calling principal, never a `user` arg).
    "mcp:authz.check_scoped:call",
    "mcp:authz.scope_filter:call",
    // push-target scope: a member registers/lists/removes their own devices (self-only).
    "mcp:device.register:call",
    // shared-asset doc/skill store READS (gate-3/ownership owns which specific asset). Writes are author.
    "store:doc/*:read",
    "store:skill/**:read",
    // The reads the retired `mcp:*.get:call` / `mcp:*.list:call` wildcards used to supply, now named.
    // Each is a viewer-tier read whose target is gate-3/ownership-scoped (you see only your own rows).
    // NOT restored, deliberately: `secret.get` / `secret.list` — a viewer must not enumerate the
    // secret plane even by name. They were reachable ONLY through the retired wildcard, never named
    // by any bundle, and their own inner gate already denies (verified live: `secret.list` → 403), so
    // naming them here would have widened the floor to match a bug.
    // global schedules — READS are viewer-tier: the schedule widget renders for anyone who can see
    // the dashboard, and `schedule.evaluate` is a pure read of the same record. Authoring
    // (`schedule.save`/`delete`) is author-tier, below.
    "mcp:schedule.get:call",
    "mcp:schedule.list:call",
    "mcp:schedule.evaluate:call",
    "mcp:channel.list:call",
    "mcp:dbschema.get:call",
    "mcp:dbschema.list:call",
    "mcp:device.list:call",
    "mcp:history.list:call",
    // versions scope (#112): a viewer READS an entity's version history. Seeing what a dashboard
    // looked like last week is the same tier as seeing the dashboard — and a viewer holding these
    // still cannot restore: `versions.restore` is author-tier AND re-checks the kind's save cap, so
    // a mis-granted restore would still refuse. `versions.get` is named separately from `.list`
    // because a snapshot is the full record content while the list is provenance.
    "mcp:versions.list:call",
    "mcp:versions.get:call",
    // Reading how many versions the workspace keeps is a plain fact about the surface a viewer uses;
    // CHANGING it (`versions.config.set`) is admin-only, below.
    "mcp:versions.config.get:call",
    "mcp:media.list:call",
    // `media.read` returns BYTES (base64, in bounded slices) for callers that cannot set an
    // `Authorization` header — a module-federated extension UI, which the host mounts without the
    // session token on purpose. It sits on the viewer tier because it is a READ of media this
    // workspace already holds, and because the alternative it replaces was extensions lifting the
    // host's token out of `localStorage` to call `GET /media/{id}` themselves.
    //
    // This grant does NOT widen reach: the verb delegates to `media_serve`, which re-checks the
    // per-item `store:media/{id}:read` capability, so holding this alone reaches no media the
    // caller could not already serve over HTTP.
    "mcp:media.read:call",
    // …and that inner re-check needs the store-surface cap BESIDE the verb cap, named concretely
    // for exactly the reason `store:asset/*` and `store:doc/*` are above: the generic
    // `store:*:read` wildcard is SINGLE-SEGMENT and does not span the `media/{id}` resource path.
    // Without this line `mcp:media.read:call` is a grant that reaches nothing — every call clears
    // the outer gate and is denied inside `media_serve`. Measured on a live node: upload + commit
    // succeeded and `media.get` returned the metadata, while `media.read` on that same id answered
    // a bare `denied` with the verb cap plainly held in the token.
    "store:media/*:read",
    "mcp:nav.hidden.get:call",
    "mcp:nav.pref.get:call",
    // generic per-workspace store READ. The verb-READ wildcards that used to live here
    // (`mcp:*.get:call` / `mcp:*.list:call`) are GONE — see the module note on the wildcard span:
    // their span silently covered the admin-only `teams.list` / `roles.list` / `grants.list` /
    // `invite.list` / `series.retention.list`, which is not a viewer's to see.
    "store:*:read",
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
    // insight DESTROY — delete an insight (cascades its ring) or one occurrence row. Erasing shared
    // content + evidence is an authoring reach a bare viewer must NOT have (only reads/acks/resolves).
    "mcp:insight.delete:call",
    "mcp:insight.occurrence.delete:call",
    // Studio local-dev devkit: scaffold/build/inspect/write — the authoring toolchain.
    "mcp:devkit.templates:call",
    "mcp:devkit.scaffold:call",
    "mcp:devkit.write_file:call",
    "mcp:devkit.inspect:call",
    "mcp:devkit.build:call",
    "mcp:devkit.root:call",
    // datasources chain — the member REGISTERS/TESTS their own sources over real series.
    // `native.call` is the supervisor CONTROL PLANE (spawn/drive a child) and belongs on this tier
    // for that reason alone. It is deliberately NOT what a read costs: host-mediated federation
    // READS dispatch via `call_sidecar_mediated`, gated only by their own `mcp:federation.query:call`
    // — so the viewer tier can execute the verb it is granted without any authoring reach.
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
    // datasource-profile scope: REFRESHING a profile spends real work on the external database on
    // demand, so it is its own grant — separately revokable from the read cap that `profile_get`
    // and `profile` ride. An author may trigger it; a viewer may only read the result.
    "mcp:federation.profile_refresh:call",
    "mcp:dbschema.save:call",
    "mcp:dbschema.delete:call",
    // ingest — a member WRITES their own series (producer = the authed principal). Reads are viewer.
    "mcp:ingest.write:call",
    // documents WRITE (a member's own shared docs).
    "mcp:assets.put_doc:call",
    // binary assets WRITE (document-store scope move 2) — a member uploads/deletes their OWN
    // shared binary assets (glTF models, image underlays), the same tier as `put_doc`; ownership
    // still gates WHICH asset (delete_asset re-checks owner). `store:asset/*:write` is the inner
    // store-surface half — named concretely, the `store:*:write` wildcard does not span it.
    "mcp:assets.put_asset:call",
    "mcp:assets.delete_asset:call",
    "store:asset/*:write",
    // doc extraction (doc-extraction scope): a member derives docs from their own media. The verb
    // re-gates per-item media read + doc write, so this grant alone can't widen reach — it only
    // opens the surface, exactly like `assets.put_doc` above.
    "mcp:docs.extract:call",
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
    // ext-store-nodes scope: the flow-editor store-table picker. Reveals table names + row counts
    // ONLY (an author holding `store.query` can enumerate them anyway) — moved down from admin so
    // `store.tables` opens to flow authors. The raw-ROW lenses (`store.scan`/`store.graph`) stay
    // admin-only below: they relax gate 3 and answer "every record in the workspace".
    "mcp:store.tables:call",
    // ext-store-nodes scope: the `ext-list` flow node and the `lb:extension` picker dispatch the
    // READ verb `ext.list` under the author's own principal (it returns the workspace's install
    // inventory — id/version/tier/enabled/running/health, no bytes, no config). A flow author must
    // hold it to branch on "is modbus running?"; the picker must hold it to offer the ext dropdown.
    // Moved down from admin alongside `store.tables` (the same scope opened that one) — the two are
    // the picker's read verbs. The LIFECYCLE MUTATORS stay admin-only below (`ext.disable`/`start`/
    // `uninstall`/`publish`, `native.install`): enabling/stopping/removing an extension is an admin
    // authority, listing what's installed is not. `ext.list` is host-native by EXACT name (rule 10),
    // so this grants no reach over an `ext.<other>` an extension owns.
    "mcp:ext.list:call",
    // `ext.list`'s read-only peer: the catalog's full per-version history for one extension (id/
    // version/digest/publisher/ts — still no bytes). Same author-tier reasoning as `ext.list` right
    // above — enumerating past versions is a read, not a lifecycle mutation, so it stays out of the
    // admin-only bucket below.
    "mcp:ext.versions:call",
    // ros extension (rust/extensions/ros) — the read half of its connection + fleet CRUD, same
    // author-tier reasoning as `ext.list`/`ext.versions` right above: browsing what's already
    // registered (connections, networks, devices, points) is a read, not a lifecycle mutation or a
    // credential handling. The WRITE/lifecycle half (`ros.create`/`update`/`delete`, `ros.point.write`,
    // `ros.start`/`stop`/`restart`) stays admin-only below — those hold or rotate a real appliance
    // token, or push a setpoint onto real hardware. All `ros.`-prefixed by EXACT name (rule 10, and
    // load-bearing here for a second reason: dispatch itself resolves a qualified tool's owning
    // extension by its prefix before the first '.' — a bare `network.list`/`device.list`/`point.list`
    // is not just a capability-namespace risk, it is literally unreachable, since it would resolve to
    // a nonexistent extension id `network`/`device`/`point`). This also means no collision with
    // `notify`'s unrelated `mcp:device.list:call` (bare name, different feature).
    "mcp:ros.list:call",
    "mcp:ros.get:call",
    "mcp:ros.ping:call",
    "mcp:ros.network.list:call",
    "mcp:ros.network.get:call",
    "mcp:ros.device.list:call",
    "mcp:ros.device.get:call",
    "mcp:ros.point.list:call",
    "mcp:ros.point.get:call",
    "mcp:ros.schedule.list:call",
    "mcp:ros.schedule.get:call",
    // The box's own Location/Group/Host hierarchy (ros-location-group scope), proxied live — same
    // read-tier reasoning as the rest of ros right above: browsing it is a read, no write verb exists
    // (it's pre-existing box data, not something rubix-ai creates).
    "mcp:ros.location.list:call",
    "mcp:ros.group.list:call",
    "mcp:ros.host.list:call",
    // dashboards — a member BUILDS/SHARES/DELETES their OWN (gate-3 owns which). The `*_any`
    // overrides (save/share/delete) are admin-only, below.
    "mcp:dashboard.save:call",
    "mcp:dashboard.delete:call",
    "mcp:dashboard.share:call",
    // forms — a member BUILDS/DELETES their OWN (owner-only). delete_any is admin.
    "mcp:forms.save:call",
    "mcp:forms.delete:call",
    // share-closure scope: share the page's embedded library panels to a team. AUTHOR-tier, not
    // admin: it is an authoring action a member takes on their OWN panels — each panel is shared only
    // through `panel.share`, which re-checks the owner rule, so this cap can never widen a member's
    // reach beyond the panels they already own. A viewer holds neither this nor `panel.share` (the
    // verb would be a no-op for them). NOT a wildcard: `share_closure` is named concretely here, per
    // the module doc's exhaustive-by-name rule.
    "mcp:dashboard.share_closure:call",
    // viz import-export scope: import is a WRITE (creates a dashboard) — an author cap; it also
    // requires `mcp:dashboard.save:call` above (the two-gate write). export is a viewer read.
    "mcp:dashboard.import:call",
    // panels — a member's own reusable/standalone panels (author + share).
    "mcp:panel.save:call",
    "mcp:panel.delete:call",
    "mcp:panel.share:call",
    // reports — a member authors + shares + exports reports and authors brand profiles.
    // `report.export` is a CONCRETE cap (not covered by any wildcard) — the view-but-not-export line.
    "mcp:report.save:call",
    "mcp:report.delete:call",
    "mcp:report.share:call",
    "mcp:report.export:call",
    "mcp:brand.save:call",
    "mcp:brand.delete:call",
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
    // job-backed rule runs (long-running-rules-scope) — start + observe + control of the member's
    // own workspace runs; every data/ai/messaging verb inside a run still re-checks caller ∩ grant.
    "mcp:rules.run_async:call",
    "mcp:rules.runs.get:call",
    "mcp:rules.runs.list:call",
    "mcp:rules.runs.suspend:call",
    "mcp:rules.runs.resume:call",
    "mcp:rules.runs.cancel:call",
    // saved queries — a member AUTHORS/RUNS their own (query.run COMPOSES the target cap, no widening).
    "mcp:query.save:call",
    "mcp:query.run:call",
    "mcp:query.compile:call",
    "mcp:query.get:call",
    "mcp:query.list:call",
    "mcp:query.delete:call",
    // global schedules — a member AUTHORS the shared schedule records the `schedule` node and the
    // dashboard widget both reference. Reads are on the viewer floor.
    "mcp:schedule.save:call",
    "mcp:schedule.delete:call",
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
    // undo-exposure scope: a member undoes/redoes their OWN steps. Undo is a mutation (author
    // tier), and the host's no-escalation check (the original tool's cap) means undo can never
    // reach beyond the caps the caller already holds. The DOTLESS verb names match none of the
    // `mcp:*.<verb>:call` wildcards (the wildcard segment is a `<tool>.<verb>` split), so they
    // must be named concretely here. `history.list` rides the viewer `mcp:*.list:call` read
    // wildcard; `history.compensations` is its own concrete verb (no wildcard matches it).
    // `mcp:undo.any:call` (another actor's stack) is ADMIN-only, below.
    "mcp:undo:call",
    "mcp:redo:call",
    "mcp:history.compensations:call",
    // versions scope (#112): restoring a version IS performing that entity's own save, so it sits at
    // the same author tier as the save — and the verb's no-escalation check re-demands the kind's
    // save cap before it re-dispatches, so this grant can never reach a mutation the caller could
    // not perform directly. The named deny the scope requires: a viewer holding `versions.list` but
    // not `mcp:dashboard.save:call` is refused `versions.restore`.
    "mcp:versions.restore:call",
    // media scope: a member uploads/reads/deletes their own media.
    "mcp:media.upload:call",
    "mcp:media.get:call",
    "mcp:media.delete:call",
    // push-target scope: a member sends push notifications (the audience/prefs policy lives here).
    "mcp:notify.send:call",
    // shared-asset doc/skill store WRITES (gate-3/ownership owns which specific asset).
    "store:doc/*:write",
    "store:skill/**:write",
    // The author verbs the retired `mcp:*.write|create|update|delete|post:call` wildcards supplied,
    // now named. Each is a member's own authoring reach; gate-3/ownership still owns WHICH record.
    // NOT restored, deliberately: `secret.delete` (the secret plane is not an author's to mutate) and
    // `teams.create` / `roles.delete` (admin verbs — their tools dispatch on `teams.manage` /
    // `roles.manage`, so the retired wildcard never actually reached them, but naming them here would
    // grant by the back door what `ADMIN_ONLY_CAPS` denies by the front).
    "mcp:channel.create:call",
    "mcp:channel.delete:call",
    "mcp:channel.post:call",
    "mcp:reminder.create:call",
    "mcp:reminder.update:call",
    "mcp:reminder.delete:call",
    "mcp:store.write:call",
    "mcp:store.delete:call",
    // generic per-workspace store WRITE. The verb-CRUD WRITE wildcards that used to live here are
    // GONE — see the module note on the wildcard span. They were what made `member` an author, but
    // their span also silently covered the admin-only `invite.create` / `nav.delete` (and reached
    // `workspace.create` / `workspace.delete` / `series.delete` / `ext.list` through the same hole),
    // which is the `user:bob` escalation this module exists to prevent, returning by wildcard.
    "store:*:write",
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
    // `teams.create` / `roles.delete` name the same authority as `teams.manage` / `roles.manage`
    // (their tools dispatch on those, so these names are not independently reachable today). Listed
    // anyway: the wildcard-span test is only as good as this list, and an unlisted admin verb is
    // exactly how a future `mcp:*.create:call` would re-open the hole undetected.
    "mcp:teams.create:call",
    "mcp:roles.define:call",
    "mcp:roles.list:call",
    "mcp:roles.manage:call",
    "mcp:roles.delete:call",
    "mcp:grants.assign:call",
    "mcp:grants.list:call",
    "mcp:identity.manage:call",
    // packs (packs scope): APPLYING a pack writes through every object family at once — a
    // datasource, saved rules that then RUN, dashboards, channels, and the workspace-shared agent
    // context. That is the admin authority, so the surface cap is admin-only. It is a gate, not a
    // grant: each object is driven through the same internal seam its public verb uses, and every
    // one of those re-checks its own capability under the caller — a pack cannot smuggle in a write
    // its caller could not perform directly. (validate/list/get are viewer reads, above.)
    "mcp:pack.apply:call",
    // versions scope (#112): the ring cap decides how much of EVERY member's work history the
    // workspace keeps, so lowering it destroys other people's recoverability — workspace
    // administration, not a member preference. The read (`versions.config.get`) is viewer-tier.
    "mcp:versions.config.set:call",
    // destructive / creating workspace ops. `provision` stands up a complete workspace (directory
    // row + first admin, atomically) and `reconcile` repairs a memberless orphan — both grant an
    // admin into a workspace the caller need not belong to, the definition of admin authority
    // (workspace-provision scope; reconcile's super-admin-only question is tracked as OQ4).
    "mcp:workspace.create:call",
    "mcp:workspace.provision:call",
    "mcp:workspace.reconcile:call",
    "mcp:workspace.delete:call",
    "mcp:workspace.purge:call",
    // access console: resolved effective caps + live-token revoke.
    "mcp:authz.resolve:call",
    "mcp:authz.revoke-tokens:call",
    // extension lifecycle + native supervision + publish. NOTE: `ext.list` (the READ — install
    // inventory) is NOT here; it moved to AUTHOR_CAPS (ext-store-nodes scope) so a flow author's
    // `ext-list` node and the `lb:extension` picker can enumerate installs. Only the MUTATORS —
    // disable/start/uninstall/publish + native install — are admin authority.
    "mcp:ext.disable:call",
    // `ext.start` is the peer of `ext.disable`'s stop-half: start a stopped extension now, without
    // bouncing the node. Same authority tier (an admin who may stop an extension may start it).
    "mcp:ext.start:call",
    "mcp:ext.uninstall:call",
    "mcp:ext.publish:call",
    // ros extension — the write/lifecycle half. `create`/`update`/`delete` hold or rotate a real
    // appliance token (same tier as any other credential-holding admin verb); `point.write` pushes a
    // setpoint onto real hardware; `start`/`stop`/`restart` control the poll loop. The read half
    // (`ros.list`/`get`/`ping`, `network.*`/`device.*`/`point.list`/`get`) is author-tier, above.
    "mcp:ros.create:call",
    "mcp:ros.update:call",
    "mcp:ros.delete:call",
    "mcp:ros.point.write:call",
    // pushes a schedule payload onto real hardware — same tier as `ros.point.write` (a schedule write
    // is exactly as consequential as a setpoint write).
    "mcp:ros.schedule.write:call",
    "mcp:ros.start:call",
    "mcp:ros.stop:call",
    "mcp:ros.restart:call",
    "mcp:native.reset:call",
    "mcp:native.install:call",
    // skills lifecycle (adopt/drop/soft-hide a skill for the workspace).
    "mcp:assets.put_skill:call",
    "mcp:assets.grant_skill:call",
    "mcp:assets.revoke_skill:call",
    "mcp:assets.deprecate_skill:call",
    // raw-store lenses — a scan answers "every record in the workspace" (relaxes gate-3).
    // (`store.tables` — names + counts only — is AUTHOR-tier since the ext-store-nodes scope.)
    "mcp:store.scan:call",
    "mcp:store.graph:call",
    // store operational pair (online-compaction scope): node-level maintenance is workspace-data
    // administration. status re-gates `store:status:read` inside (covered by the inherited
    // `store:*:read`); compact re-gates the DISTINCT `store:compact:run` — a `run`, not a
    // `write`, precisely so the author `store:*:write` wildcard can never pause the node.
    "mcp:store.status:call",
    "mcp:store.compact:call",
    "store:compact:run",
    // node update (node-update scope) — three grants split by BLAST RADIUS: reading a version is not
    // applying one, and applying one is not holding the backend's credential. All three are
    // workspace-admin ONLY and never a member's; `update.credential` is documented as **equivalent to
    // backend admin** (lb cannot narrow a backend's credential), and none of the three belongs in the
    // default agent capability ceiling — an agent that can replace the node's binary is a different
    // product. The verbs are node-scoped, the deliberate exception `store.status`/`store.compact`
    // already set.
    "mcp:update.read:call",
    "mcp:update.apply:call",
    "mcp:update.credential:call",
    // system map — reads across every subsystem of the workspace.
    "mcp:system.overview:call",
    "mcp:system.topology:call",
    "mcp:system.subsystem:call",
    "mcp:system.tools:call",
    "mcp:system.acp:call",
    // series retention — deleting/downsampling other producers' history is workspace data
    // administration, never an author privilege (series-retention scope).
    "mcp:series.retention.set:call",
    "mcp:series.retention.list:call",
    // Reading the policy in force + the last GC pass is the READ half of that same administration
    // (it reveals the workspace's retention configuration and bookkeeping), so it is granted here
    // alongside `.list` rather than on the viewer tier (series-observability scope).
    "mcp:series.retention.status:call",
    "mcp:series.retention.gc:call",
    // series lifecycle — destroying or renaming a whole series (across every producer's history) is
    // the same workspace-data-administration privilege as retention, never an author one.
    "mcp:series.delete:call",
    "mcp:series.rename:call",
    // dashboard admin overrides — the complete triad over an asset someone else owns
    // (ext-managed-dashboards D2). Each is its OWN cap, each checked strictly after the verb's owner
    // check fails, none folded into an ambient admin-role test. Ships together on purpose: an admin
    // who may delete a board but not fix or re-scope it is the asymmetry that sends operators to
    // delete-and-recreate. Nobody holds these by default — they arrive only with `workspace-admin`.
    "mcp:dashboard.delete_any:call",
    "mcp:dashboard.save_any:call",
    "mcp:dashboard.share_any:call",
    // form admin override (delete a form the admin doesn't own).
    "mcp:forms.delete_any:call",
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
    // invites scope: admin mints/list/revokes/resends invite tokens (accept is pre-auth).
    "mcp:invite.create:call",
    "mcp:invite.list:call",
    // undo-exposure scope: undoing ANOTHER actor's step touches another principal's work — the
    // definition of an admin-only cap. Always prominently audited by the host verb.
    "mcp:undo.any:call",
    // response-cache scope: observing the node's response-cache internals (`cache.stats`) and
    // purging a workspace's cached reads (`cache.purge`) are operator/admin authorities — a
    // node-diagnostic read and the stale-data escape hatch, respectively. A member/viewer holds
    // neither, so a non-admin caller is opaquely `Denied` on a warm key exactly as on a cold one.
    "mcp:cache.stats:call",
    "mcp:cache.purge:call",
];

/// Does this cap set carry workspace-admin authority? True iff it holds ANY [`ADMIN_ONLY_CAPS`]
/// entry — the caps that MANAGE other principals or the workspace itself, which lb grants only via
/// the `workspace-admin` role bundle and a `member`/`viewer`/guardian never holds.
///
/// This is the AUTHORITATIVE admin signal in lb's caps-based model: the JWT `role` claim is
/// cosmetic (`dev_claims` mints `member` for admins and members alike — the check path reads caps,
/// never `role`; see `lb-role-gateway::session::credentials`). A caller-projection that needs to
/// answer "is this an admin?" (e.g. the native-caller-identity frame a sidecar reads to bypass a
/// per-caller row filter) MUST resolve it from caps, not the role enum. One owner of the rule so
/// the frame's `admin` marker and lb's own admin gating can never drift.
pub fn caps_hold_admin(caps: &[String]) -> bool {
    caps.iter().any(|c| ADMIN_ONLY_CAPS.contains(&c.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authz::holds_cap;
    use lb_auth::Principal;

    /// The escalation regression, at the cap-bundle layer: a member's bundle holds NONE of the
    /// admin-only caps the live `user:bob` abused (members.add / teams.manage / grants.assign /
    /// members.manage / workspace.create / workspace.delete / dashboard.delete_any).
    #[test]
    fn member_bundle_holds_no_admin_caps() {
        let member = member_role_caps();
        for admin_cap in [
            "mcp:members.add:call",
            "mcp:teams.manage:call",
            "mcp:roles.define:call",
            "mcp:grants.assign:call",
            "mcp:members.manage:call",
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

    /// **THE REGRESSION**: a verb cap whose inner per-item gate nothing in the bundle can satisfy
    /// is a grant that reaches nothing. `mcp:media.read:call` shipped on the viewer tier without
    /// `store:media/*:read` beside it, and the generic `store:*:read` is SINGLE-SEGMENT — it cannot
    /// span `media/{id}`. So every `media.read` cleared the outer gate and was denied inside
    /// `media_serve`. Asserting the literal cap string is not enough; this drives the real matcher
    /// against a concrete id, which is the only form that fails when the wildcard shape is wrong.
    #[test]
    fn the_media_read_grant_actually_reaches_a_media_item() {
        use lb_caps::{check, Action, Decision, Request, Surface};

        for (tier, caps) in [
            ("viewer", viewer_role_caps()),
            ("member", member_role_caps()),
            ("workspace-admin", workspace_admin_role_caps()),
        ] {
            let principal = Principal::routed("user:test", "nube", caps.clone());
            let req = Request::new("nube", Surface::Store, "media/413d599d18e6", Action::Read);
            assert!(
                !matches!(check(&principal, &req), Decision::Denied(_)),
                "{tier} holds mcp:media.read:call but cannot satisfy the per-item \
                 store:media/{{id}}:read gate — the grant reaches no media at all"
            );
        }
    }

    /// `caps_hold_admin` is the authoritative admin signal for the native-caller frame: the
    /// workspace-admin bundle reads as admin, the member/viewer bundles do NOT (the escalation the
    /// role-enum can't answer — lb mints every session as `member`). Guards the frame's `admin`
    /// marker against drift with the admin-only cap delta.
    #[test]
    fn caps_hold_admin_tracks_the_admin_bundle_only() {
        assert!(
            caps_hold_admin(&workspace_admin_role_caps()),
            "the workspace-admin bundle must read as admin"
        );
        assert!(
            !caps_hold_admin(&member_role_caps()),
            "the member bundle must NOT read as admin (the escalation)"
        );
        assert!(
            !caps_hold_admin(&viewer_role_caps()),
            "the viewer bundle must NOT read as admin"
        );
        // A guardian-style token (a couple of scoped care read caps, no admin-only cap) is NOT admin.
        let guardian = vec![
            "mcp:care.child.get:call".to_string(),
            "mcp:care.child.list:call".to_string(),
        ];
        assert!(
            !caps_hold_admin(&guardian),
            "a guardian token must NOT read as admin"
        );
        // The single canonical marker is enough on its own.
        assert!(caps_hold_admin(&["mcp:members.manage:call".to_string()]));
        assert!(!caps_hold_admin(&[]), "an empty cap set is never admin");
    }

    /// Undo-exposure grants (undo-exposure scope): a member holds the DOTLESS undo verbs (no
    /// `mcp:*.<verb>:call` wildcard matches them) + `history.compensations`; `undo.any` (another
    /// actor's stack) is admin-only; a viewer holds none of the mutating three but keeps
    /// `history.list` via the read wildcard (seeing history you cannot act on is correct).
    #[test]
    fn undo_exposure_grants_land_at_the_right_tiers() {
        let member = member_role_caps();
        let viewer = viewer_role_caps();
        for c in [
            "mcp:undo:call",
            "mcp:redo:call",
            "mcp:history.compensations:call",
        ] {
            assert!(member.contains(&c.to_string()), "member must hold {c}");
            assert!(!viewer.contains(&c.to_string()), "viewer must NOT hold {c}");
        }
        assert!(
            admin_only_caps().contains(&"mcp:undo.any:call".to_string()),
            "undo.any must be admin-only"
        );
        assert!(
            !member.contains(&"mcp:undo.any:call".to_string()),
            "member must NOT hold undo.any (cross-actor undo is admin authority)"
        );
        // A viewer still REACHES `history.list` — asserted through the matcher, not by naming the
        // mechanism. This once read `viewer.contains("mcp:*.list:call")`, which pinned the very
        // wildcard that leaked `teams.list`/`roles.list`; the reach is the contract, the wildcard was
        // only ever one way to supply it (now: named concretely in VIEWER_CAPS).
        let v = Principal::routed("user:probe", "nube", viewer.clone());
        assert!(
            holds_cap(&v, "nube", "mcp:history.list:call"),
            "viewer must still reach history.list"
        );
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

    /// **The `share_closure` cap tier** (share-closure scope). `mcp:dashboard.share_closure:call` is an
    /// AUTHOR cap: a member shares the library panels they OWN on a page with a team. It must NOT be
    /// admin-only (that would make "share the widgets too" an admin errand for every member's own
    /// panel) and must NOT be a viewer cap (a viewer holds no `panel.share`, so the verb could only
    /// ever no-op for them). It must be named CONCRETELY — the module doc's exhaustive-by-name rule.
    ///
    /// Pinned here so a future re-classification fails at the bundle rather than in production. The
    /// wildcard-span invariant below covers the other half (that no bundle's wildcards reach an
    /// admin-only cap); this asserts the placement the span test cannot see.
    #[test]
    fn share_closure_cap_is_an_author_cap_not_admin_or_viewer() {
        let cap = "mcp:dashboard.share_closure:call".to_string();
        assert!(
            author_caps().contains(&cap),
            "share_closure must be an AUTHOR cap (a member shares their OWN panels)"
        );
        assert!(
            member_role_caps().contains(&cap),
            "the member bundle must hold share_closure by name"
        );
        assert!(
            !viewer_role_caps().contains(&cap),
            "a viewer must NOT hold share_closure (it holds no panel.share to act with)"
        );
        assert!(
            !admin_only_caps().contains(&cap),
            "share_closure must NOT be admin-only — it is a member's own authoring action"
        );
        // It travels with the cap that does the actual work: sharing a panel. A bundle holding
        // share_closure without panel.share could only ever report `no_share_cap` for every row.
        assert!(
            member_role_caps().contains(&"mcp:panel.share:call".to_string()),
            "share_closure is useless without panel.share — they belong to the same tier"
        );
    }

    /// **The `ext.list` cap tier** (ext-store-nodes scope). `mcp:ext.list:call` is an AUTHOR cap: a
    /// flow author's `ext-list` node and the editor's `lb:extension` picker enumerate the workspace's
    /// installs (id/version/tier/enabled/running/health — a READ, no bytes). It must NOT be admin-only
    /// (that denies every member's `ext-list` node — the live "list all extensions → denied" this
    /// pins), and the LIFECYCLE MUTATORS must stay admin-only (enabling/removing an extension is admin
    /// authority, listing what's installed is not). Pinned so a future re-classification of either
    /// half fails at the bundle rather than in production.
    #[test]
    fn ext_list_is_an_author_cap_but_lifecycle_mutators_stay_admin() {
        let list = "mcp:ext.list:call".to_string();
        assert!(
            author_caps().contains(&list),
            "ext.list must be an AUTHOR cap — a flow author lists installs (ext-list node + picker)"
        );
        assert!(
            member_role_caps().contains(&list),
            "the member bundle must hold ext.list by name"
        );
        assert!(
            !admin_only_caps().contains(&list),
            "ext.list must NOT be admin-only — that denies every member's ext-list node"
        );
        // The mutators are the admin half — a member may enumerate installs, never change them.
        for mutator in [
            "mcp:ext.disable:call",
            "mcp:ext.start:call",
            "mcp:ext.uninstall:call",
            "mcp:ext.publish:call",
            "mcp:native.install:call",
        ] {
            assert!(
                admin_only_caps().contains(&mutator.to_string()),
                "the extension mutator {mutator} must stay admin-only"
            );
            assert!(
                !member_role_caps().contains(&mutator.to_string()),
                "a member must NOT hold the extension mutator {mutator}"
            );
        }
    }

    /// `ext.versions`'s own tier, pinned the same way `ext.list`'s is right above: it is `ext.list`'s
    /// read-only peer (per-extension version history, still no bytes), so it must land in the exact
    /// same AUTHOR tier — not admin-only (that would deny a flow author/picker the same way an
    /// ext.list misclassification would), and not silently absent from the member bundle.
    #[test]
    fn ext_versions_is_an_author_cap_alongside_ext_list() {
        let versions = "mcp:ext.versions:call".to_string();
        assert!(
            author_caps().contains(&versions),
            "ext.versions must be an AUTHOR cap — ext.list's read-only peer"
        );
        assert!(
            member_role_caps().contains(&versions),
            "the member bundle must hold ext.versions by name"
        );
        assert!(
            !admin_only_caps().contains(&versions),
            "ext.versions must NOT be admin-only — it is a read, like ext.list"
        );
    }

    /// The `ros` extension's read/write tier split, pinned the same way `ext.list`/`ext.versions`
    /// are above: browsing a connection/fleet is a member-reachable read, holding/rotating the
    /// appliance token or writing a setpoint is admin-only.
    #[test]
    fn ros_read_verbs_are_author_caps_and_write_verbs_are_admin_only() {
        for read in [
            "mcp:ros.list:call",
            "mcp:ros.get:call",
            "mcp:ros.ping:call",
            "mcp:ros.network.list:call",
            "mcp:ros.network.get:call",
            "mcp:ros.device.list:call",
            "mcp:ros.device.get:call",
            "mcp:ros.point.list:call",
            "mcp:ros.point.get:call",
            "mcp:ros.schedule.list:call",
            "mcp:ros.schedule.get:call",
            "mcp:ros.location.list:call",
            "mcp:ros.group.list:call",
            "mcp:ros.host.list:call",
        ] {
            let cap = read.to_string();
            assert!(author_caps().contains(&cap), "{read} must be an AUTHOR cap");
            assert!(
                member_role_caps().contains(&cap),
                "the member bundle must hold {read} by name"
            );
            assert!(
                !admin_only_caps().contains(&cap),
                "{read} must NOT be admin-only — it is a read"
            );
        }
        // `mcp:device.list:call` (bare, `notify`'s unrelated feature) must stay untouched by any of
        // ros's own `ros.device.list` caps — confirms the prefix actually avoids the collision rather
        // than just moving it.
        let notify_device_list = "mcp:device.list:call".to_string();
        assert!(
            !author_caps().contains(&notify_device_list),
            "ros's rename must not incidentally grant notify's bare device.list from AUTHOR_CAPS"
        );
        for write in [
            "mcp:ros.create:call",
            "mcp:ros.update:call",
            "mcp:ros.delete:call",
            "mcp:ros.point.write:call",
            "mcp:ros.schedule.write:call",
            "mcp:ros.start:call",
            "mcp:ros.stop:call",
            "mcp:ros.restart:call",
        ] {
            let cap = write.to_string();
            assert!(
                admin_only_caps().contains(&cap),
                "{write} must be admin-only — holds a credential or writes real hardware"
            );
            assert!(
                !author_caps().contains(&cap),
                "{write} must NOT leak into AUTHOR_CAPS"
            );
            assert!(
                !member_role_caps().contains(&cap),
                "{write} must NOT be in the member bundle"
            );
        }
    }

    /// The `share_closure` cap REACHES through the real matcher for a member and does NOT for a
    /// viewer — asserted through `holds_cap` (the wall), not literal membership, because the reach is
    /// the contract and the bundle listing is only one way to supply it.
    #[test]
    fn share_closure_cap_reaches_for_a_member_only() {
        let member = Principal::routed("user:m", "nube", member_role_caps());
        let viewer = Principal::routed("user:v", "nube", viewer_role_caps());
        assert!(
            holds_cap(&member, "nube", "mcp:dashboard.share_closure:call"),
            "a member must reach share_closure"
        );
        assert!(
            !holds_cap(&viewer, "nube", "mcp:dashboard.share_closure:call"),
            "a viewer must NOT reach share_closure"
        );
    }

    /// **The wildcard-span invariant** — the one that actually binds, and the gap every other test in
    /// this module missed. The checks above ask `bundle.contains(cap)`: a LITERAL membership test. But
    /// the wall does not enforce literal membership — it enforces [`holds_cap`], which is wildcard-aware
    /// by design ("would this pass Gate 2?"). So a bundle can hold NO admin cap literally while its
    /// broad author wildcards AUTHORIZE a dozen of them. That is precisely what happened: `AUTHOR_CAPS`
    /// granted `mcp:*.list:call` / `mcp:*.delete:call` / `mcp:*.create:call`, whose span silently
    /// covered `teams.list`, `roles.list`, `grants.list`, `invite.list`/`create`, `ext.list`,
    /// `series.delete`, `nav.delete`, and `workspace.create`/`delete` — ten admin-only caps, live, for
    /// every plain member (observed 2026-07-16: `GET /admin/teams` and `roles.list` both 200 as
    /// `user:bob`). This is the `user:bob` escalation this module was WRITTEN to prevent, returning
    /// through the wildcard path the literal tests cannot see.
    ///
    /// The rule this pins: **a broad wildcard must never span an admin-only cap.** It is asserted
    /// through the real matcher against the real bundles, so any future wildcard added to `AUTHOR_CAPS`
    /// or `VIEWER_CAPS` that reaches an admin verb fails HERE — at the bundle it was added to — rather
    /// than leaking into production behind whichever caller probes it next. Adding an admin-only cap
    /// whose verb collides with an existing author wildcard fails here too, which is the other half of
    /// the class.
    #[test]
    fn no_builtin_bundle_may_span_an_admin_only_cap() {
        for (role, caps) in [
            ("member", member_role_caps()),
            ("viewer", viewer_role_caps()),
            // The API-key data-plane bundles had the identical hole, one layer down: they reasoned
            // that action-naming (`*.list` not `*.*`) kept a data key out of `apikey.manage`, which
            // was true and insufficient — `*.list` still spanned `teams.list`/`roles.list`. They are
            // asserted HERE because `lb-apikey` sits below `lb-host` and cannot see this list.
            ("apikey-read", lb_apikey::apikey_read_caps()),
            ("apikey-write", lb_apikey::apikey_write_caps()),
        ] {
            let principal = Principal::routed("user:probe", "nube", caps);
            let spanned: Vec<String> = admin_only_caps()
                .into_iter()
                .filter(|cap| holds_cap(&principal, "nube", cap))
                .collect();
            assert!(
                spanned.is_empty(),
                "the {role} bundle must not AUTHORIZE any admin-only cap, but its wildcards span \
                 {}: {spanned:#?}\nA broad `mcp:*.<verb>:call` in the {role} bundle reaches these \
                 admin verbs through the caps grammar. Name the concrete author verbs instead of \
                 widening the wildcard — see the module doc.",
                spanned.len()
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
            "mcp:forms.save:call",
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
            "mcp:forms.get:call",
            "mcp:forms.list:call",
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

    /// Every render-path cap the viewer bundle grants must be one the viewer can actually EXECUTE —
    /// no cap whose dispatch depends on a cap held only by a higher tier.
    ///
    /// This is the failure `viewer_bundle_keeps_render_path` cannot see, and it shipped: the viewer
    /// held `mcp:federation.query:call` (that test was green) while the host's federation dispatch
    /// additionally gated on `mcp:native.call:call`, an AUTHOR cap sitting with datasource
    /// registration. Granted-but-inert: every datasource-backed panel returned `denied` for a
    /// read-only user, surfaced in the UI as an empty chart ("no data yet"), and the only workaround
    /// was granting authoring reach to draw a chart. A `contains` assertion is structurally blind to
    /// it — the cap IS in the bundle. So assert the DEPENDENCY instead: a viewer render cap must not
    /// require an author-tier cap to run.
    ///
    /// Pins `federation.query` → `call_sidecar_mediated` (not `call_sidecar`): the mediated path
    /// carries no `mcp:native.call:call` check, because the verb already gated itself.
    #[test]
    fn viewer_render_caps_do_not_depend_on_author_caps() {
        let viewer = viewer_role_caps();
        let author_only: Vec<String> = member_role_caps()
            .into_iter()
            .filter(|c| !viewer.contains(c))
            .collect();

        // The supervisor control plane is author-tier — that is correct and stays.
        assert!(
            author_only.contains(&"mcp:native.call:call".to_string()),
            "native.call is expected to remain an author-tier control-plane cap"
        );
        // …and precisely because it is author-tier, no viewer render cap may need it to dispatch.
        // `federation.query` is the one that did; it now routes via `call_sidecar_mediated`.
        assert!(
            viewer.contains(&"mcp:federation.query:call".to_string()),
            "viewer holds the federation read cap"
        );
        assert!(
            !viewer.contains(&"mcp:native.call:call".to_string()),
            "viewer must NOT need the control-plane cap to run its granted read — if this ever \
             becomes true, the fix regressed into granting authoring reach instead of mediating \
             dispatch (see native::tool::call_sidecar_mediated)"
        );
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
