# Messaging slice — bus pub/sub + inbox + UI + hot-reload (session)

- Date: 2026-06-26
- Scope: ../../scope/bus/bus-scope.md (+ inbox-outbox, tenancy, frontend)
- Stage: S2 — First app: messaging + UI + hot-reload (STAGES.md)
- Status: done

## Goal

Build the S2 messaging slice as a **vertical** slice through every layer (not "finish the bus
crate"): bus pub/sub + presence on the existing Zenoh peer; channels persisted to SurrealDB
(state vs motion); a generic inbox; a React/Tauri UI against the local node; and a proven
hot-reload. Every access goes through `caps::check` (capability-first), workspace-first.

**Exit gate (S2), restated as the acceptance criterion:** post a message in the UI and see it
appear in real time; restart the node and the history is intact; swap an extension version with
no dropped state.

## What changed

### PART 1 — the headless messaging contract (Rust)

- **bus** (one verb per file): `publish.rs` (put onto `ws/{id}/chan/{cid}/msg/{id}`),
  `subscribe.rs` (`Subscription` over a `ws/{id}/chan/{cid}/msg/**` key expr), `presence.rs`
  (Zenoh **liveliness** tokens under `ws/{id}/presence/{member}`, with a `history(true)` watch).
  All keys go through the existing `ws_key` prefix — the workspace wall is structural on the bus.
- **store**: added a `list.rs` verb — a namespace-scoped *filter* (`WHERE data.<field> = $value`),
  with a `[a-z0-9_]` guard on the interpolated field (injection class shut). It does **not**
  order (see Debugging); ordering is the caller's.
- **inbox** (was a stub): the normalized `Item` model + `record.rs` (persist via store, idempotent
  on `(channel,id)`) + `list.rs` (read a channel, sorted by `ts` in Rust). Inbox items are STATE.
- **host `channel/` service** — the capability chokepoint that ties it together, verb per file:
  `authorize.rs` (the shared `caps::check` gate, `bus:chan/{cid}:{pub|sub}`), `post.rs`
  (authorize → **persist (state) → publish (motion)**, in that order), `history.rs` (durable
  read), `subscribe.rs` (live `Item` stream), `presence.rs` (authorized join/watch), `key.rs`
  (the one place channel id → cap resource + bus keys, so they can't drift).

### PART 2 — the React + Tauri UI against the local node

- **ui/**: Vite + React + TS + Tailwind (quiet control-surface tokens) + lucide. One
  component/hook per file (FILE-LAYOUT §4): `features/channel/{ChannelView,MessageList,
  MessageComposer}.tsx` + `useChannel.ts`; `lib/channel/{channel.api,channel.types}.ts`
  (the api client mirrors the Rust verbs `post`/`history` one-to-one); `lib/ipc/{invoke,fake}.ts`
  (the single IPC seam — Tauri `invoke` in the shell, an in-memory faithful node fake in the
  browser/tests until SSE lands at S3).
- **ui/src-tauri/**: a Tauri v2 shell. The node runs **in-process** (the shell IS a node, §3.1).
  The IPC commands `channel_post` / `channel_history` (one verb per file) are thin glue over
  `lb_host::post`/`history` with the session principal — the *same* capability check guards the
  desktop UI as every other caller. Command logic is a library so it is unit-tested **headlessly**
  (no webkit toolchain); the window wiring is behind a `desktop` feature.

### PART 3 — prove hot-reload

- **extensions/hello-v2**: the same `echo` tool, output carries `"v": 2` (the swap target).
- **host `reload.rs`**: `reload_extension` re-instantiates a component and **replaces** the
  registry entry in place — the store (durable state) is never touched. A reload of a
  not-yet-hosted id is rejected (swap, not silent install).

## Decisions & alternatives

- **Persist before publish** in `post` (not publish-then-persist): the durable record is the
  source of truth; a subscriber that missed the live push recovers from `history`. The inverse
  could echo a message that never durably landed. (§3.3 state-vs-motion made concrete.)
- **Presence on liveliness, not a stored "online" flag** — Zenoh auto-retracts a token when a
  peer drops (even on crash), so presence can't go stale; a stored flag would. (bus scope.)
- **The generic store `list` does not order; the inbox sorts by `ts`** — the generic store has
  no business knowing where a record's order key lives, and SurrealDB rejected the coupled query
  anyway (Debugging). Cleaner layering as a side effect of the bug.
- **One IPC seam (`invoke.ts`) with a faithful fake** rather than branching on "am I in Tauri"
  throughout the UI — the same `ChannelView` powers the desktop shell, the browser, and the tests
  unchanged. The fake mirrors the node contract (ordered, idempotent, ws-scoped) so behavior
  matches the real node; it is dropped at S3 when the browser talks to a node over SSE.
- **Shell command logic in a library + `desktop` feature gate** — so the capability-checked IPC
  path is unit-tested on a machine with **no** webkit toolchain (this one), while the windowed
  `tauri build` remains a packaging step. No `if cloud`; the window is config, not core code.
- **Reload = replace the registry instance, store untouched** — the stateless-extension
  guarantee (§3.4) is *why* this is safe: nothing durable lives in an instance, so swapping it
  cannot drop state. The test proves both halves (state intact AND the new version answers).

## Tests

Mandatory categories that apply at S2 and now exist: **capability-deny**, **workspace-isolation**
(across bus + store + inbox), and **hot-reload**. Offline/sync: n/a (single node; arrives S3).
Determinism held: tokens use a fixed clock; item `ts` and the UI clock are injected; **each bus
test uses a unique workspace id** (in-process Zenoh peers share a workspace's keyspace — see
Debugging).

Rust (host integration, real wasm + real embedded SurrealDB + in-proc Zenoh):
- `messaging_test` (3): live subscriber sees a post; history survives independent of the bus
  (the restart guarantee at the store layer); idempotent re-post.
- `messaging_deny_test` (3, **mandatory deny**): post without `pub`; read/listen without `sub`;
  a grant on another channel doesn't authorize this one.
- `messaging_isolation_test` (2, **mandatory isolation**): a sub in ws B never receives a publish
  in ws A (BUS); ws B's history never returns ws A's items (STORE + INBOX).
- `presence_test` (2): join is seen / drop is seen (liveliness); presence needs a `sub` grant.
- `hot_reload_test` (2, **mandatory hot-reload**): swap hello v1→v2 live — channel history
  intact AND v2 answers (`"v":2`) AND the channel keeps working; reload of an uninstalled id
  refused.

Store / inbox units (real SurrealDB):
- `store/list_test` (3): filters by field/value; ws-isolation; rejects a non-identifier field.
- `inbox/inbox_test` (4): record+list ordered; idempotent; channels independent; ws-isolation.

UI:
- Vitest `ChannelView.test.tsx` (3): **post a message, see it appear** (the exit gate in the UI),
  ordered oldest→newest, ignores empty — through the real hook + api client + IPC seam.
- Vitest `channel.api.test.ts` (3): the client over the node fake — ordered, idempotent,
  ws-scoped (the node's guarantees, asserted against the stand-in).
- Shell `commands_test` (2): `channel_post` → `channel_history` round-trips through the real
  capability-checked node — the Rust mirror of the ChannelView test.

### Green output

```
# Rust workspace (was 35 at S1 → 54 now; +19 this slice)
$ cargo test --workspace            # 54 passed, 0 failed
  host/messaging_test ........ 3 passed
  host/messaging_deny_test ... 3 passed   # MANDATORY capability-deny
  host/messaging_isolation ... 2 passed   # MANDATORY workspace-isolation (bus+store+inbox)
  host/presence_test ......... 2 passed
  host/hot_reload_test ....... 2 passed   # MANDATORY hot-reload (live v1→v2 swap)
  host/spine_test ............ 4 passed   # S1 exit gate, still green
  store/list_test ............ 3 passed
  store/isolation_test ....... 2 passed
  inbox/inbox_test ........... 4 passed
  caps/* + auth + bus + sdk + ext_loader ... 29 passed
  TOTAL PASSED: 54

$ cargo fmt --all --check          → FMT OK
$ bash rust/scripts/check-file-size.sh
  FILE-LAYOUT: all source files within 400 lines

# Shell command layer (headless — no webkit window)
$ cd ui/src-tauri && cargo test    # 2 passed
  commands_test::post_then_history_round_trips_through_the_command_layer ... ok
  commands_test::history_is_empty_for_an_untouched_channel ................. ok

# UI (Vitest) + type-check + bundle
$ cd ui && pnpm test               # 6 passed (2 files)
  ChannelView.test.tsx ....... 3 passed   # "post a message, see it appear"
  channel.api.test.ts ........ 3 passed
$ pnpm build                       → tsc --noEmit clean; vite build ✓

# Node binary still live end to end
$ cargo run -p node
  loaded hello: tools=["echo"] granted_caps=[]
  hello.echo -> {"echo":"hi"}
```

## Debugging

Two non-trivial breakages this session, each with a debug entry + regression test:

- [store/order-by-needs-selected-idiom](../../debugging/store/order-by-needs-selected-idiom.md)
  — SurrealDB rejects `ORDER BY data.ts` when only `data` is projected. Fixed by making the
  generic store `list` a pure filter and sorting by `ts` in the inbox. Regression:
  `messaging_test::history_survives_independent_of_the_bus`.
- [bus/in-process-peers-share-the-keyspace](../../debugging/bus/in-process-peers-share-the-keyspace.md)
  — in-process Zenoh peers auto-discover and share a workspace's keyspace, so concurrent tests
  reusing `ws="acme"` cross-talked. Not a product bug (it's the multi-node design); fixed by a
  unique workspace id per test. Standing rule recorded in the bus scope.

## Public / scope updates

- Promoted to `public/`: `bus`, `inbox-outbox` (new), `frontend` (new), and `public/SCOPE.md`.
- Fleshed out the stub scopes: `inbox-outbox` (Item shape, idempotency, state-vs-motion) and
  `tenancy` (the workspace wall across the three messaging surfaces). Refreshed open questions in
  `bus` (pub/sub + presence resolved; message classes / outbox deferred) and `frontend` (IPC seam
  + the SSE follow-up).

## Dead ends / surprises

- First instinct was to order in the store query (`ORDER BY data.ts`); SurrealDB's
  idiom-in-projection rule turned that into a better layering (store filters, inbox orders).
- Tauri's native window needs the webkit2gtk toolchain, absent here; splitting the command
  layer into a headlessly-testable library + a `desktop`-feature window kept the capability path
  fully tested without it. The windowed `tauri build` is a packaging step for a desktop machine.

## Follow-ups

- S3: replace the in-memory UI fake with a real node over SSE/HTTP; push others' messages into
  `useChannel`'s `setItems` (the seam is already there) and wire presence into the UI.
- Message classification (fire-and-forget / must-deliver / must-replay) + the transactional
  outbox as the must-deliver path (bus + inbox-outbox open questions) — when a second node lands.
- A `#[lb_test]` macro baking in the multi-thread flavor + a `unique_ws()` fixture, if the bus
  test boilerplate grows.
- STATUS.md updated? **Yes** — Messaging slice marked `shipped`; S2 exit gate met.
