//! WHICH CAPABILITY a host-native verb actually gates on — the cap-ALIAS table, in one file.
//!
//! Usually a verb gates on its own name. The exceptions are verbs that deliberately ride an EXISTING
//! grant: the same privilege, no new cap, re-checked inside the verb. This is the ONE place that
//! mapping lives, and it has three callers — the dispatcher's outer gate, the `tools.catalog`
//! visibility gate, and the stale-grant repair — because the catalog's cardinal rule ("advertise a
//! tool only if the call would allow it, never hide one that would pass") holds only if all of them
//! ask the same question.
//!
//! **Why a missing arm is so expensive.** A verb whose namesake cap exists in no role bundle is
//! unreachable for EVERY caller, admins included, the moment it ships — and the tests that would
//! catch it are the ones nobody writes, because a direct `call_*_tool` test never crosses this gate.
//! `media.upload_*` and `series.retention.delete` both shipped in that state and were found on a
//! live node. Every arm below carries the reasoning that put it there, deliberately, so the next
//! person adding a verb sees the shape of the mistake before making it.
//!
//! Split out of `tool_call.rs` (FILE-LAYOUT): the dispatcher answers "who runs this verb", this file
//! answers "what may reach it", and the table's documentation had grown to a third of that file.

use crate::tool_call::is_host_native;

/// The capability a call to `qualified_tool` actually gates on. Usually the tool's own name; the
/// exceptions are verbs that deliberately ride an EXISTING grant (same privilege, no new cap).
/// ONE mapping, two callers: the dispatcher's outer gate ([`call_tool_at_depth`]) and the
/// `tools.catalog` visibility gate — the catalog's cardinal rule ("advertise a tool only if the
/// call would allow it, never hide one that would pass") only holds if both consult the same alias.
///
/// - `federation.schema` / `federation.sample` — the no-SQL discovery verb and the AI-context
///   snapshot are the SAME read privilege as a live query (datasources-ux / datasource-samples
///   scopes): both gate under `mcp:federation.query:call`, the cap their service layer re-checks.
///   Without the alias the gate demanded a per-verb grant no role carries (the Datasources browse
///   panel was denied opaquely; the palette hid the verbs from callers who could run them).
/// - `outbox.enqueue_held` — rules-approvals scope: staging a gated effect is the SAME authority as
///   staging any effect (the *release* on approval is the gated step), so it rides
///   `mcp:outbox.enqueue:call`; no `enqueue_held` cap exists. The host fn re-checks inside.
/// - `telemetry.*` — telemetry-console scope: the read verbs (query/trace/tail) collapse onto the
///   ONE `mcp:telemetry.read:call` grant; purge keeps `mcp:telemetry.purge:call`. Re-checked inside.
/// - `nav.pref.*` — nav scope: the member-owned active pick gates on the SAME `mcp:nav.resolve:call`
///   read grant its verb re-checks; curating which nav you use is part of resolving your own menu.
/// - `nav.set_default` — nav scope: the workspace-default pointer is an authoring action — it gates
///   on the `mcp:nav.save:call` grant that creates the navs it points at. Re-checked inside.
///   The capability name the dispatcher's gate will ACTUALLY check for `qualified_tool`, across both
///   tiers — a host-native verb resolves through the [`gate_tool_for`] alias table, while an
///   `<ext>.<tool>` is gated on its own qualified name by `lb_mcp`'s `authorize`.
///
/// It exists so the stale-grant repair asks the gate's own question rather than a near-miss of it.
/// Getting this wrong is silent in the worst way: ask about the wrong cap and the repair either
/// never fires (the bug stays) or fires on a verb the caller still cannot reach (a wasted store read
/// and a `Some` that the gate immediately rejects). The `format.`/`convert.` tier is deliberately
/// absent — it is grant-free and returns before this is ever reached.
pub(crate) fn gate_for(qualified_tool: &str) -> &str {
    if is_host_native(qualified_tool) {
        gate_tool_for(qualified_tool)
    } else {
        qualified_tool
    }
}

