# Workspace scope — atomic provision (a new workspace with its first admin, in one verb)

Status: **IMPLEMENTED on master, 2026-07-30 — unreleased** (needs the next `node-v*` tag; the
rubix-ai consumer is built against a local `[patch]`). See
`../../sessions/workspace/workspace-provision-session.md` for what shipped and the answers to the
blocking questions (short form: `write_batch` gives a real single-namespace transaction, there is NO
explicit flush point — flagged in `../store/store-scope.md` — so the delivered shape is atomic
in-namespace bootstrap + directory row LAST + reconcile). Promotes to `public/workspace/` once
tagged.

> Read with: `workspace-scope.md` (**shipped** — the directory record, `_lb_workspaces`, and the
> `workspace_create` this scope repairs), `../auth-caps/global-identity-scope.md` (**shipped** —
> identities + `membership.add`, and decision #3, the first-member bootstrap this scope makes
> atomic), `../auth-caps/invites-scope.md` (the email/credential half of onboarding — this scope
> deliberately does **not** duplicate it), `../auth-caps/admin-crud-scope.md` (the lifecycle verbs
> a half-provisioned workspace currently escapes), `../auth-caps/login-hardening-scope.md`
> (`CredentialCheck`, the seam any credential work must land behind).

Creating a workspace today is four independent, best-effort writes behind one verb. `workspace_create`
writes the directory row, then *separately* adds the creator's membership, grants `role:member` and
`role:workspace-admin`, seeds the built-in role records, and grants the default core skills — each with
its error discarded (`let _ = …`). The doc comment promises "a brand-new workspace always has exactly one
admin and is never orphaned"; the code cannot keep that promise, because any interruption between the
directory write and the membership write produces exactly the thing the promise forbids: **a workspace
that exists, has no members, and is therefore unreachable by anyone — including the admin who made it,
and including every lifecycle verb that could clean it up.** Worse, the verb still returns `Ok(record)`,
so the caller is told it succeeded.

This was hit for real: a rubix-ai setup wizard created a workspace, the node restarted before the
segment was durable, and the result was a directory-less namespace plus a permanent
`not a member of that workspace` on every attempt to enter it — with no route to list, fix, or delete it.

We want **one transactional provision verb** that either yields a complete, enterable workspace — row,
membership, role grants, skill grants — or yields nothing and says so. And we want it to be callable
**for a target workspace without re-minting the caller's session**, so a multi-workspace admin can set
one up without being thrown out of the one they are working in.

## Goals

- **`workspace.provision` — one verb, one outcome.** Takes `{ ws, name, admin?, skills? }`; performs the
  directory row, the first-member membership, the built-in role seed, the admin/member role grants, and
  the default core-skill grants as **one unit**. Returns the complete `WorkspaceRecord` plus what was
  bootstrapped (`{ record, admin_sub, roles_granted, skills_granted }`) so the caller can show truth
  rather than assume it.
- **All-or-nothing, and honest when it is nothing.** If any constituent write fails, the verb fails with
  a typed error and leaves **no** partially-provisioned workspace. No `let _ =`. A provision that cannot
  bootstrap an admin is a provision that did not happen.
- **Durability before success.** The verb does not return `Ok` until the writes are durable (flushed to
  the commit log / manifest), because "reported created, lost on restart" is the exact failure that
  motivated this scope. State the flush point explicitly rather than inheriting whatever the store
  happens to do.
- **`admin` may be someone other than the caller.** `admin: Option<Subject>` — defaults to the caller
  (preserving today's decision #3 bootstrap), or names a different existing identity as the first admin.
  This is what lets an operator stand a workspace up *for* somebody.
- **Target-workspace authorization without a re-mint.** The verb is gated by
  `mcp:workspace.provision:call` against the **caller's own** workspace (as `workspace_create` is
  today) — the new workspace is the *object*, never the authorization context. No `auth.switch` is
  required to provision, so the caller's session is untouched.
