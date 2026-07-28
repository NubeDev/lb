# Auth-caps — wiring the stale-grant repair (session)

- Date: 2026-07-28
- Scope: ../../scope/auth-caps/builtin-role-freshness-scope.md (the sibling problem — see below)
- Stage: post-S10
- Status: done

## Goal

`cargo build` emitted three warnings in `crates/host/src/authz/`:

```
warning: unused import: `refresh_grants_if_denied`   (authz/mod.rs:38)
warning: constant `USER_PREFIX` is never used        (authz/resolve_live.rs:41)
warning: function `refresh_grants_if_denied` is never used (authz/resolve_live.rs:78)
```

They look like tidy-up. They are not: all three are one symptom of a **shipped-but-unreachable
security fix**. `refresh_grants_if_denied` (and its `lb_auth::Principal::with_live_grants`
counterpart) were written, fully documented, and **never connected to a gate**. `USER_PREFIX` reads
as dead only because its sole user is that dead function — so wiring the function clears all three.

## The bug the dead code was written to fix

A session token is a **cached projection** of `resolve_caps`, taken once at login
(`role/gateway/src/session/mint_session.rs`). A grant written afterwards is invisible to it until it
expires — and one such write happens on a completely routine action: `grant_ui_scope_to_admin` runs
on **every extension install** and grants the manifest's `[ui]`/`[[widget]]` scope to
`role:workspace-admin`.

The function's own doc records the live observation: `modbus` 0.1.7 → 0.1.8 added
`device.status`/`point.status`; the install granted both; a freshly minted token carried both; and an
operator session minted minutes earlier was refused both — **every device on the page rendered
`unknown`, for up to the 12h token lifetime, with nothing in any log to explain it.**

This is a *different* staleness from the one `builtin-role-freshness-scope.md` fixes. That scope is
about the stored **role row** going stale against new built-in caps, and its fix
(`LiveBuiltinRoleCaps` / `resolve_caps_live`) **is** wired. This is the **token** going stale against
grants written after it was minted. Same family, different axis, no scope doc of its own.

## What changed

One insertion in `crates/host/src/tool_call.rs::dispatch_at_depth`, immediately before the gates:

```rust
let refreshed =
    crate::authz::refresh_grants_if_denied(&node.store, principal, ws, gate_for(qualified_tool))
        .await;
let principal = refreshed.as_ref().unwrap_or(principal);
```

plus a small `gate_for` helper — the capability the dispatcher's gate will *actually* check across
both tiers (`gate_tool_for(t)` for a host-native verb, `t` itself for an `<ext>.<tool>`).

## Decisions & alternatives

