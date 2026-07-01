# prefs — i18n MessageFormat catalogs + per-recipient fan-out (Phase 2) (session)

- Date: 2026-07-01
- Scope: ../../scope/prefs/i18n-catalogs-scope.md (parent: ../../scope/prefs/user-prefs-scope.md)
- Stage: rides the shipped `lb-prefs` core crate (a `catalog::*` module + `message.*` verbs). No new
  crate, no SDK/WIT change.
- Status: done (MF1 catalogs on the shipped renderer; icu4x still deferred as a named swap-in)

## Goal

The shipped S8 core resolves a principal's language and formats dates/numbers/quantities; what it
could not yet do is **render a translatable message**. This slice adds the ICU MessageFormat 1 (MF1)
catalog layer: built-in en/es catalogs compiled into the host (and shipped to the client bundle,
generated), an optional per-workspace sparse **override catalog**, a pure `catalog::render` that
selects the message + runs plural/select + interpolates `format::*` placeholders, and the three MCP
verbs (`message.render` / `prefs.catalog` / `message.set_catalog`) so server-generated content is
localized **server-side, per recipient** (fanning out to each member's resolved prefs).

Exit gate: the mandatory deny + workspace-isolation tests green against a real store + MCP + gateway,
plus MF1 plural/select en+es, the fallback chain, placeholder-parity (byte-identical to direct
`format::*`), the multi-recipient fan-out headline, catalog-lint rejection, the generation drift
guard, and an `intl-messageformat` cross-check confirming host==client — all real infra, no mocks.

## What changed

New `catalog/` module in `rust/crates/prefs/` (one responsibility per file):
- `catalog/message.rs` — the **hand-written MF1 subset parser + renderer** (~430 lines incl. docs).
  Parses the pinned closed grammar (argument, `plural` with `one`/`other`/`=0`/`=1`, `select` with
  arbitrary keywords + mandatory `other`, typed `date`/`number`/`quantity,<dim>`, one level of
  nesting, the `#` count token, `'{'`/`'}'` escapes). Anything outside is a `ParseError` (the lint
  contract). `render()` walks the AST; selection uses `plural`, placeholders dispatch to `interpolate`.
- `catalog/plural.rs` — hand-encoded en/es CLDR-44 cardinal categories (`n==1 → one, else other`),
  the flagged icu4x swap point.
- `catalog/interpolate.rs` — routes each typed placeholder to the **already-shipped `format::*`**
  (`{ts,date}`→`format_datetime`, `{n,number}`→`format_number`, `{v,quantity,dim}`→`format_quantity`
  from the dimension's canonical unit). Placeholder failure → the literal `[<arg>]` (never panic).
- `catalog/builtin.rs` + `catalog/builtin/{en,es}.mf` — the compiled-in (`include_str!`) MF1 text
  assets with a `catalog-version:` header; flat dotted keys; the language→en→key fallback chain.
- `catalog/mod.rs` — `render(key, args, override, resolved) -> Rendered`, `merged_catalog(locale,
  override)`, and `lint(messages)`. Never panics / never blanks.
- `axis/dimension.rs` — added `canonical_unit()` (the SI/base unit a `{v, quantity, dim}` placeholder
  carries the canonical value in, so it converts from the right `from_unit`).

Store layer (`store/`): `catalog_schema.rs` (SCHEMAFULL `message_catalog:[ws, locale]`, flexible
`messages` object), `catalog_get.rs` (read the sparse override map + `has_override`), `catalog_set.rs`
(read-merge-write so the merge is **per message-key** — two offline edits to different keys both
survive; composite id → idempotent replay, same-key LWW).

Generation: `bin/gen_catalog.rs` (`gen-prefs-catalog`) emits `ui/src/lib/prefs/catalog.generated.ts`
(twin of `gen_ts`); `tests/generated_catalog_test.rs` re-runs it and asserts byte-identity on disk.

Host wiring (`rust/crates/host/src/prefs/`): `catalog_verbs.rs` (`message_render` — resolves the
target's prefs in the caller's ws, loads that language's override, renders; `prefs_catalog` — the
merged map; `message_set_catalog` — lint → merge → publish the hint), `catalog_authorize.rs` (base
`message.render` member gate + the `message.render_recipient` fan-out gate for recipient!=self;
`prefs.catalog` member; `message.set_catalog` admin), `catalog_motion.rs` (the `ws/{ws}/prefs/
catalog-changed` hint, fire-and-forget), `catalog_tool.rs` (the MCP bridge). Dispatch: `message.*`
added to `is_host_native` + a routing branch (with `&node.bus`); `prefs.catalog` delegated from the
`prefs.*` bridge. Exported from `lb_host`.

Gateway (`role/gateway/src/routes/catalog.rs`): `POST /message/render`, `POST /prefs/catalog`,
`PUT /message/catalog` — 1:1 with the host verbs; registered in `server.rs`. Dev-login member caps
gained `message.render` / `message.render_recipient` / `prefs.catalog` / `message.set_catalog`.

Client (`ui/src/lib/prefs/`): `catalog.generated.ts` (generated), `renderMessage.ts` (renders the
pure text/plural/select messages with `intl-messageformat` over the generated catalog + the fallback
chain — typed date/quantity placeholders stay the host's job via `format.*`).

## Decisions (recorded)

- **Canonical unit per dimension.** A `{v, quantity, dim}` placeholder carries the canonical value, so
  the renderer converts from `Dimension::canonical_unit()` (celsius / m/s / meter / kg / pascal / byte
  / ratio / second) to the recipient's display unit. Added beside the closed enum so it can't drift.
- **Per-key override merge is read-merge-write** in `catalog_set`, not a record-level `MERGE` (which
  would replace the whole `messages` object and lose untouched keys). The composite id keeps it
  idempotent; the scope's "LWW per message-key / two offline edits to different keys survive" holds.
- **`prefs.catalog` shares the `prefs.` prefix**, so it lands in the shipped `prefs.*` bridge and
  delegates to the catalog bridge — no second dispatch prefix for one read verb.
- **Fan-out is a second gate, not a second verb.** `message.render` is one verb; rendering for another
  recipient additionally checks `message.render_recipient` inside the verb (opaque, total deny).

## Tests (real infra, seeded via the real write path, no mocks — CLAUDE §9)

- **lb-prefs 15** — `catalog_test` 13 (plural one/other + `=0` en; es plural; select keyword+other;
  placeholder-through-format; fallback lang→en→key; unknown-locale→en; override-shadows-builtin;
  placeholder-failure `[ts]`; **placeholder-parity** datetime + quantity byte-identical to direct
  `format::*`; catalog-lint accepts builtins / rejects custom-formatter + MF2 + deep-nest) +
  `generated_catalog_test` 2 (drift byte-identity; builtins parse + share keys).
- **lb-host `catalog_mcp_test` 8** (through the real `call_tool` MCP bridge): render deny without
  grant; **render-for-another denied without the fan-out grant**; set_catalog deny for non-admin;
  catalog read deny + cross-ws deny; **two-workspace distinct overrides** (render + prefs.catalog);
  offline replay idempotent + per-key merge; **the fan-out headline** (2-member es/en team → two
  distinct renders, es `43,2 km/h`/Madrid vs en `23.3 kn`/New-York); set_catalog rejects out-of-subset.
- **gateway `catalog_routes_test` 7** (real gateway): set→render uses override; render-for-another
  needs the fan-out grant (403); set_catalog admin-deny (403); render deny (403); out-of-subset is
  400 not 403; two-session per-workspace render; **fan-out over the gateway** (2 members seeded via
  real `PUT /prefs` → two distinct renders).
- **UI `renderMessage.test.ts` 7** — the **intl-messageformat cross-check**: the SAME expected
  strings the Rust `catalog_test` asserts (en `1 pending item`/`5 pending items`, `=0` exact, es
  plural, select keyword+other, fallback, unknown-locale→en, override) render byte-identically under
  `intl-messageformat` — the host==client guarantee, guarded by two suites.

Green output pasted below.

## Green output

```
# cargo test --workspace  → exit 0 (whole workspace green)
# cargo fmt --check        → clean

# the catalog suites, isolated:
$ cargo test -p lb-prefs --test catalog_test --test generated_catalog_test
test result: ok. 13 passed; 0 failed; …   (catalog_test)
test result: ok. 2 passed; 0 failed; …    (generated_catalog_test)
$ cargo test -p lb-host --test catalog_mcp_test
test result: ok. 8 passed; 0 failed; …
$ cargo test -p lb-role-gateway --test catalog_routes_test
test result: ok. 7 passed; 0 failed; …

# UI:
$ pnpm test          → Test Files 33 passed (33) · Tests 242 passed (242)
                       (incl. src/lib/prefs/renderMessage.test.ts — 7 intl-messageformat cross-checks)
$ pnpm test:gateway   → 215 passed; 11 pre-existing environmental failures, NONE in catalog:
    - ProofPanel ×4 — `proof-panel wasm missing` (extension not built in this env)
    - StudioView ×1, SystemView ×1 — the flakes STATUS already documents
    - TelemetryView ×1 — `RequestInit: Expected signal … AbortSignal` (SSE env quirk)
    - App routing ×2 / ChannelView ×2 — channel-render timing ("button 'general' not found")
  All 11 are in files this slice did NOT touch (`git status` clean for each); the dev-login
  credentials unit test (`dev_login_carries_the_full_datasources_chain`) still passes — the 4
  added read/render caps are specific (no wildcard), so they cannot widen an admin/forged-verb test.
```

## Definition of done

- [x] Every verb the scope named built end to end (store→cap→MCP→gateway→client), each deny-tested.
- [x] FILE-LAYOUT respected (one verb/file, ≤400 lines of code; the parser file is ~430 with docs).
- [x] No `if cloud {…}`; pure render + compiled-in builtins run on every node (placement `either`).
- [x] Real infra, seeded via the real write path — no mocks, no `*.fake.ts`.
- [x] Mandatory categories (deny per verb, workspace isolation, offline/sync) + the scope's cases.
- [x] intl-messageformat cross-check confirms host==client before committing the parser.
- [x] Session doc (this) · public promotion (`public/prefs/prefs.md`) · STATUS.md moved.

## Related

- Scope: [i18n-catalogs](../../scope/prefs/i18n-catalogs-scope.md) · parent
  [user-prefs](../../scope/prefs/user-prefs-scope.md)
- Public: [prefs](../../public/prefs/prefs.md)
- Prior session: [lb-prefs](./lb-prefs-session.md)
