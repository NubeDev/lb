# Git-sync scope — periodic auto-commit-and-push (no AI)

Status: scope (the ask). Promotes to `public/git-sync/` once shipped.

We want a node to **commit and push a repo on a schedule, unconditionally and without any AI** —
e.g. every 10 minutes, "if the working tree changed, stage everything, commit with a generated
message, and push; if nothing changed, do nothing." The first concrete use is keeping a developer's
working checkout backed up to GitHub on a timer. The mechanism must reuse what the platform already
has (`lb-reminders` for the cron, `lb-jobs` for the durable firing) and a **ported `gh`/`git` CLI
wrapper** as the engine, and it must leave the door open to **more CLI-backed tools the same way**.

## Goals

- A periodic, **AI-free** "commit everything + push" that runs on a cron the operator sets
  (`*/10 * * * *` for every 10 min), is a **no-op when the tree is clean**, and survives a node
  restart (the firing is a durable job, not an in-memory `tokio::interval`).
- **Port `lazybones-gh`** (`/home/user/code/rust/lazybones/crates/lazybones-gh`) into this workspace
  as `rust/crates/gh` (crate `lb-gh`): the thin async wrapper that shells out to the user's already
  authenticated `gh`/`git` (no token handling here). Its `SyncRepo::commit_and_push` already does
  exactly "stage `-A` → commit → push, or `Pushed::Clean` when nothing changed" — that *is* the job
  body.
- A **`git.*` MCP verb family** (folder-of-verbs, mirroring `host/`): `git.commit_push` (the action)
  and `git.status` (the read), each its own file and its own capability, dispatched through the same
  `authorize_tool` chokepoint as every other tool. "Allow for more tools like gh" = this is one verb
  folder backed by one CLI crate; another CLI-backed family is a new folder + crate, never a new
  mechanism.
- **Run the node as a service** so the reminder reactor is actually alive between ticks: ship a
  `systemd` unit for Linux. systemd's role is **process supervision, not scheduling** (see Intent).

## Non-goals

- **No AI / no generated prose commit messages.** The message is a fixed template
  (`chore(autocommit): <n> files @ <ts>`), not an LLM call. The "summarize a diff into a commit" path
  is the unrelated `scripts/pq` prompt tool (`commit` role) — explicitly out of scope here.
- **No merge/conflict resolution.** This is last-writer-wins backup, like `lazybones-gh`'s sync repo.
  Divergence (remote moved ahead) is a loud, retried failure, not a silent merge (see Open questions
  on the optional pre-push `--ff-only` pull).
- **systemd is not the scheduler.** We do not ship a `*.timer` that wakes a oneshot CLI every 10 min
  (rejected — see Intent). The cron lives in a `reminder` record.
- **Not a general git host.** `git.*` v1 is `commit_push` + `status`; branch/PR/issue verbs already
  live in `lb-gh` (ported) but are not surfaced as MCP tools by this scope.

## Intent / approach

Three existing slices compose into this with **one new tool family** and **one ported crate**:

```
reminder (cron */10, action = McpTool git.commit_push)   ← lb-reminders (storage + cron math)
        │  react_to_reminders due-scan          EXISTS
        ▼
lb-jobs job (kind "reminder-fire", deterministic id)     ← host reminder reactor + lb-jobs   EXISTS
        │  fire_reminder → McpTool → call_tool chokepoint (re-checks mcp:{tool}:call)   EXISTS
        ▼
git.commit_push  →  lb-gh SyncRepo::commit_and_push(dir, branch, msg)   ← NEW: ported lazybones-gh + verb
        │
        ▼
git add -A → commit → push   (or Pushed::Clean, a no-op)
```

The periodic firing is a **`reminder` whose `Action::McpTool`** (the kind already in
`lb-reminders::Action`) calls `git.commit_push`. **Verified against the code (2026-06-30):** the
whole scheduling→job→tool path *already exists and is generic* — the reactor
(`host/src/reminder/react.rs`) enqueues **one `lb-jobs` job per firing** of the existing
`kind="reminder-fire"` with a **deterministic `fire_job_id(reminder_id, scheduled_ts)`** (so a
re-scan addresses the same job — one scheduled instant ⇒ one job ⇒ one effect, idempotent at the
reactor), and `fire_reminder` dispatches `Action::McpTool` by **re-entering the `call_tool`
chokepoint**, which re-checks `mcp:{tool}:call` under the principal's caps re-resolved from the grant
store at fire time. So this feature needs **no new job kind, no new reactor, and zero changes to
`lb-reminders` or `lb-jobs`** — only the new `git.*` verb behind `call_tool` and the crate it calls.
Idempotency is doubly safe: the reactor's stable job id prevents a duplicate firing, and
`commit_and_push` is itself a no-op on a clean tree (`Pushed::Clean`).