pub(crate) fn gate_tool_for(qualified_tool: &str) -> &str {
    if qualified_tool == "federation.schema"
        || qualified_tool == "federation.sample"
        // datasource-profile scope: a profile is strictly LESS than what the read cap can already
        // SELECT, so reading/computing one is the same read privilege — no new grant. (Only
        // `federation.profile_refresh` is separately capped: it SPENDS external-DB work on demand.)
        || qualified_tool == "federation.profile"
        || qualified_tool == "federation.profile_get"
    {
        "federation.query"
    } else if qualified_tool == "identity.set_credential"
        || qualified_tool == "identity.set_email"
        || qualified_tool == "identity.set_password"
    {
        // login-hardening + email-login scopes: setting a user's (per-ws) credential, GLOBAL password,
        // or email is the SAME admin authority as managing identities — all ride the existing
        // `mcp:identity.manage:call` grant (the scope's MCP §6.1 decision), not a new per-verb cap.
        // Each verb re-checks `identity.manage` inside.
        "identity.manage"
    } else if qualified_tool == "outbox.enqueue_held" {
        // rules-approvals scope: staging a GATED effect is the same authority as enqueuing an
        // ordinary one — the hold is a delivery decision, not a second privilege — so it rides
        // `mcp:outbox.enqueue:call`, which the host fn re-checks inside.
        //
        // RESTORED. This arm existed, was removed, and no `mcp:outbox.enqueue_held:call` exists in
        // any role bundle — so the outer gate demanded a cap nobody can hold and every held-effect
        // stage answered a bare `denied`, for admins too. The whole approvals surface was
        // unreachable: `approval_release_test` fails 7/7 on master with "stage held effect: Denied".
        //
        // The doc comment at the head of this function still describes the alias, which is what
        // makes the removal read as accidental rather than intended.
        //
        // Note the same commit ADDED this exact arm for `media.upload_begin`/`chunk_write`/
        // `upload_commit`, with a comment describing the identical failure ("the entire upload
        // surface was unreachable for every caller, admins included"). Same trap, same commit,
        // opposite directions — which is the argument for the alias table being the ONE place this
        // mapping lives, and for a green suite that fails the moment an arm goes missing.
        "outbox.enqueue"
    } else if qualified_tool == "viz.query_batch" {
        // dashboard-query-acceleration scope, slice 3: the batch fan-in is a fan-in of the SAME
        // authorized read, not a new privilege — it rides `mcp:viz.query:call`, checked ONCE for the
        // batch (each panel re-checks its own target caps inside the resolver, unchanged). No
        // `mcp:viz.query_batch:call` exists in any role bundle, so without this alias the outer gate
        // would deny the batch verb for every caller — the shipped-but-unusable state the "reuses the
        // grant" clause avoids (same shape as `series.latest_many`).
        "viz.query"
    } else if qualified_tool == "series.latest_many" {
        // series-read-perf scope: a batched fleet-snapshot read is ONE logical read of the
        // series-latest surface, not K grants — it rides the existing `mcp:series.latest:call`
        // grant, checked ONCE for the batch (the inner `authorize_ingest` in ingest/read.rs uses
        // the same `series.latest` cap). No `mcp:series.latest_many:call` exists in any role
        // bundle, so without this alias the outer gate denies the batch verb for every caller —
        // the shipped-but-unusable state the scope's "reuses the grant" clause intended to avoid.
        "series.latest"
    } else if qualified_tool == "series.rollup.read" {
        // Reading a series' STORED rollup rows is the same read privilege as reading the series —
        // it is strictly LESS than what `series.read {mode:"buckets"}` already returns (that read
        // merges these very rows in, re-aggregated). Its service layer authorizes `series.read`
        // accordingly, so without this alias the OUTER gate would demand
        // `mcp:series.rollup.read:call`, which appears in no role bundle — the shipped-but-unusable
        // state `series.retention.delete` landed in and that the aliases here exist to prevent.
        // Confirmed against a live node: the token holds `mcp:series.read:call` and the call still
        // 403'd until this arm existed.
        "series.read"
    } else if qualified_tool == "series.retention.patch"
        || qualified_tool == "series.retention.delete"
    {
        // Both are the SAME administrative privilege as the replacing write — anything `patch` can
        // do, `set` could already do by sending a full body, and `series_retention_delete`'s own doc
        // says outright that "deleting a policy is the same administrative privilege as setting one;
        // no separate cap is minted". Its SERVICE layer re-checks `series.retention.set`
        // accordingly — but the OUTER gate was still demanding `mcp:series.retention.delete:call`,
        // which appears in no role bundle, so **delete has been unreachable for every caller since
        // it shipped**. Found by driving it on a live node; every test called the host fn directly
        // and so never crossed the outer gate. This is the shipped-but-unusable state the
        // `viz.query_batch` / `series.latest_many` aliases below already exist to prevent.
        "series.retention.set"
    } else if qualified_tool == "media.upload_begin"
        || qualified_tool == "media.chunk_write"
        || qualified_tool == "media.upload_commit"
    {
        // media scope: the three phases of ONE upload ride the ONE `mcp:media.upload:call` grant —
        // beginning, writing the bytes, and committing are not three privileges, and each verb's
        // own gate (`begin.rs`, `chunk.rs` via `media_chunk_put`, `commit.rs`) already checks
        // exactly that cap. No `mcp:media.upload_begin:call` / `.chunk_write:call` /
        // `.upload_commit:call` exists in ANY role bundle, so without this arm the OUTER gate
        // demanded a cap nobody can hold and **the entire upload surface was unreachable for every
        // caller, admins included** — while `media.list`/`get`/`read` worked, because their literal
        // names ARE their caps. Found by driving it from an extension page on a live node: the
        // token held `mcp:media.upload:call` and all three verbs still answered a bare `denied`.
        // Same shipped-but-unusable shape as `series.retention.delete` and `viz.query_batch` above.
        "media.upload"
    } else if qualified_tool == "media.list" {
        // `media_list`'s own gate checks `media.get` (see `get.rs`) — the outer gate must ask the
        // same question, or a caller holding `mcp:media.get:call` alone passes here and is denied
        // inside. It works today only because the member bundle happens to grant BOTH.
        "media.get"
    } else if qualified_tool.starts_with("telemetry.") {
        crate::read_or_admin_cap(qualified_tool)
    } else if qualified_tool.starts_with("nav.pref.")
        || qualified_tool == "nav.hidden.get"
        || qualified_tool == "nav.ext_boards.get"
    {
        // hide-and-pins scope: reading the hidden-set is part of resolving one's own menu (the
        // resolver echoes it to every member anyway) — same `mcp:nav.resolve:call` read grant.
        // host-authored-ext-nav-boards scope: the host-authored ext board rows are read for the
        // same reason by the same people — EVERY member's rail renders them, so gating the read on
        // the authoring cap would make an admin-placed board invisible to the members it was placed
        // for. The write below is the privileged half.
        "nav.resolve"
    } else if qualified_tool == "nav.set_default"
        || qualified_tool == "nav.hidden.set"
        || qualified_tool == "nav.order.set"
        || qualified_tool == "nav.ext_boards.set"
    {
        // hide-and-pins scope: curating the workspace hidden-set is the SAME authoring authority as
        // the workspace-default pointer — it rides `mcp:nav.save:call`, no separate cap. Ordering
        // the sidebar is that same curation, over the same record, so it rides the same cap. So is
        // binding a host board into an extension's section (host-authored-ext-nav-boards scope):
        // it is menu authoring over an opaque ref, and `mcp:nav.ext_boards.set:call` exists in NO
        // role bundle — without this arm the verb answers `denied` for every caller, admins
        // included, exactly the shipped-but-unusable shape the media/retention arms above record.
        "nav.save"
    } else if qualified_tool == "grants.revoke" {
        // authz-verbs-mcp-dispatch scope: assign/revoke MUTATE the same grant surface and share the
        // ONE cap `mcp:grants.assign:call` — the verb's inner gate (`authz/grants.rs`) checks that
        // cap for both. No `mcp:grants.revoke:call` exists in any role bundle, so without this alias
        // the outer gate would deny revoke for every caller, admins included.
        "grants.assign"
    } else if qualified_tool == "grants.list_scoped" {
        // authz-verbs-mcp-dispatch scope: listing scoped grants is the SAME read privilege as
        // `grants.list` — the inner gate checks `mcp:grants.list:call`; no per-verb cap exists.
        "grants.list"
    } else if qualified_tool == "teams.create" {
        // authz-verbs-mcp-dispatch scope: the inner gate + admin role bundle use
        // `mcp:teams.manage:call` (there is no `mcp:teams.create:call`); align the outer gate.
        "teams.manage"
    } else if qualified_tool == "update.status"
        || qualified_tool == "update.check"
        || qualified_tool == "update.history"
        || qualified_tool == "update.credential.status"
    {
        // node-update scope: the family's three grants split by BLAST RADIUS, not by verb — reading a
        // version is not applying one, and applying one is not holding the backend's credential. So
        // eight verbs collapse onto three caps, and THIS table is the only place that collapse is
        // expressible: via a tool registry every tool would gate on its own literal name (scope
        // decision 7), which is exactly the shipped-but-unusable state the aliases above exist to
        // prevent. Each verb re-checks the same cap inside itself, so the two gates agree.
        crate::update::READ_CAP
    } else if qualified_tool == "update.apply" || qualified_tool == "update.rollback" {
        crate::update::APPLY_CAP
    } else if qualified_tool == "update.credential.set"
        || qualified_tool == "update.credential.claim"
    {
        // Documented as equivalent to BACKEND ADMIN (§Risks): lb cannot narrow a backend's
        // credential, so the grant's weight is made visible rather than quietly bundled.
        crate::update::CREDENTIAL_CAP
    } else if qualified_tool == "roles.delete" {
        // authz-verbs-mcp-dispatch scope: deleting a role is the SAME authority as defining/managing
        // one — the inner gate checks `mcp:roles.manage:call`; no `mcp:roles.delete:call` exists.
        "roles.manage"
    } else {
        qualified_tool
    }
}

