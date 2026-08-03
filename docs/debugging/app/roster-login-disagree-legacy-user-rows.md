# The People roster lists a member the login path then refuses ("not a member of any workspace")

- Area: app / auth-caps
- Status: resolved
- First seen: 2026-08-03
- Resolved: 2026-08-03
- Session: ../../sessions/auth-caps/legacy-removal-sweep-session.md
- Regression test: `rust/crates/host/tests/identity_membership_test.rs::roster_and_login_path_agree_on_the_one_membership_source`
- Twin (same class, prior instance): [bare-login-handle-not-a-member](bare-login-handle-not-a-member.md)

## Symptom
In `acme`, `GET /admin/members` returned `user:ap` — the People tab showed the person on the
roster — but `GET /admin/identities/user:ap/workspaces` returned `[]`, and `/auth/login` refused
that person with **403 "not a member of any workspace"**. Two admin reads of "who belongs to this
workspace", disagreeing, on the same store, at the same moment.

## Reproduce
1. Create a person through the legacy admin path: `user.create(ws, "ap", …)` (or `POST /admin/users
   {user:"ap"}`), which is what the old admin console did.
2. `GET /admin/members` → the roster contains `user:ap`.
3. `GET /admin/identities/user:ap/workspaces` → `[]`.
4. `POST /auth/login {email of ap, …}` → `403 "not a member of any workspace"`.

## Investigation
The roster and the login path were reading **two different sources**, and the legacy one was keyed
differently in each reader:

- `user_create` (`host/src/users/create.rs:32`) wrote the legacy row under the **BARE** handle:
  `write(store, ws, "user", "ap", { user: "ap", … })`.
- `membership_list`'s legacy pass (`host/src/membership/list.rs:37-55`) **listed** that table and
  synthesized the sub from the row's `user` FIELD: `format!("user:{u}")` → `user:ap`. It matched.
- `is_effective_member` (`host/src/identity/workspaces.rs:59-74`) did a **keyed point read** with
  the already-prefixed sub: `read(store, ws, "user", "user:ap")` → the key is `ap`, so it missed.

So the two legacy fallbacks *of the same union* did not even agree with each other: one read the
field, one read the key. `has_any_effective_member` (`:79-91`) was a third variant — a list, like
`membership_list` — which meant a workspace populated only by legacy rows could read "empty" to one
caller and "populated" to another.

The one-line fix (read the key the way the write wrote it) was available, and rejected: it would
have preserved two sources of truth for "who belongs", which is what makes this class of bug
possible at all. This is the **second** instance of that class — the first
([bare-login-handle-not-a-member](bare-login-handle-not-a-member.md)) was the same shape one layer
up: a handle that was canonical in one code path and raw in another, so a person existed twice.
A second instance of a class is the signal to delete the class, not to patch the instance.

## Root cause
Global-identity decision #10 chose a **lazy migration**: a legacy per-workspace `user:*` row with no
`membership` row would count as an implicit membership, so an upgraded workspace kept its people.
That decision created a permanent second source of truth for membership, reachable by two different
keys, in three functions written at three different times. It was correct for a product with
deployed data to migrate. lb has none — it is pre-production.

## Fix
Deleted the legacy source outright (the removal sweep `email-login-scope.md` §Sequencing had already
tracked), rather than aligning the key:

- **Deleted** `crates/host/src/users/` entirely (`user.create`/`list`/`disable`/`enable`/`delete`,
  the `UserRecord`, `user_login_check`) and its MCP bridge; the `/admin/users*` gateway routes; the
  `mcp:user.manage:call` / `mcp:user.disable:call` caps.
- **Deleted** `POST /login` and `membership_login_resolve`. `/login` was independently an
  **account-enumeration oracle** (it resolved membership at `login.rs:84` BEFORE checking the
  credential at `:97`, so an unauthenticated caller could distinguish member/non-member/disabled
  with a garbage password), and `membership_login_resolve`'s bootstrap-on-empty would have become a
  self-promotion hazard the moment legacy rows stopped counting toward `has_any_effective_member`.
  `/auth/*` is the only human door; machines carry an lb API key.
- **Collapsed** `membership_list` to the `membership` rows and `is_effective_member` to
  `membership_is_member`; deleted `has_any_effective_member` and the per-workspace disable filter in
  `login_workspaces`.
- The first admin is now **explicitly provisioned** by the operator (`seed_dev_identity` at boot, or
  identity → password → `create_workspace`), which is what `email-login-scope.md` already specified.

## Behaviour deliberately lost
**Per-workspace disable/enable is gone.** There is no membership-row equivalent of `active=false`.
The nearest surviving control is `membership.remove`, which is strictly stronger: it tombstones the
row, revokes the subject's grants, and marks the live token. Nothing silently no-ops — the verbs,
the caps, and the routes were removed, so a caller gets a 404/unknown-tool, not a quiet success. If
a real "suspend without revoke" requirement appears, it belongs on the membership row as its own
scoped change, not as a resurrected parallel record.

## Verification
- New regression test asserts the invariant from **both** directions on a real `mem://` store:
  after `membership_add(ws, sub)`, `membership_list(ws)` contains `sub` AND `identity_workspaces(sub)`
  / `login_workspaces(sub)` contain `ws`; for a sub with no membership row, all three are empty; and
  `membership_remove` drops it from both in the same step.
- `cargo test -p lb-host`, the gateway integration targets, and `cargo test -p lb-node` green
  (excluding the pre-existing reds noted in the session log).

## Prevention
There is now exactly ONE membership record. The People tab, `identity.workspaces`, and the login
path all read `membership:{sub}` keyed by the canonical `user:<name>` sub, so they cannot disagree —
and the regression test asserts the agreement rather than either side's behaviour in isolation.
Lesson (the same one the twin taught, now enforced structurally): a "lazy migration" that lets an
old record imply a new one creates a second source of truth whose two readers will drift on the
key. In pre-production there is nothing to migrate — delete the old record and keep one source.