- **Idempotent re-provision, tombstone-respecting.** Re-provisioning an existing active workspace is a
  no-op that returns the current record (never a silent re-bootstrap that would undo a later admin
  revoke — the trap the current code's early-return exists to avoid). A purged tombstone still wins and
  is never resurrected.
- **A repair path for what already exists.** `workspace.reconcile(ws)` — admin-gated — re-runs the
  bootstrap for a workspace whose directory row exists but whose membership is empty, and reports what it
  fixed. Orphans created before this ships must be recoverable without a store wipe.
- **`workspace_create` keeps working.** It becomes a thin call into provision with `admin = caller` and
  default skills, so existing callers (the UI switcher's "add workspace") inherit atomicity for free. Its
  best-effort internals are deleted, not kept alongside.

## Non-goals

- **No credential, email, or password.** Setting an identity's email/password is
  `auth-caps/invites-scope.md`'s territory (`invite.accept` → the `CredentialCheck` seam) and
  `login-hardening-scope.md`'s seam. This scope provisions a *workspace* and names an *existing* identity
  as its admin. Provisioning a person who does not exist yet is an invite, and the two compose:
  provision the workspace, then invite the admin into it. **Do not add a `password` field to this verb** —
  it would fork the credential path that invites owns.
- **No general cross-workspace admin.** A header/param that lets *every* admin verb act on another
  workspace (`X-LB-Target-Workspace`) is a much larger change to the caps model and deserves its own
  scope. This verb takes `ws` as an object because provisioning inherently has no session in the target
  yet; that argument does not generalise to `user.create`.
- **No data-namespace provisioning.** A workspace's namespace still springs into existence on first
  write (`workspace-scope.md`). This verb makes a workspace *listable and enterable*, not pre-populated.
- **No UI.** The consumer surface is `NubeIO/rubix-ai` — see its
  `docs/scope/frontend/workspace-provision-scope.md`.
- **No change to the tenant wall.** Workspace data reads still use the token's workspace. Provision
  writes only the reserved `_lb_workspaces` directory row plus rows *inside* the target namespace it is
  creating; it never reads across a wall.

## Intent / approach

**The shape:** collapse the four best-effort writes into one host verb with a single failure domain, and
make the bootstrap a precondition of success rather than a convenience afterthought.

Today's control flow in `rust/crates/host/src/workspaces/create.rs` is: authorize → tombstone check →
write directory row → *return early if already a member* → seed roles → add membership → grant roles →
grant skills → `Ok`. Every step after the directory write is `let _ =`. The rewrite inverts the priority:
assemble the full write set, apply it, flush, and only then report success — mapping any failure to
`WorkspacesError::ProvisionFailed { stage }` so the caller learns *where* it broke.

**Atomicity, concretely.** The store is a commit log, not a transactional DB, so "atomic" here means a
**single batched append plus flush**, not a distributed transaction: build every row (directory record,
membership, role records, grant edges, skill grants) in memory, write them as one batch, flush, and treat
a failure at any point as "nothing was provisioned" — with the directory row written **last** in the
batch so a torn write can never leave a listable-but-memberless workspace. That ordering is the crux:
today the directory row goes first, which is precisely why the observed orphan was listable-then-lost
rather than simply absent. If the store cannot express a batch of this shape, that is a finding for
`../store/` and the ordering guarantee alone still removes the orphan class.

**Rejected alternative — "just fix the `let _ =`s."** Propagating those errors makes failures loud but
still leaves a half-written workspace behind on the failure path, requiring the caller to clean up a
state that has no cleanup verb (an unlisted workspace is invisible to `workspace.archive`/`purge`). Loud
partial failure is better than silent partial failure but still wrong; the caller wants a boolean.

**Rejected alternative — provision by switching the session, then using existing verbs** (what the
rubix-ai wizard does today). It requires an `auth.switch` mid-flow, evicts the operator from their own
workspace, needs a switch-back on cancel, and is still non-atomic — six sequential HTTP calls, any of
which can leave the workspace half-built. It also cannot provision *for* another admin. This is the
status quo the scope replaces.

**Rejected alternative — the general `X-LB-Target-Workspace` header.** Strictly more powerful and the
likely long-term direction for cross-workspace administration, but it re-opens the caps-evaluation
context for every verb (which workspace is a cap checked against?) and is a much larger blast radius than
this fix needs. Kept as an explicit non-goal and a future scope.

## How it fits

- **Isolation / tenancy.** Unchanged and load-bearing. The directory row lives in the reserved
  `_lb_workspaces` namespace (node-level metadata, not tenant data); membership/grant/skill rows live
  *inside* the new workspace's namespace. The caller's token still names their own workspace; `ws` is an
  object identifier, never an authorization context. No verb starts reading request-body workspaces.
- **Capabilities & the deny path.** New cap `mcp:workspace.provision:call`, bundled into
  `role:workspace-admin` (alongside the existing `workspace.create`); `workspace.reconcile` gated by the
  same. Deny is the ordinary `authorize_tool` refusal, tested: a `role:member` token calling provision
  gets `Denied` and **no** directory row is written (a denied provision must not leave residue — the same
  invariant as a failed one).
- **Placement (FILE-LAYOUT).** `rust/crates/host/src/workspaces/` is already a folder-of-verbs; add
  `provision.rs` (the verb), `reconcile.rs` (the repair verb), and `bootstrap.rs` (the shared write-set
  builder both call, so create/provision/reconcile cannot drift). `create.rs` shrinks to a delegation.
  Each stays well under the 400-line hard limit.
- **The API/MCP surface.** MCP is the contract: `workspace.provision` and `workspace.reconcile` as MCP
  tools, exposed over the gateway as `POST /workspaces/{ws}/provision` and
  `POST /workspaces/{ws}/reconcile`. Shape is **command**, not CRUD — provision is a single
  transactional action with a report-shaped reply, so it is a POST returning the outcome, not a PUT on a
  resource.
- **Data.** No new record types. `Membership`, `WorkspaceRecord`, the role records, and the grant edges
  are exactly today's shapes; only the write grouping, ordering, and durability change. That is
  deliberate — a pure atomicity fix should not migrate data.
- **Motion.** None. Provision is state, not motion; no outbox effect. (Invites, which *do* have motion,
  own the email path.)
- **Secrets.** None — no credential handled here (see non-goals).
- **Symmetric nodes.** No role branch; provision behaves identically on a gateway, a reactor, or an
  embedded node. No `if cloud`.
- **Rule 10.** No extension is named or special-cased. The default core-skill set stays the existing
  compiled-in list, still widenable by the binary via `LB_DEFAULT_CORE_SKILLS` at the boot boundary.
- **Rule 9 (no mocks).** Tests boot the real store (`mem://`) and the real gateway; the crash-durability
  case uses a real on-disk store, not a fake that simulates a torn write.

## Example flow

An operator in `nube` stands up `other-ws` with `alice` (an existing identity) as its admin:

1. Operator's token: `sub = user:test`, `ws = nube`, holding `role:workspace-admin`.
2. `POST /workspaces/other-ws/provision  { "name": "Other Co", "admin": "user:alice" }`.
3. Gateway authenticates, then `authorize_tool(principal, principal.ws() /* nube */,
   "workspace.provision")` → allowed.
4. Tombstone check on `_lb_workspaces/workspace/other-ws` → not purged. Existing-active check → absent, so
   this is a genuine first provision.
5. Build the write set: built-in role records for `other-ws`; `Membership { sub: "user:alice" }` in `other-ws`;
   grants `role:member` + `role:workspace-admin` to `user:alice`; default core-skill grants; and **last**,
   the `_lb_workspaces` directory row.
6. Apply as one batch, then flush. Any failure → `ProvisionFailed { stage }`, nothing left behind, and
   `other-ws` does not appear in `workspace_list`.
7. Reply: `{ record: {ws:"other-ws", name:"Other Co", status:"active", …}, admin_sub:"user:alice",
   roles_granted:["role:member","role:workspace-admin"], skills_granted:[…] }`.
8. **Test's session is untouched** — still `nube`, no re-mint, no switch-back needed.
9. Alice logs in: `login_workspaces` finds the `other-ws` membership, so `other-ws` is in her roster and
   `auth.switch`/`auth.select` into it succeeds. She is its admin.
10. Repair case: for a `other-ws` orphaned by the *old* code path,
    `POST /workspaces/other-ws/reconcile` re-runs step 5's bootstrap and reports
    `{ fixed: ["membership", "role_grants"] }`.

## Testing plan

Mandatory categories that apply: **capability-deny**, **workspace-isolation**, **offline/sync**
(durability across restart). Hot-reload: N/A.

- **Capability-deny** — a `role:member` token calling `workspace.provision` gets `Denied`; assert
  afterwards that `workspace_list` does **not** contain `ws` and the namespace has no membership row
  (denied ⇒ zero residue). Same for `reconcile`.
- **Atomicity / no orphans** — inject a failure at each bootstrap stage (membership, role seed, grant,
  skill) against a real store; assert for every one: verb returns `Err`, `workspace_list` omits `ws`, and
  `login_workspaces(creator)` does not list `ws`. This is the regression test for the observed bug.
- **Durability across restart** — provision on a real on-disk store, drop the node **without a clean
  shutdown**, reboot, then assert the workspace is listable *and* enterable. This is the exact scenario
  that produced the orphan; it must fail before the fix and pass after.
- **The orphan is now impossible, and the old one is repairable** — hand-craft the broken state (directory
  row, no membership), assert `auth.switch` fails with `not a member of that workspace` (documenting the
  symptom), then `reconcile` and assert the switch succeeds.
- **Admin-other-than-caller** — provision with `admin = user:alice`; assert alice is a member with both
  roles, that the caller (`test`) is **not** silently a member of `other-ws`, and that test's session still
  names `nube`.
- **No re-mint** — assert the provision response carries no token and the caller's token is byte-identical
  before and after (the "does not force the user to leave the workspace" guarantee, asserted not assumed).
- **Idempotency & tombstones** — re-provision an active workspace ⇒ returns the current record, grants
  unchanged (specifically: a previously *revoked* `role:workspace-admin` is **not** re-granted).
  Provision over a purged tombstone ⇒ refused, no resurrection.
- **Workspace-isolation** — provisioning `other-ws` writes nothing readable in `nube`; a `other-ws` membership
  grants no `nube` cap; cross-namespace reads stay absent.
- **`workspace_create` parity** — the existing create-then-switch path (the UI switcher) still works
  end-to-end and now inherits atomicity: the same failure injection that orphaned a workspace before now
  leaves none.

## Risks & hard problems

- **The store may not offer a real batch/flush primitive.** The whole atomicity claim rests on grouping
  writes and flushing before `Ok`. If `lb_store` cannot express that, the honest fallback is **write
  ordering** (directory row last) plus reconcile — which removes the *unreachable-orphan* class but not
  every torn intermediate. Settle this first; it decides whether the goal is "atomic" or
  "ordered + repairable". Flag to `../store/` if the primitive is missing.
- **Flushing on every provision costs latency.** Acceptable: provision is rare and correctness-critical.
  Do not extend the flush to hot paths.
- **`reconcile` is a privileged repair verb.** It writes memberships and grants into a workspace the
  caller may not be in. Keep it admin-gated, audit-logged, and strictly limited to workspaces whose
  membership set is *empty* — never a tool for adding yourself to a populated workspace. This is the
  sharpest security edge in the scope; review it as such.
- **Existing orphans predate the fix.** Any node that ran the old path may hold unlisted namespaces.
  Reconcile handles the listable-but-memberless case; a namespace with *no* directory row needs a
  `workspace.adopt`-style path or a store-level sweep — call that out rather than pretending reconcile
  covers it.
- **Overlap with invites.** Provision + invite is two calls for the common "new workspace for a new
  person" case. Resist fusing them; the seam is right, and invites already own atomic accept.

## Open questions

1. ~~Does `lb_store` expose a multi-row batched append with an explicit flush point?~~ **ANSWERED
   (2026-07-30):** batch yes (`write_batch`, one namespace, one transaction), flush no — flagged as a
   `../store/` finding; the shipped guarantee is "atomic bootstrap + ordered directory write +
   reconcile".
2. Should `workspace.provision` supersede `workspace.create` in the MCP surface (deprecate create), or
   stay alongside it permanently as the richer verb? Recommendation: keep `create` as the thin
   caller-is-admin default and document provision as the primitive.
3. For a workspace with a directory row but **no** namespace rows at all, is `reconcile` the right verb,
   or does that want a separate `workspace.adopt`?
4. Should `reconcile` be callable by a super-admin only, rather than any `workspace-admin`? (Leaning
   super-admin, given it grants admin into a workspace the caller need not belong to.)
5. Does `admin: Option<Subject>` accept a team subject (`team:ops`) as first admin, or users only?
   Recommendation: users only in this slice — a team as sole admin has no login.

## Related

- `workspace-scope.md` — the shipped directory record, `_lb_workspaces`, and the `workspace_create` this
  scope repairs.
- `../auth-caps/global-identity-scope.md` — identities, `membership.add`, and decision #3 (first-member
  bootstrap) that this scope makes atomic.
- `../auth-caps/invites-scope.md` — the email/credential/onboarding half; composes with provision and
  owns everything this scope declines.
- `../auth-caps/login-hardening-scope.md` — the `CredentialCheck` seam any credential work lands behind.
- `../auth-caps/admin-crud-scope.md` — the archive/purge lifecycle a half-provisioned workspace currently
  escapes.
- Consumer: `NubeIO/rubix-ai` → `docs/scope/frontend/workspace-provision-scope.md` (the setup wizard that
  drives this verb).
