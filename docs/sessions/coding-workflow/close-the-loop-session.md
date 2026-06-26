# Coding workflow — closing the loop: enriched PR payload + the resolution reactor (session)

- Date: 2026-06-27
- Scope: ../../scope/inbox-outbox/outbox-scope.md + ../../scope/coding-workflow/coding-workflow-scope.md
- Stage: S7 — platform maturity (STAGES.md). Two of the listed follow-ups, shipped together: the
  **producer payload enrichment** a live PR needs, and a **resolution reactor** that auto-starts the
  job on approval. The S7 exit gate was already MET; these connect the ingress and egress slices
  (`github-webhook` + `github-target`) into one live loop.
- Status: done

## Goal

Make the just-built **ingress** (webhook → triage → approval) and **egress** (outbox → GitHub)
connect end to end into a real PR, with no manual `start_job` in the middle. Two pieces:

1. **Producer payload enrichment.** `start_coding_job` emitted a `create_pr` effect whose payload
   was `{"scope_doc":"…"}`, but `lb-role-github-target`'s `request.rs` expects
   `{repo, head, base, title, body}`. Feed the adapter the shape it already maps — keep the
   producer's stable `pr_key` as the idempotency key, do **not** touch the adapter.
2. **A resolution reactor** that, when an approval inbox item resolves to `Approved`
   (`lb_inbox::Resolution`, the S6 facet), auto-starts the durable coding job — closing
   webhook → triage → approval → **JOB** → outbox → GitHub. A **durable-scan** reactor (the relay's
   altitude), not a LIVE-query one (noted as a follow-up).

## What changed

**The PR is now state, keyed by the approval (`crates/host/src/workflow/pr_spec.rs`, new):**

- A `PrSpec { repo, head, base, title, body }` — exactly the shape `github-target`'s `create_pr`
  payload deserializes, so the producer emits it verbatim and the adapter maps it with no shaping
  step. `PrSpec::create_pr_payload()` is the single source of truth for the wire JSON (built with
  `serde_json`, which also **fixes a latent escaping bug**: the old `format!`-built
  `{"scope_doc":"…"}` broke on a quote/brace in the value).
- `record_pr_spec`/`pr_spec` persist & read it, keyed by `approval_id` in the workspace namespace —
  the same sibling-record pattern as `lb_inbox::Resolution`. The PR coordinates must survive the
  approver disconnecting and be readable by the reactor with **no caller input at react time**, so
  they are recorded when approval is requested.

**The producer emits the enriched payload (`start_job.rs`):**

- `CodingJob` gains `pr: &PrSpec`. `start_coding_job` emits `pr.create_pr_payload()` as the
  `create_pr` effect body (was `{"scope_doc":…}`). `pr_key` stays the stable idempotency key.

**`request_approval` records the spec (`request_approval.rs`):**

- Takes `pr: &PrSpec` and `record_pr_spec`s it alongside the `needs:approval` item, in the same
  workspace, after the capability gate. Idempotent on `approval_id` (item + spec both upsert).

**The resolution reactor (`crates/host/src/workflow/react.rs`, new):**

- `react_to_approvals(node, principal, ws, channel, now)` — a **durable scan**, the same shape as
  `relay_outbox` (a function over a durable set, holding no state). It scans `lb_inbox::approved`
  (the new verb), and for each approved-but-not-started approval reads its `PrSpec` and calls
  `start_coding_job` with a deterministic `job_id = reactor_job_id(approval_id) = "job:{approval_id}"`
  and a stable `pr_key = "pr:{approval_id}"`.
- **Idempotency**: it **skips an approval whose job already exists** (`lb_jobs::load`), so re-running
  the pass — or a deferred→approved re-resolve — starts **one** job and queues **one** PR, never a
  second. `start_coding_job`'s `create` upsert + `emit_effect`'s effect-id dedup are a second line of
  defence; the existence check is what keeps the *pass* a no-op (and avoids re-streaming chatter).
- An approved item **without** a recorded `PrSpec` is skipped silently — not every approved inbox
  item is a coding-job request, so the reactor is safe to run over a workspace's whole resolution set.
- Runs under a host **service principal** (the workflow service is the actor); `start_coding_job`
  re-runs its own `mcp:workflow.start_job:call` gate for that principal — the capability wall holds.

**The new scan verb (`crates/inbox/src/approved.rs`, new):**

- `approved(store, ws)` lists every `Approved` `Resolution` in the workspace (filters on the
  kebab-case `"approved"` discriminant), oldest→newest — the inbox sibling of the outbox's
  `pending`/`due`. A durable scan, the source of truth (the LIVE-query push is the later optimization,
  same as the relay).

**The MCP bridge (`tool.rs`):**

- `request_approval` gains a `pr` object arg (`{repo, head, base, title, body?}`), read by `pr_arg`.
- `start_job` no longer takes PR fields on the wire — it **reads the recorded spec back by
  `approval_id`** (like the reactor), so the manual path and the reactor path are consistent; a
  missing spec is a `BadInput`.

## How it fits the core (the platform checklist)

- **Capability-first.** The reactor closes the loop only *through* `start_coding_job`'s
  `mcp:workflow.start_job:call` gate — an under-granted principal is refused and no job starts
  (mandatory deny test). Two surfaces, both enforced (the MCP grant ≠ the in-loop caps).
- **Workspace-first isolation.** Every read the reactor makes (the `approved` scan, the `pr_spec`
  read, the `job` existence check, the gate) selects the workspace namespace — a ws-B reactor pass
  physically cannot see or start a ws-A job (mandatory isolation test).
