# Query variable resolves empty options for `series.find` / `series.list`

**Area:** frontend (dashboard variables) · **Status:** fixed · **Date:** 2026-07-09

## Symptom

A `query`/`source` dashboard variable whose resolver tool is `series.find` (or `series.list`,
`datasource.list`) always resolved to an **empty option list** — the variable-bar dropdown showed only
the `(none)` placeholder even though the tool returned real, workspace-scoped rows and the viewer held the
tool's cap. Surfaced building the advanced-template-variables slice: the workspace-isolation gateway test
asserting a `series.find` variable's options never populated for a permitted viewer.

## Root cause

`resolveOptions.ts`'s `extractRows` only understood two result shapes: a **bare array**, or an object with
a **`rows`** key (`store.query`). But the ingest read tools wrap their list under a **tool-specific key** —
`series.find`/`series.list` return `{ series: [...] }`, `series.read` returns `{ samples: [...] }`,
`datasource.list` returns `{ datasources: [...] }` (see `rust/crates/host/src/ingest/tool.rs`). None of
those matched `rows`, so `extractRows` fell through to `[]` and every such variable had no options. The
`custom`/`interval` (static) path masked it — only the query-over-bridge path was affected, and no test
had exercised a `series.find`-backed variable end to end before.

## Fix

Broaden `extractRows` to try the known wrapper keys (`rows`, `series`, `samples`, `datasources`, `items`,
`results`) and then fall back to the **first array-valued property** — so a new read tool's list resolves
without a per-tool branch (rule 10: the id/tool stays opaque data, no `if tool == "series.find"`).

## Regression test

`resolveOptions.test.ts` → "extracts the `series` array (series.find / series.list shape)". Also proven
live in `variablesAdvanced.gateway.test.tsx` (the isolation case now populates real ws-scoped options).

## Lesson

The variable resolver is "the same `{tool,args}` a cell source uses" — but a cell binds ONE known shape
per view, while a variable must reduce **any** read tool's result to an option list. The row extractor has
to be shape-tolerant across every read tool, not just `store.query`'s `rows`.