1. **Wire it, don't delete it.** Three ways to clear the warnings: wire the feature, delete the code,
   or `#[allow(dead_code)]` it. Suppressing is dishonest — it hides a real defect behind a pragma.
   Deleting throws away a documented fix for an *observed production bug*, and the function's doc
   specifies its exact call-site contract ("`gate_tool` must be the tool the gate will actually check
   (post-alias)"), which is a function written to be called whose wiring was forgotten. Wiring is the
   only option that makes the warning's disappearance mean something.
2. **One call site, not two.** There are two gates: the host-native one in `dispatch_at_depth`, and
   `lb_mcp`'s own `authorize` inside the extension path. The **observed bug is on the extension
   side**, so wiring only the host-native gate would have left the reported failure unfixed while
   looking done. `dispatch_at_depth` is the one point both tiers pass through, and it holds the
   `&Store` that `lb_mcp` (correctly) does not have — so the repair goes here, once. Rejected:
   inverting the dependency so `lb-mcp` could reach a store; that would put a store read inside the
   pure authorize/resolve/dispatch pipeline for one caller's benefit.
3. **Shadow `principal`, don't just gate on the refreshed copy.** Load-bearing: host verbs re-check
   their own capability *inside* the service. Refreshing only the outer gate would pass it and then
   be denied internally — the call still fails, and the repair looks like it worked. Test 4 exists
   solely to pin this.
4. **`gate_for` as its own named helper.** Asking about a near-miss of the gate's question is silent
   in the worst way: the repair either never fires (bug stays) or fires on a verb the caller still
   cannot reach (wasted store read, `Some` the gate rejects immediately).

## Why this is safe (and where it costs)

The policy lives entirely in `refresh_grants_if_denied`, which is conservative by construction:

- **Cached caps are consulted first**, so an allowed call never touches the store — the hot path
  costs nothing, and the cost lands only on a path that was already about to error.
- **Bounded by a re-login.** Caps are resolved server-side for the caller's own `(sub, ws)`, so the
  widened principal is exactly what logging out and back in would mint. It cannot invent authority.
- **Refuses delegated and run-scoped principals** (`with_live_grants` enforces this itself), so an
  agent can never outgrow the delegation it was created with.
- **Returns `Some` only when the verdict flips**, so a genuine denial denies under the caller's
  original identity with the identical error — the audit line names the caps actually dispatched
  under.
- **Never narrows.** Revocation deliberately rides its own mechanism (the `token_revoke` tombstone
  in `verify_token`, bounded by TTL).

**The cost, stated plainly:** a denied call by a `user:` principal now performs an O(teams) store
read. Denied calls are rare and already error paths, and non-`user:` subjects bail before touching
the store — but an authenticated caller *can* spam denials to force store reads. Judged acceptable
(it is bounded, per-workspace, and already audited), and recorded here rather than left for someone
to discover.

## Tests

`crates/host/tests/stale_grant_repair_test.rs` — real infra (booted node, real `grant_assign` writes,
real `call_tool` dispatch); no hand-built grant rows, so a test cannot pass against a row shape the
resolver would reject.

| test | property |
|---|---|
| `a_grant_written_after_login_reaches_an_already_minted_token` | **the repair** — denied before the grant, allowed after, same stale token |
| `a_caller_with_no_durable_grant_is_still_denied_identically` | **no widening** — bob is denied even though a grant row for that *cap* exists (for ada), so a repair keyed on cap rather than subject would fail here |
| `a_delegated_principal_is_never_rewidened_from_the_grant_store` | **no resurrection** — the human holds the grant durably; the agent delegated without it stays denied |
| `the_refreshed_principal_reaches_the_verbs_own_inner_gate` | the repaired identity survives to the verb's **own** cap re-check, not just the dispatcher's |

```
$ cargo test -p lb-host --test stale_grant_repair_test
test result: ok. 4 passed; 0 failed

$ cargo build --workspace          # ZERO warnings (was 3)
$ cargo fmt --all --check          # clean
$ cargo clippy -p lb-host --lib    # no warning in any touched file
$ cargo test -p lb-host --lib      # 376 passed
$ cargo test -p lb-host --test versions_test --test versions_authz_test --test authz_test \
      --test undo_dashboard_test --test catalog_mcp_test --no-fail-fast   # 8/8/4/8/9 passed
```

**Revert-checked, honestly.** With the wiring removed, `a_grant_written_after_login_…` and
`the_refreshed_principal_reaches_…` go **red**; the other two stay green. That is correct and worth
stating rather than glossing: those two assert *denial*, and denial is also the un-wired behaviour —
they are guards against a future widening regression, not proof that the wiring exists. Only the
first pair pins the feature.

## Debugging

No `debugging/` entry: nothing regressed. The defect was an omission (code never called), not a
misbehaviour, and it is recorded here.

## Public / scope updates

`scope/auth-caps/builtin-role-freshness-scope.md` gains a "Sibling problem" note pointing at this
session — the two staleness axes are easy to confuse, and the scope previously described only one.

## Dead ends / surprises

- **The warnings were the interesting part.** Three "unused" warnings in an auth module were the only
  visible trace of an entire unreachable security fix. Worth remembering the next time a warning
  looks like lint noise: `USER_PREFIX` in particular reads as trivially deletable, and deleting it
  would have quietly cemented the real defect.
- **It arrived in an unrelated commit.** `git log` puts this code in `dd91c9a5` ("added switch,
  sliders writes for extentions"), which has nothing to do with grant freshness — the shape of
  work-in-progress swept into a commit and then forgotten. No scope doc, no session doc, no caller.

## Follow-ups

- The `role/gateway` REST routes call `lb_host::call_tool`, so they inherit the repair for free. The
  few paths that authorize *outside* `dispatch_at_depth` (e.g. route-level pre-checks) do not — worth
  an audit if the same symptom is ever reported against a route rather than a verb.
- STATUS.md updated ✅.