- **State vs motion.** The PR coordinates, the resolution, and the job are all **state** (records
  behind the wall); the reactor's act of starting is the motion. The PR itself still goes out through
  the **transactional outbox** (never raw pub/sub) — unchanged.
- **Stateless service.** The reactor holds no durable state; kill it mid-pass and the next pass
  re-reads `approved` and resumes — idempotent on the deterministic job id.
- **No `if cloud`, no SDK/WIT/cap-grammar change.** A `PrSpec` record + a host service function +
  one inbox scan verb. The github-target adapter is untouched (it already mapped the rich payload).

## Tests (all green — pasted below)

New:
- `crates/host/tests/workflow_reactor_test.rs` (5): enriched-payload auto-start + relay-out;
  idempotency (re-resolve/re-scan → ONE job, ONE PR); **capability-deny** (no `start_job` grant →
  refused, no job); **workspace-isolation** (ws-B reactor can't start ws-A's job); approved-without-
  spec is skipped.
- `role/github-target/tests/github_reactor_loop_test.rs` (1): **the full loop over a real socket** —
  approval → reactor auto-starts → enriched `create_pr` → outbox → the **real `GithubTarget`** opens
  the PR against a fake GitHub on `127.0.0.1:0`; the enriched body arrives intact; a second pass opens
  no second PR (end-to-end idempotency).
- `crates/host/src/workflow/pr_spec.rs` units (2): the payload is the github-target shape; special
  characters in the title are escaped (the `format!` bug fixed).

Updated (signature change): `workflow_test.rs`, `workflow_isolation_test.rs`,
`workflow_offline_test.rs` — all still green.

```
$ cargo test -p lb-host --test workflow_reactor_test
running 5 tests
test the_reactor_is_denied_without_the_start_job_grant ... ok
test an_approved_item_without_a_pr_spec_is_skipped ... ok
test a_ws_b_reactor_cannot_start_a_ws_a_job ... ok
test an_approval_auto_starts_the_job_with_the_enriched_pr_payload ... ok
test re_resolving_the_same_approval_starts_exactly_one_job ... ok
test result: ok. 5 passed; 0 failed

$ cargo test -p lb-role-github-target --test github_reactor_loop_test
running 1 test
test an_approval_opens_a_real_pr_through_the_reactor_and_the_github_target ... ok
test result: ok. 1 passed; 0 failed

$ cargo test -p lb-role-github-target --test github_target_test      # regression
running 4 tests ... test result: ok. 4 passed; 0 failed

$ cargo test -p lb-host --test workflow_test --test workflow_isolation_test --test workflow_offline_test
workflow_isolation_test: ok. 2 passed
workflow_offline_test:   ok. 3 passed
workflow_test:           ok. 3 passed

$ cargo test -p lb-host --lib   # pr_spec units
test workflow::pr_spec::tests::special_characters_in_the_title_are_escaped ... ok
test workflow::pr_spec::tests::create_pr_payload_is_the_github_target_shape ... ok
test result: ok. 2 passed; 0 failed

$ cargo test -p lb-inbox
test result: ok. 4 passed; 0 failed

$ cargo test -p lb-role-github-webhook    # ingress regression (other end of the loop)
test result: ok. 3 passed; 0 failed

$ cargo build --workspace      # green
$ cargo fmt --all --check      # clean
$ bash scripts/check-file-size.sh
FILE-LAYOUT: all source files within 400 lines (325 checked)
```

Net: **~206 Rust + 26 Vitest + 2 shell** tests green (+8 Rust this slice).

## Decisions & alternatives rejected

- **Where the PR coordinates live.** Rejected stuffing JSON into the approval `Item.body` (a freeform
  string — fragile to parse, and the body is human-facing). Rejected a closure/spec-resolver passed
  into the reactor (the reactor is a durable scan, not an API call — it must read state, not receive
  it). **Chose** a small `PrSpec` sibling record keyed by `approval_id`, mirroring `Resolution` — the
  spec is *state*, it survives a disconnect, and the reactor reads it with no caller input.
- **Durable-scan reactor, not LIVE-query.** S6/S7 drive the workflow with durable scans + explicit
  starts (the relay is the same shape). The scan is the source of truth, so a restarted reactor never
  misses an approval. The LIVE-query push (instant pickup) is the latency optimization layered on
  later — recorded as a follow-up, same as the relay's.
- **Idempotency via job-existence, not just upsert.** The `create` upsert + effect-id dedup already
  prevent a *duplicate*, but a re-scan would re-stream "job started" chatter and re-emit on every
  pass. The `lb_jobs::load` existence check makes the whole *pass* a no-op — the right semantics for
  a reactor that runs repeatedly.
- **`start_job` MCP verb reads the recorded spec** rather than re-accepting PR fields — one source of
  truth for the PR shape (the spec recorded at approval), and the manual + reactor paths converge.

## Cross-links

- Scope: ../../scope/inbox-outbox/outbox-scope.md (open questions refreshed — producer enrichment +
  resolution reactor resolved; LIVE-query reactor, search-before-create still open).
- Public: ../../public/inbox-outbox/inbox-outbox.md (the loop is now end-to-end).
- Prior slices this connects: ../extensions/github-webhook-session.md (ingress),
  ./outbox-egress-session.md (egress).
- No debugging entry — nothing broke (the signature change was mechanical, caught at compile time).
