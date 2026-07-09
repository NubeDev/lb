# Session — Buildings demo rule examples: scrubbed + structurally pinned to a real test

Follows `buildings-demo-examples-HANDOVER.md`. That handover left step 2 (the three click-to-load
buildings rule examples) **NOT DONE**: the `---` comment trap was still in the shipped file, and the
examples had only ever been hand-verified with throwaway tests — no committed regression test, so they
could silently drift from a query that actually runs. This session closes both.

## What shipped

1. **Scrubbed the `--` trap.** The three example bodies no longer contain any `--`/`---` inside a
   `body` string. The header lines that read `// --- Raise the finding ---` are now plain-words
   (`// Raise the finding (LIVE):`). This is the exact thing that broke the user 2–3 times: when a
   `// ---` line is un-commented (the `//` stripped), Rhai reads the bare `---` as a reserved operator
   (`'--' is a reserved symbol`). The only remaining `--` in `examples.ts` are TS-source section
   dividers *outside* any `body:` string — never uncommented into a rule.

2. **One source of truth, host-owned.** The three bodies moved out of the hardcoded TS arrays into
   `rust/crates/host/src/rules/buildings_examples.json` (id / title / summary / `body` as a line
   array). Both consumers read that one file:
   - the UI: `ui/src/features/rules/examples/examples.ts` imports the JSON and spreads
     `...BUILDINGS_EXAMPLES` in place (same cross-tree JSON-import pattern as
     `widgetCatalog.consistency.test.ts` importing the host's `widget_catalog.json`);
   - the test (below) `include_str!`s the same JSON and runs each body.

   So the strings the editor ships **are** the strings the test proved green — they cannot diverge.

3. **A committed regression test on the REAL path** —
   `rust/crates/host/tests/rules_buildings_examples_test.rs`. Rule 9 (no mocks): real embedded
   SurrealDB, real caps, the REAL supervisor spawning the REAL `federation` sidecar, and the REAL
   committed `.lazybones/data/demo/buildings.db` (testing §0's one sanctioned external). Modeled on
   `federation_sqlite_test.rs` (install + `datasource.add`) merged with a real `rules_run`. It:
   - installs the federation sidecar, registers `buildings.db` as the `demo-buildings` sqlite source
     (absolute canonicalized path DSN);
   - runs each of the three example bodies through `rules_run`;
   - **query / strict** → 8 rows, Riverside Data Center on top at 4.68 kWh/m², **0 findings** (their
     emit blocks are commented out);
   - **alert** → 8 rows + exactly **1 alert finding** (only Riverside > 1.0 kWh/m²), and asserts the
     alert fanned out to one real inbox item on the `rules` channel;
   - **capability-deny**: the same body minus `mcp:federation.query:call` is denied mid-run;
   - **workspace-isolation**: ws-B holds the cap but never registered the source → the run fails, it
     never reads ws-A's data.

## Proof

- `cargo test -p lb-host --test rules_buildings_examples_test` → **1 passed**.
- Drift is genuinely caught: temporarily changing `s.name AS building` → `s.nonexistent AS building`
  in the shared JSON made the test **FAIL** with the DataFusion schema error (then restored). A broken
  example query fails CI — which is the structural fix for the whole drift disaster.
- `cd ui && npx tsc --noEmit -p tsconfig.json` → clean (the JSON import typechecks).
- `cd ui && npx vitest run src/features/rules` → 31 passed.

## Hard-won facts confirmed still true (from the handover, re-verified by the green test)

`.records()` returns positional arrays (`r[0]` = building, `r[1]` = intensity); the last-expression
array lands at `output` = `RuleOutput::Scalar(array)`; `alert` needs
`mcp:inbox.record:call` + `mcp:outbox.enqueue:call` (in the admin cap set) while `emit` does not; a
rule's `query()` routes through `federation.query` and re-checks `mcp:federation.query:call` mid-run.
DataFusion (not SQLite) is the planner — the GROUP-BY-every-column / `CAST(... AS DOUBLE)` / JOIN-not-
subquery rules in the handover all hold, and the committed test now guards them.

## Still NOT done (the actual lesson — unchanged from the handover)

Only step 2 (rules) is now trustworthy-and-tested. The beginner lesson itself — datasource+query →
rules → **flows → insights → dashboard** — is still not written. The handover's "rest of the lesson"
section (verified mechanics for each remaining step) stands as the starting point. Deliverable format
(repo MDX under `doc-site/content/public/` vs. a visual page) is still undecided with the user.
