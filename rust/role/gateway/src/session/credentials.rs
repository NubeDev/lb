//! The **dev-login** credential map — the ONE non-real piece of the session (collaboration scope,
//! Non-goals: "no IdP yet; the token path is real even if the credential check starts as a dev-login").
//!
//! It maps a `(user, workspace)` login request to the claim set the gateway then mints into a real
//! signed token. There is no password DB here — a real credential check / IdP plugs in *here*, behind
//! the same `mint`/`verify` seam, without touching any route. The granted caps are the full member
//! set for the collaboration surfaces (channels, members, inbox, outbox, workspace directory) so the
//! demo principal can exercise every wired verb; a narrower dev principal is built by the tests to
//! prove the deny path.

use lb_auth::{Claims, Role};

/// The capability strings a dev member is granted — every collaboration verb's gate. Channel pub/sub
/// over `*` (post/read/list/create any channel) plus the MCP verb caps the new services check.
fn member_caps() -> Vec<String> {
    [
        "bus:chan/*:pub",
        "bus:chan/*:sub",
        "mcp:members.list:call",
        "mcp:members.add:call",
        "mcp:inbox.list:call",
        "mcp:inbox.resolve:call",
        "mcp:outbox.status:call",
        // proof-workflow-sim scope: the two durable-workflow WRITE verbs a guest drives over the host
        // callback to PRODUCE motion — create an inbox item, stage an outbox effect. Member-level, like
        // the other workflow verbs (the author/actor is host-forced to the principal's sub; the gateway
        // re-checks each server-side). `proof.simulate` calls these inside `caller ∩ install-grant`.
        "mcp:inbox.record:call",
        "mcp:outbox.enqueue:call",
        "mcp:workspace.list:call",
        "mcp:workspace.create:call",
        // admin-crud: the dev principal is a workspace admin so the console can exercise every
        // destructive verb. The gateway re-checks each on the server — the UI cap-gate is only a
        // convenience (admin-console scope). `workspace.purge` is the higher hard-delete ceiling.
        "mcp:workspace.delete:call",
        "mcp:workspace.purge:call",
        "mcp:user.manage:call",
        "mcp:user.disable:call",
        "mcp:teams.manage:call",
        "mcp:teams.list:call",
        "mcp:grants.assign:call",
        "mcp:grants.list:call",
        "mcp:roles.define:call",
        "mcp:roles.list:call",
        // admin-console slice 4: the extensions console lifecycle verbs, so the dev admin can list +
        // enable/disable/uninstall extensions from the browser. The gateway re-checks each on the
        // server; the UI cap-gate (showing the Extensions section) is convenience.
        "mcp:ext.list:call",
        "mcp:ext.disable:call",
        "mcp:ext.uninstall:call",
        // admin-console: publish (upload) a signed extension artifact over POST /extensions. The host
        // verb verify-before-stores; the gateway re-checks this cap server-side.
        "mcp:ext.publish:call",
        // extension SDK / Studio: local-only by deployment convention. The gateway dev node grants
        // these so the built-in Studio can scaffold/build/inspect through the universal MCP bridge.
        "mcp:devkit.templates:call",
        "mcp:devkit.scaffold:call",
        "mcp:devkit.inspect:call",
        "mcp:devkit.build:call",
        // Publishing a native devkit build reuses `ext.publish` plus the existing native install
        // gate before any child process is supervised.
        "mcp:native.install:call",
        // datasources scope: `federation.query`/`datasource.test` dispatch to the federation sidecar
        // via `call_sidecar`, which gates on `mcp:native.call:call`. The dev admin (which installs
        // natives) must also be able to CALL them, else the Datasources page's Test/query path 500s
        // with an opaque sidecar deny. The host re-checks; a token without it is refused.
        "mcp:native.call:call",
        // data-console (Ingest page): the S8 ingest/series verbs, surfaced over the gateway. These
        // are **member-level** — any member may explore + manually write their own series (the
        // producer is the authenticated principal, un-spoofable).
        "mcp:ingest.write:call",
        "mcp:series.read:call",
        "mcp:series.latest:call",
        "mcp:series.find:call",
        "mcp:series.list:call",
        // bus pub/sub (widget-config-vars "Platform fix") — member-level generic workspace-walled
        // motion. `bus.publish` (fire-and-forget) + `bus.watch` (subscribe). The subject is walled to
        // `ws/{id}/ext/{subject}` host-side from the token; a reserved prefix / cross-ws subject is refused.
        "mcp:bus.publish:call",
        "mcp:bus.watch:call",
        // command-palette catalog (channels-command-palette scope): the `/` palette's read. Member-
        // level — every UI-capable principal holds it; `tools.catalog` leaks only the tool *shapes*
        // the caller may already run (a denied tool is absent), never data. Without it the UI has no
        // palette at all.
        "mcp:tools.catalog:call",
        // host-callback scope: the proof-panel guest's own backend tool `proof.derive`, reachable over
        // the live `POST /mcp/call` bridge. The dev member may run it; the guest's INNER callbacks
        // (series.latest/ingest.write) authorize against `caller ∩ install-grant` — both held here.
        "mcp:proof-panel.proof.derive:call",
        // proof-workflow-sim scope: the guest sim tool the page's "Run workflow simulation" card
        // invokes over the live bridge. Its INNER callbacks (inbox.record/list/resolve,
        // outbox.enqueue/status) authorize against `caller ∩ install-grant` — all held here.
        "mcp:proof-panel.proof.simulate:call",
        // tag a series entity (member-level): the discovery edges `series.find` intersects. A member
        // may tag their own series; the test gateway's `/_seed/series` route uses this real write path.
        "mcp:tags.add:call",
        // data-console (Data page, the DB browser): the raw-store lens verbs. **ADMIN-ONLY** by
        // decision — they relax the per-record membership gate (gate 3): a raw scan answers "every
        // record in the workspace". The dev principal is a workspace admin (it holds the destructive
        // verbs above), so it carries them; a true member role must NOT. The gateway re-checks each
        // server-side, and a deny-test asserts a token without the cap is refused (data-console risk).
        "mcp:store.tables:call",
        "mcp:store.scan:call",
        "mcp:store.graph:call",
        // widget-builder Slice A (the "direct SurrealDB" widget source): the read-only SQL verbs the
        // `/store/query` + `/store/schema` routes check, and a widget cell reaches over the bridge.
        // `store.query` is a parse-allowlisted, bounded, workspace-walled SELECT (the parse gate +
        // wall + row-cap are the boundary; the cap grant is convenience). `store.schema` feeds the
        // visual SQL builder's dropdowns. The gateway re-checks each server-side; a token without the
        // cap is refused (the deny test). Granted here like the other raw-store lenses.
        "mcp:store.query:call",
        "mcp:store.schema:call",
        // system-map scope: the two read verbs the `/system/*` routes check. Admin-only by grant
        // convention — a system snapshot reads across every subsystem of the workspace (like the
        // `store.*` lens), so the cap rides the workspace-admin role, NOT the member set. The gateway
        // re-checks server-side; a token without the cap is refused (system_verb_without_cap_denied).
        "mcp:system.overview:call",
        "mcp:system.topology:call",
        // system-map subsystem detail: the per-subsystem detail verb a no-page card (gateway/bus/mcp)
        // drills into. Admin-only by the same convention — it reads one subsystem's full live state.
        "mcp:system.subsystem:call",
        // tool-catalog scope: the reachable MCP tool catalog (host-native + extension, with
        // descriptions) behind the MCP service page, and the ACP adapter's static facts behind the ACP
        // service page. Admin-only by the same convention — the catalog reads across the workspace's
        // whole tool surface. The gateway re-checks server-side; a token without the cap is refused.
        "mcp:system.tools:call",
        "mcp:system.acp:call",
        // dashboard scope (the grid-of-widgets surface): the five `dashboard.*` verbs the dashboard
        // routes check. Member-level — any member may build/share their own dashboards over real
        // series (gate 3 / ownership still decides which *specific* dashboard they read/edit). The
        // gateway re-checks each cap server-side; a token without them is refused per verb.
        "mcp:dashboard.get:call",
        "mcp:dashboard.list:call",
        "mcp:dashboard.save:call",
        "mcp:dashboard.delete:call",
        "mcp:dashboard.share:call",
        // viz Phase 3 (backend-resolved panel data): `viz.query(panel) -> { frames }` is THE render
        // path every panel now reads through (usePanelData). Member-level like `dashboard.get` — it
        // dispatches each target under `caller ∩ grant` (composing the target tool's own cap), so a
        // token still cannot read a target it lacks. A token without it is refused per verb.
        "mcp:viz.query:call",
        // widget-builder scope (the tool-driven widget builder): the four `template.*` verbs the
        // builder reaches over the `POST /mcp/call` bridge to persist/load durable scripted-view
        // (Plot/D3/JSX) snippets. Member-level — any member may author their own templates
        // (author-ownership decides which *specific* template they may update/delete). A token without
        // a given verb is refused server-side per verb (the deny-per-verb test).
        "mcp:template.save:call",
        "mcp:template.get:call",
        "mcp:template.list:call",
        "mcp:template.delete:call",
        // rules-workbench scope (the Playground · chain canvas · datasources admin): the shipped
        // `rules.*` / `chains.*` / `datasource.*` verbs the rules/chains/datasources gateway routes
        // check. Member-level — any member may author/run their own rules + chains and (as admin)
        // register datasources over real series (workspace wall + per-source `caps::check` inside a
        // run still decide what a rule may actually read). The gateway re-checks each cap server-side;
        // a token without a given verb is refused per verb (the deny-per-verb test).
        "mcp:rules.run:call",
        "mcp:rules.save:call",
        "mcp:rules.get:call",
        "mcp:rules.list:call",
        "mcp:rules.delete:call",
        "mcp:chains.save:call",
        "mcp:chains.run:call",
        "mcp:chains.get:call",
        "mcp:chains.list:call",
        "mcp:chains.delete:call",
        "mcp:chains.runs.get:call",
        "mcp:datasource.add:call",
        "mcp:datasource.remove:call",
        "mcp:datasource.list:call",
        "mcp:datasource.test:call",
        "mcp:federation.query:call",
        // `datasource.add` mediates the DSN into lb-secrets under the caller's authority — the host
        // requires this secret-write grant alongside `mcp:datasource.add:call` (federation/add.rs).
        // Member-level for the dev login so the datasources admin page's Add actually persists.
        "secret:federation/*:write",
        // rules-workbench: the `rules.*`/`chains.*` verbs add a defense-in-depth Store-surface check
        // (`store:rule:*` / `store:chain:*`) BELOW the MCP gate — unlike dashboard, which gates on MCP
        // + the S4 edges only. The dev member needs the store grants so the Playground/canvas
        // save/get/list/delete actually persist over the live gateway (mirrors `store:doc/*` above).
        "store:rule:read",
        "store:rule:write",
        "store:chain:read",
        "store:chain:write",
        // coding-workflow scope: the `workflow.*` verbs the approval-gate routes check
        // (`POST /approvals/{id}/request|resolve|start`). The dev member can open an approval,
        // resolve it, and start the gated coding job from the browser; the gateway re-checks each
        // cap server-side (the S6 approval gate itself is enforced regardless of caps). A token
        // WITHOUT these is still refused server-side (workflow_verb_without_the_cap_is_denied).
        "mcp:workflow.request_approval:call",
        "mcp:workflow.resolve_approval:call",
        "mcp:workflow.start_job:call",
        // agent-run scope Part 2: the per-tool-call human gate. `agent.decide` first-settles a
        // suspended tool call (member-level — the same authority that resolves the surfaced inbox
        // item). `agent.policy.set` edits the ws Allow/Deny/Ask policy — an ADMIN act (who-may-run-
        // what), so it rides the workspace-admin role the dev principal already holds, NOT the bare
        // member set. The gateway re-checks each cap server-side (a token without it is refused).
        "mcp:agent.decide:call",
        "mcp:agent.policy.set:call",
        // agent-run scope Part 3: `agent.watch` gates the live `RunEvent` SSE feed (`GET
        // /runs/{job}/stream`). Read-only on the run; checked inside `watch_run` (a `403` before any
        // stream body). Member-level — observing a run is not an admin act.
        "mcp:agent.watch:call",
        // agent-run scope Part 5: model-activated skills. `skill.activate` is a LOOP-INTERNAL tool
        // (the loop intercepts the model's proposed call and loads the body under the S4 grant gate),
        // so the dev principal does not strictly need this to drive the loop. It is granted for the
        // catalog/activation surface symmetry and so a future direct-MCP route is reachable; the S4
        // skill GRANT remains the real wall (an ungranted skill is denied regardless of this cap).
        "mcp:skill.activate:call",
        // files/skills scope: the shared-asset surface caps the doc/skill routes check directly
        // (`authorize_doc`/`authorize_skill` gate on `store:doc/{id}` / `store:skill/{id}`, NOT an
        // MCP verb). The dev member may put/get/share/link their docs and manage skills; gate 3
        // (membership/ownership) still decides which *specific* asset they may read. `add_team_member`
        // is gated by `store:doc/*:write` (an admin act at S4), so the dev admin can populate teams.
        "store:doc/*:read",
        "store:doc/*:write",
        "store:skill/*:read",
        "store:skill/*:write",
        // api-keys scope: the management verb gate, plus the built-in role cap bundles. The dev admin
        // HOLDS the read-only/read-write cap sets so the no-widening guard lets it mint keys under
        // either built-in role (a key created by this admin never widens beyond it). The write role's
        // caps are action-named (not `*.*`) so a data key can never reach `apikey.manage`.
        "mcp:apikey.manage:call",
        "store:*:read",
        "store:*:write",
        "mcp:*.get:call",
        "mcp:*.list:call",
        "mcp:*.write:call",
        "mcp:*.create:call",
        "mcp:*.update:call",
        "mcp:*.delete:call",
        "mcp:*.post:call",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Build the claim set for `user` logging in to `workspace`, valid for `ttl` seconds from `now`.
/// Real signed claims — only the *credential check* (here, "any user, any workspace") is the
/// dev-login stand-in. The workspace becomes the token's hard wall (§7).
pub fn dev_claims(user: &str, workspace: &str, now: u64, ttl: u64) -> Claims {
    Claims {
        sub: user.to_string(),
        ws: workspace.to_string(),
        role: Role::Member,
        caps: member_caps(),
        iat: now,
        exp: now.saturating_add(ttl),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Datasources page drives `federation.query`/`datasource.test`, which dispatch to the
    /// federation sidecar via `call_sidecar` (gated `mcp:native.call:call`) after mediating the DSN
    /// (`secret:federation/*:write` for Add). A dev login missing ANY of these silently 500s the live
    /// page even though the route-level cap is present — the end-to-end gap this guards against.
    #[test]
    fn dev_login_carries_the_full_datasources_chain() {
        let caps = member_caps();
        for needed in [
            "mcp:datasource.add:call",
            "mcp:datasource.list:call",
            "mcp:datasource.test:call",
            "mcp:federation.query:call",
            "mcp:native.call:call",
            "secret:federation/*:write",
        ] {
            assert!(
                caps.iter().any(|c| c == needed),
                "dev login must grant {needed} for the Datasources page to work end to end"
            );
        }
    }
}