**Why a reminder + job, not a `systemd` timer firing a CLI** (the rejected alternative): a
`*.timer` + `lb local git commit-push` oneshot would *work* and is tempting for one dev box. But it
re-implements scheduling the platform already owns, and it bypasses the durable record, the audit
transcript, the workspace wall, and the capability check — the firing would be an ungated shell-out
instead of a `caps::check`'d tool call. It is also **not symmetric** (§3 rule 1): a cloud hub has no
systemd-per-workspace. Putting the cron in a `reminder` record means the *identical* mechanism runs on
a Pi edge and a cloud node; systemd's only job is to keep the `node` process up so the reactor scans.
On Windows (where systemd doesn't exist) the in-process reactor still fires — only the supervision
wrapper differs (a Windows service / Task Scheduler "keep running", not "schedule the commit").

`lb-gh` is a **true-external seam** (CLAUDE §9): it wraps `gh`/`git`, the one thing we can't run
in-process. That's the sanctioned single-trait-in-one-crate exception, and it's already structured
that way (`Gh`/`SyncRepo` over `tokio::process::Command`). We port it verbatim, rename to `lb-gh`,
add it to the workspace `members`, and let the `git.*` verbs call it.

## How it fits the core

- **Tenancy / isolation:** the `reminder` and the `job` are workspace-scoped records (the hard wall);
  the due-scan and job claim are ws-scoped. The git working tree is a node-local *path* (external,
  not in SurrealDB) — which repo a workspace auto-commits is a per-workspace pref/config naming the
  `dir` + `branch`, so workspace A can never drive workspace B's checkout.
- **Capabilities:** `git.commit_push` is gated by `mcp:git.commit_push:call`, `git.status` by
  `mcp:git.status:call`. Each has its own **deny test**. The reminder fires under a stored principal,
  re-checked at fire time (reminders already does this) — a withdrawn grant stops the auto-commit.
- **Placement:** **either** node, by config. The cron is in the record; the running node with the
  checkout executes. systemd unit is a Linux ops artifact; the feature itself is OS-agnostic.
- **MCP surface (API shape, §6.1):** mostly a single **action** verb (`git.commit_push`) + one
  **read** (`git.status`). No CRUD (nothing is created/listed in the store by these). **Not a batch.**
  The periodic work is correctly a **job** rather than a blocking call: `git push` does network I/O,
  can be slow or fail offline, and must survive a restart — exactly the playbook's "long/awaitable
  work is a job." A failed push is retried on the next tick (the remote is the durable sink).
- **Data (SurrealDB):** touches `reminder:{id}` and `job:{id}` only — no new tables. The git tree
  lives on disk (the external), never in the store (§3.2 holds).
- **Bus (Zenoh):** none required for v1. The reminders reactor and job lifecycle already emit their
  events; an optional "auto-commit pushed `<sha>`" channel post is a follow-up, not v1.
- **Sync / authority:** node-local execution; the *remote git repo* is the authority for the pushed
  content (last-writer-wins, like `lazybones-gh`'s `SyncRepo`).
- **Secrets:** **none handled here.** Push auth is the user's existing `gh auth login` /
  `gh auth setup-git` credential helper or SSH key — `lb-gh` carries no token (its whole design
  point). The systemd unit runs as a user with that credential available (a risk to flag, below).
- **Symmetric nodes:** no `if cloud`; the only OS-specific artifact is the supervision unit, isolated
  from the feature.
- **Durability:** the push is must-reach-the-remote, but git push is idempotent and the job retries,
  so it does **not** need the outbox — the remote ref *is* the durable record. Stated explicitly so a
  later reader doesn't "fix" this by routing it through `lb-outbox`.
- **SDK/WIT impact:** none — no change to the plugin ABI.

## Example flow

1. Operator creates a reminder (UI cron-builder or `lb`): `schedule: "*/10 * * * *"`,
   `action: { kind: "mcp-tool", tool: "git.commit_push", args: { dir: "/home/user/code/rust/lb",
   branch: "improverules-ui" } }`, `enabled: true`. Stored as `reminder:{id}` in the workspace.
2. The `node` runs under `systemd` (`lb-node.service`, `Restart=always`), so `react_to_reminders`
   scans on its tick.
3. At `T = :10`, the due-scan finds the reminder and enqueues the `kind="reminder-fire"` job at the
   deterministic id `reminder-fire:{id}:{scheduled_ts}` (existing path — no new kind).
4. `fire_reminder` re-resolves the principal's caps, dispatches `Action::McpTool` into `call_tool`,
   which re-checks `mcp:git.commit_push:call` and runs the verb → `lb-gh`
   `SyncRepo::open(dir, branch).commit_and_push(msg)`.
5. The tree is dirty (3 files changed) → `git add -A` → commit `chore(autocommit): 3 files @
   2026-06-30T12:10:00Z` → `git push origin improverises-ui` → returns `Pushed::Committed`. Job
   completes `Done`; the reminder's `next` advances to `:20`.
6. At `T = :20` nothing changed → `commit_and_push` returns `Pushed::Clean` → job `Done`, no commit,
   no push. (Idempotent: had the node restarted mid-job, re-running the single step is the same
   no-op.)

## Testing plan

Per `scope/testing/testing-scope.md`, **no mocks** — exercise the real store, the real reminder/job
verbs, and a **real temp git repo + a real bare `file://` remote** (the pattern `lazybones-gh`'s
`sync.rs` tests already use: no network, no GitHub login). Mandatory categories that apply:

- **Capability deny (mandatory):** a principal without `mcp:git.commit_push:call` is denied at
  `authorize_tool`; a withdrawn grant stops a reminder from firing the tool.
- **Workspace isolation (mandatory):** a reminder/job in ws-B never drives ws-A's checkout; the
  due-scan and job claim are ws-scoped; B cannot see A's `git-sync` jobs.
- **Tool behavior (real repo):** `commit_push` commits + pushes to the bare remote when dirty;
  returns `Clean` (no commit, no push) when the tree matches `HEAD`; `git.status` reports dirty/clean.
- **Cron math:** `next_after("*/10 * * * *", T)` on the injected logical clock (no wall-clock,
  testing §3).
- **Idempotent firing:** a re-scan of the same scheduled instant addresses the same
  `reminder-fire:{id}:{ts}` job (existing reactor behavior); and re-running `commit_push` on a clean
  tree is a no-op — no second commit.
- **E2E:** reminder due → reactor → job → tool → the bare remote actually receives the commit.

## Risks & hard problems

- **Push credentials on a headless service.** The pushing user needs a working credential
  (`gh auth setup-git` token or an SSH key the systemd unit's user can read). A bare auto-committer
  that silently fails to push for days is the real hazard — surface push failure loudly (job `Failed`
  with stderr, and ideally a degraded signal). Document the one-time `gh auth login` + setup-git.
- **Committing unintended files.** `git add -A` stages everything not ignored — `.env`, large
  artifacts, secrets. We rely entirely on `.gitignore`. Flag prominently; consider an allow/limit on
  paths as a follow-up, not v1.
- **Remote divergence.** `commit_and_push` does not pull first; if the remote moved ahead the push is
  a non-ff failure. For a single-operator backup that's acceptable (retries next tick once the user
  resolves), but see Open questions on an optional `fetch + pull --ff-only` pre-step.
- **Committing mid-edit.** Auto-committing every 10 min can capture half-saved work. Inherent to
  "commit no matter what"; the user has opted in. Pushing to a dedicated `wip/*` branch (Open
  questions) contains the blast radius.
- **Where the systemd unit lives** — packaging an OS service unit is an ops concern this scope only
  *names*; it may belong to a future `node-roles`/ops scope rather than `git-sync`.

## Open questions

- **Pre-push pull?** Add an optional `fetch + pull --ff-only` before commit (`commit_and_push` would
  gain a flag), or keep it pure push-and-fail-on-divergence (v1 default)?
- **Target branch.** Push the current working branch, or force a dedicated `wip/<host>` branch so
  auto-commits never touch the human's feature branch? (Recommend: a config field, default = current
  branch.)
- **Message template.** `chore(autocommit): <n> files @ <ts>` — include a short changed-paths summary,
  or keep it minimal? (`lb-gh` would need a `commit_and_push_with` taking a pre-built message.)
- **Repo path config.** Per-workspace pref naming `dir` + `branch`, or pass them as tool args on the
  reminder (as in the Example)? (Recommend: tool args on the reminder for v1; a pref later.)
- **Crate name** — `lb-gh` (matches `lazybones-gh`) vs `lb-git`. (Recommend `lb-gh`; it wraps `gh`.)
- **systemd unit home** — `docker/` (alongside the node image), a new `ops/`, or a `node-roles`
  deploy doc? Decide before shipping the unit so it isn't orphaned.

## Related

- `../jobs/jobs-scope.md` — the durable job this reuses (`lb-jobs`; the firing record + idempotent
  resume). README §6.9.
- `../reminders/reminders-scope.md` — the cron scheduled trigger + `Action::McpTool` + the
  `react_to_reminders` reactor that enqueues one job per firing.
- `../host-tools/host-tools-scope.md` — the folder-of-verbs MCP tool pattern `git.*` follows
  (one verb/file, one cap/verb, `authorize_tool` chokepoint). README §6.5.
- `../mcp/mcp-scope.md` — the tool surface + the `authorize_tool` gate.
- Source to port: `/home/user/code/rust/lazybones/crates/lazybones-gh` (`Gh`, `SyncRepo`,
  `commit_and_push`/`Pushed`).
- `../../../scripts/pq/` — the unrelated *prompt* `commit` role (AI-assisted message), kept separate.
- README `§3` (core rules), `§6.3` (Tier-2 / supervision), `§6.9` (jobs), `§6.10` (outbox — why this
  doesn't need it).
