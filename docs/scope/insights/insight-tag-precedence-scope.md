# Insights scope — tag echo: multi-source precedence

Status: scope (the ask). Filed by the triage session
([`insight-triage-session.md`](../../sessions/insights/insight-triage-session.md)) as the resolution
of slice 1's inherited open question — the **rule** is decided here; the **change** is not built.

## The problem

Tag edge identity in the shipped graph is **`(entity, tag, source)`**, so two edges for the same key
can legitimately coexist:

```
insight:01H…  classification=plumbing    source=Producer   (the nightly rule asserted it)
insight:01H…  classification=mechanical  source=Human      (an operator corrected it)
```

`Insight.tags` is a **flat `{k: v}` map**, so the echo must pick one. Today it doesn't pick — it
collects, and `materialize_facets` (`host/src/insight/raise.rs`) keeps whichever `tags.of` returned
last. That order is not specified, so the rendered dimension column for a corrected key is
**non-deterministic across raises**.

This was left open by [`insight-tag-echo-scope.md`](insight-tag-echo-scope.md) ("Open questions after
building" #3) because nothing in slices 1–2 let a human write a tag. Slice 3 changes the stakes: the
triage plane is where an operator first disagrees with the machine about a finding they now own.

## The resolved rule

**Source precedence: `Human` beats `Producer` for the same key.** Highest-precedence source wins; ties
within one source keep the newest edge by provenance timestamp.

The argument is the one the triage slice makes concrete: under last-write-wins, an operator
re-classifies a finding, the rule fires overnight, and their correction silently reverts. That is the
same class of trust bug as evicting a human comment
([`insight-triage-scope.md`](insight-triage-scope.md) resolved decision 4) — the platform quietly
discarding what a person deliberately recorded — and it is worse here because nothing surfaces it: the
column just reads differently tomorrow.

**Rejected: newest-wins regardless of source.** Simpler, and wrong in exactly the case that matters —
the machine re-asserts on every firing, so a producer always eventually wins. Rejected: a rule that
guarantees the human loses is not a tie-break.

**Rejected: making the echo multi-valued (`{k: v[]}`).** Honest about the graph, but it changes the
shipped `Insight.tags` shape for every consumer to serve a case that has one right answer anyway, and
"which of these two do I render in a column" just moves to every UI.

**Not in scope: which sources exist or who may write them.** This is a precedence rule over the
existing `Source` enum, not a change to the tag grammar or its caps.

## What building it touches

- `materialize_facets` (`rust/crates/host/src/insight/raise.rs`) — fold by `(key, source)` precedence
  instead of collecting into a `BTreeMap`. **This is the raise hot path**, and it changes what an
  existing echoed row means, so it is a shipped-semantics change with its own regression surface.
- The echo is refreshed on every raise, so corrected records self-heal on their next firing — but a
  record that never fires again keeps the old value, which is the **same** blind spot as slice 1's
  still-open backfill job. The two should probably land together.
- Blocked in practice by slice 1's other open item: **`tags.*` has no dispatcher entry**, so there is
  no wire door to write a `Human`-sourced tag edge yet. Until that exists, this rule is unreachable
  in production and untestable end-to-end over MCP (a test can still write edges directly).

## Testing plan

Per `scope/testing/testing-scope.md`, real store + real tag graph, no mocks:

1. **Precedence.** Seed both edges for one key through the real graph, raise, assert the echo carries
   the `Human` value — and that raising again does **not** flip it back.
2. **Determinism.** The same two edges in the reverse insertion order produce the identical echo (the
   bug today is that they don't).
3. **Tie within a source.** Two `Producer` edges for one key → newest by provenance ts.
4. **No regression for the common case.** A single-source key (the overwhelming majority) echoes
   exactly as it does today.
5. **Mandatory:** capability-deny + workspace-isolation on the raise path are unchanged, but re-run —
   this touches the verb they gate.

## Related

- Parent: [`insight-tag-echo-scope.md`](insight-tag-echo-scope.md) (the echo; "Open questions after
  building" #3 — this doc closes it as *decided*, not *built*)
- Sibling: [`insight-triage-scope.md`](insight-triage-scope.md) (why the stakes changed)
- `scope/tags/tags-scope.md` — the `(entity, tag, source)` edge identity this rests on
- Session that filed it:
  [`insight-triage-session.md`](../../sessions/insights/insight-triage-session.md)