#[cfg(test)]
mod media_gate_tests {
    use super::gate_tool_for;

    /// **THE REGRESSION**: the outer gate asked for a cap that exists in no role bundle, so the
    /// whole upload surface was unreachable for every caller while the read verbs worked. Each of
    /// these three re-checks `media.upload` INSIDE itself (`begin.rs`, `chunk.rs`, `commit.rs`);
    /// the outer gate must ask the same question or the two gates disagree and the strictest wins.
    #[test]
    fn the_upload_phases_ride_the_one_upload_cap() {
        for verb in [
            "media.upload_begin",
            "media.chunk_write",
            "media.upload_commit",
        ] {
            assert_eq!(
                gate_tool_for(verb),
                "media.upload",
                "{verb} must gate on mcp:media.upload:call — no per-phase cap is minted"
            );
        }
    }

    /// `media_list` checks `media.get` inside (`get.rs`), so the outer gate must too — otherwise a
    /// caller holding only `mcp:media.get:call` passes the outer gate and is denied within.
    #[test]
    fn list_gates_on_the_same_cap_its_body_checks() {
        assert_eq!(gate_tool_for("media.list"), "media.get");
    }

    /// The verbs whose literal name IS their cap must stay unaliased — over-aliasing would widen
    /// `read`/`delete` onto a grant their bodies never check.
    #[test]
    fn the_self_named_media_verbs_are_untouched() {
        for verb in ["media.read", "media.get", "media.delete"] {
            assert_eq!(gate_tool_for(verb), verb);
        }
    }
}

#[cfg(test)]
mod ext_boards_gate_tests {
    use super::gate_tool_for;

    /// The two host-authored-ext-nav-boards verbs ride EXISTING nav caps — the read with every
    /// member's `nav.resolve`, the write with the admin's `nav.save`. No `nav.ext_boards.*` cap is
    /// minted, so without these aliases the outer gate would deny both for every caller while the
    /// direct-call tests stayed green (they never cross this gate).
    #[test]
    fn the_ext_board_verbs_ride_the_existing_nav_caps() {
        assert_eq!(gate_tool_for("nav.ext_boards.get"), "nav.resolve");
        assert_eq!(gate_tool_for("nav.ext_boards.set"), "nav.save");
    }

    /// The read must NOT ride the authoring cap: a board an admin places is rendered in EVERY
    /// reached member's rail, so gating its read on `nav.save` would make the feature invisible to
    /// exactly the people it exists for.
    #[test]
    fn the_read_is_member_level_not_admin() {
        assert_ne!(gate_tool_for("nav.ext_boards.get"), "nav.save");
    }
}
