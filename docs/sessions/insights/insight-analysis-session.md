# Session — insights: `analysis` (the finding explains itself)

Status: **shipped** (slice 2 of GitHub issue #119 — `analysis` only).
Scope: [`../../scope/insights/insight-analysis-scope.md`](../../scope/insights/insight-analysis-scope.md).
Branch: `master` (no commits made — the human owns git).
Date: 2026-07-30.

## The ask, restated

`evidence` says **where the data is**; nothing on the record says what the producer **concluded**.
That reasoning had two wrong homes — free-form `body` (opaque, so no consumer renders it
consistently and the analyst persona sniffs JSON shapes) or `title` (one line, truncated into
uselessness). Add an optional closed `analysis` struct beside `evidence`: four prose fields plus two
`Quantity` fields, `get`-only, no new capability.

Sequencing: slice 1 (the tag echo) shipped first because the roster's dimension columns come from it,
not from here. Slice 3 (triage) is untouched by this session.

## What shipped

| Layer | Change |
|---|---|
| Type | `Analysis` (six closed fields) + `Quantity` (`value`/`unit`/`note`) + `MAX_ANALYSIS_BYTES` + `validate_analysis` — `rust/crates/insights/src/analysis.rs` (new file, one responsibility: the shapes + their guard) |
| Record | `Insight.analysis: Option<Analysis>` — `#[serde(default)]` + `skip_serializing_if`, so a pre-field row still decodes (`insight.rs`) |
| Raise | `RaiseInput.analysis`, validated **up front** beside the evidence/occurrence guards; refresh-on-supply on the dedup arm, carried on the create arm (`raise.rs`) |
| Read | `list.rs` strips `analysis` beside `evidence` (`get`-only boundary); `get` returns the typed record and the gateway routes pass it through |
| Frontend | `Analysis` + `Quantity` interfaces and `Insight.analysis?` on the canonical TS shape, exported from the package (`packages/insights/src/types.ts`, `index.ts`) — mirroring what slice 1 did for `tags`. Doc comments carry the three rules a renderer must not lose: the label-level hedge on `suspected_cause`, group-by-`unit` before summing, and `analysis`-refreshes-while-`title`-doesn't. |
| Doors | **none needed** — every producer door deserializes `RaiseInput` at one place (`host/src/insight/tool.rs:44`), so the rhai handle, the flow sink, and the MCP door all carry the field for free. The scope's "SDK/WIT impact: none" claim held literally. |

No new verb, no new capability, no new table — as the scope requires.

## Decisions made while building

**1. `validate_analysis`, not `validate_analysis_size`.** The evidence precedent is named for a size
check, but this type has two *shape* rules the scope demanded as rejections (test §1a) — a `value`
with no `unit`, and an all-fields-absent `Quantity`. Folding them into one guard called once at the
raise boundary keeps the "reject before any write, no orphan row" contract in a single place; naming
it `_size` would have lied about what it enforces.

**2. The shape rules reject rather than normalise.** A `value` with no `unit` could have defaulted to
a blank unit, and `{}` could have been stored as-is. Both were refused: the whole reason `Quantity`
is typed is cross-finding aggregation, and a unit-less number is the seed of exactly the unit
mismatch the scope's §Risks names. `{}` says strictly *less* than omitting the field, so storing it
would invent a distinction the producer never made.

**3. The error message names `body`.** Scope test §4 asks for it and the reason is worth restating:
for a closed struct, the rejection is the producer author's only teacher. The oversize message says
"anything outside the six named fields, in `body`" — the drop rule and the overflow in one sentence,
at the moment someone hits it.

**4. `Analysis` derives `Default`.** Not required by the scope, but it makes the "producer that knows
one field" case constructible without six `None`s in the rhai/flow bridges downstream.

## Testing

`rust/crates/host/tests/insight_analysis_test.rs` — 16 cases, real booted `Node`, real store, real
caps, the real `call_tool` MCP bridge. No mocks (CLAUDE §9): every record is seeded by raising
through the verb under test and read back through it.

```
cargo test -p lb-insights                        → 3 + 10 ok
cargo test -p lb-host --test insight_analysis_test → 16 passed; 0 failed
                      --test insight_evidence_test → 10 passed   (the neighbour it reuses)
                      --test insight_tag_echo_test → 11 passed   (slice 1, unbroken)
                      --test insights_test         → 22 passed
cargo fmt --check → clean;  cargo build --workspace → green
cargo clippy -p lb-insights --tests → no new warnings from `analysis.rs`
packages/insights: pnpm typecheck → clean;  pnpm test → 12 passed (3 files)
```

Mandatory categories, both present: **capability-deny** (raise denied without the cap; a `LIST`-only
reader never receives prose; and the deny asserted on the property only the outer gate has — a real
id and a fictional id produce **identical** errors) and **workspace-isolation** (ws-B reads no ws-A
reasoning through `get` or `list`).

Scope cases covered: round-trip (§1), the three `Quantity` shapes incl. **`value` decodes as a
number, not a stringified one** (§1a), the pre-field migration guard (§2), dedup-refresh + the omit
arm + evidence/analysis independence (§3), the 4 KB reject naming `body` with **no orphan row** (§4),
the `get`/`list` boundary under a filter *and across a keyset page boundary* (§5), and the deliberate
closed-struct **drop** of an unknown key (§6).

**Revert-check performed** (the one the scope names): making the dedup arm's assignment
unconditional turned exactly the two omit-arm tests red
(`re_raise_refreshes_analysis_and_omitting_it_leaves_the_stored_value`,
`analysis_and_evidence_refresh_independently`) and nothing else. Restored.

**Three pre-existing failures, all verified unrelated** by re-running them on a clean tree
(`git stash`) and getting the identical result:

- `lb-cli`'s `sign_test` and `lb-role-gateway`'s `publish_install_test` don't **compile** here — both
  read a wasm artifact under `extensions/`, a tree that does not exist in this working copy, so they
  are structurally unrunnable rather than failing.
- `lb-host`'s `rules_test::registered_datasource_is_in_the_rule_allowlist` fails (21/22 pass). Not
  touched by this slice and red before it; noted rather than fixed, since it belongs to whoever owns
  the rule-allowlist path.

A full `cargo test --workspace --exclude lb-cli --exclude lb-role-gateway` otherwise reported **zero**
`FAILED` suites. (One earlier full run showed a one-off `federation::direct_path_pg_test` harness
error that did not reproduce — that suite is gated on a real Postgres and runs 0 tests here.)

## Live verification (not just the suite)

Against a real booted node from **this** working tree — `cargo run -p lb-role-gateway --features
test-harness --bin test_gateway`, `LB_DEV_LOGIN=1 PORT=8137` — over the real `POST /mcp/call` wire
with a real minted member token.

```
raise {analysis:{…six fields…, deviation:{note:"N/A"}, estimated_impact:{note:"N/A (data quality)"}}}
  → {"count":1,"created":true}
get   → all six labels echoed verbatim, both note-only quantities intact
list  → {"tags":{"asset_type":"water-meter","building":"chullora-dc"}, analysis: ABSENT}
        ^ the roster row carries the slice-1 dimension echo and NOT the drawer's prose

re-raise with deviation:{value:-100.0,unit:"%"} estimated_impact:{value:180.0,unit:"AUD/day"}
  → count 2;  get → impact value 180.0, type "number"   ← the sortable corpus, on the wire
re-raise OMITTING analysis
  → count 3;  get → impact value still 180.0            ← omission means unchanged

raise {analysis:{deviation:{value:-100.0}}}          → refused: "…value requires a unit…"
raise {analysis:{trigger_logic: <5000 chars>}}        → refused: "…5020 bytes exceeds the 4096-byte
                                                         cap … in `body`"
raise {analysis:{trigger_logic:"ok", confidence:0.9}} → accepted; get → {"trigger_logic":"ok"}
list  → ["drop-1","rule:no-water-1d:WM-CHU-01"]       ← neither refused raise left an orphan row
```

Every behaviour the scope specifies, confirmed end to end on the real wire — including the two that
`cargo test` alone has historically been the weakest evidence for (the number staying a number
through JSON, and the refusals leaving no partial write).

**Honest caveat on the "drawer renders six labels" half of scope §7.** The drawer is a *downstream*
surface: lb is a library and the UI shell is out-of-tree (`MIGRATION.md`), so no drawer in this repo
can be checked. The verification above proves the field arrives at the consumer boundary with the
right shape; rendering the six labelled rows — and carrying the hedge in the **label**
("Suspected cause", scope §Risks) — is a `rubix-ai` change this session cannot make. Worth naming
because the label is the one place that hedge can quietly evaporate.

Filed downstream as **[NubeIO/rubix-ai#69](https://github.com/NubeIO/rubix-ai/issues/69)**, with the
two facts that make it actionable rather than a vague hand-off, both verified in that tree: its
vendored `ui/packages/insights/src/types.ts` carries `tags?` but **no `analysis`** (so the field is
silently dropped before any component sees it), and `ui/src/features/insights/InsightDetail.tsx:177`
renders `insight.body` under an **"Evidence"** heading via the shape-driven `EvidenceBody` — i.e. the
producer's reasoning currently shows as a JSON dump, which is the exact problem this slice exists to
fix. The issue carries the three rules the backend cannot enforce: the label-level hedge, attributing
`estimated_impact` rather than asserting it, and preferring `note` when rendering a `Quantity`.

## Filed, not fixed

Scope resolved-decision 6 says to **file** the `title`/`body` refresh change rather than fix it
inline, because it alters shipped semantics. Filed as
[`insight-prose-refresh-scope.md`](../../scope/insights/insight-prose-refresh-scope.md)
(issue [#124](https://github.com/NubeDev/lb/issues/124)) — and this
session is what makes the old behaviour indefensible: the drawer now shows *fresh* reasoning above a
firing-#1 narrative. That also closes `insight-evidence-scope.md` Q1 as "decided elsewhere".

## Next step

Slice 3 of issue #119: [`insight-triage-scope.md`](../../scope/insights/insight-triage-scope.md) —
`assigned_to` + the append-only comment thread, two new member-grade caps, and the load-bearing rule
that a re-raise leaves **both** untouched. Note it inherits an **open** question from slice 1: how a
flat tag echo resolves same-key multi-source edges, which needs a rule before triage lets humans
re-classify.

Also still open from slice 1: the tag-echo **backfill job** for records that never fire again.

## Related

- Scope: [`insight-analysis-scope.md`](../../scope/insights/insight-analysis-scope.md) (status
  updated to shipped; "Open questions after building" added)
- Slice 1: [`insight-tag-echo-session.md`](insight-tag-echo-session.md)
- Skill doc: `docs/skills/insights/SKILL.md` — the raise/get walkthrough now carries `analysis` and
  states the closed-struct rule
- Public: `doc-site/content/public/insights/insights.md`
