# i18n catalogs scope — MessageFormat catalogs + per-recipient server localization (prefs Phase 2)

Status: **scoped, implementation-ready (not built)** — every contract pinned, zero open questions (see
*Pinned contracts*). The units + formatting core (`lb-prefs`) shipped in S8; this is the
**Phase 2** deferred half named on that row: *"i18n catalogs + per-recipient fan-out, … settings/
bootstrap-locale UI."* See [`user-prefs-scope.md`](./user-prefs-scope.md) (the parent — all the design
decisions this doc builds on are already resolved there) and [`STATUS.md`](../../STATUS.md) line 433.
Stage: rides the **existing `lb-prefs` core crate** (a `catalog::*` module + `message.render` verb),
no new crate, no SDK/WIT change. **`icu4x` stays deferred** — see *Resolved decisions*.

One paragraph: the shipped core resolves a principal's language and formats dates/numbers/quantities;
what it can't yet do is **render a translatable message**. This slice adds the **ICU MessageFormat 1
(MF1)** catalog layer: built-in en/es catalogs compiled into the host (and shipped in the client
bundle), an optional per-workspace **override catalog** (a sparse `message_catalog` store record
shadowing specific keys), a pure `catalog::render(key, args, resolved_prefs)` that selects the message,
runs MF1 plural/select, and interpolates `format.*`-shaped date/number/quantity placeholders — and a
`message.render` MCP verb so server-generated content (inbox items, emails, push) is localized
**server-side in each recipient's resolved language**, fanning out per member. The dialect was already
pinned (MF1 / `intl-messageformat`); this slice implements it.

## Goals

- **MF1 catalog rendering, host + client, one source.** A `catalog::render(key, args, ResolvedPrefs) ->
  String` that: looks up `key` in the resolved language, falls back **language → `en` → the key itself**
  (never panics, never a blank), runs MF1 plural/select, and interpolates placeholders through the
  **already-shipped `format::*`** (a `{ts, date}` placeholder renders via `format::datetime` in the
  resolved tz/style; `{n, number}` via `format::number`; `{v, quantity, wind_speed}` via
  `format::quantity`). One message source serves the Rust host; the client parses the *same* MF1 with
  `intl-messageformat`.
- **Built-in catalogs (en/es), version-stamped.** Compiled into the host as an asset per locale; the
  same files ship in the client bundle. Each catalog carries a **version stamp**; the **host catalog is
  the source of truth** for any key that also appears in server-generated content, and host/client assert
  a compatible stamp (drift is the named risk, below).
- **Per-workspace override catalog.** A sparse `message_catalog:[ws, locale]` store record shadowing
  specific keys per workspace, without a rebuild. Merge is **override-shadows-builtin, then builtin,
  then key**. An edge that hasn't synced the override falls back cleanly (never blocks).
- **Server-side per-recipient fan-out.** A single notification to an N-member team renders **N times**,
  once per member's resolved prefs (language + tz + date/number style) — a team has no language of its
  own. This is the `message.render` verb called once per recipient by the outbox/inbox producer.
- **`message.render` MCP verb.** The universal contract: inbox/outbox producers, extensions, and the UI
  call it identically. Grant tier decided below.
- **`prefs.catalog(locale)` fetch verb** so a rich client can pull the merged (override-over-builtin)
  catalog for local formatting, refreshing on the "catalog changed" hint. Built-ins are already in the
  bundle; this fetches the **override** layer.

## Non-goals

Inherited verbatim from the parent scope — restated so this doc stands alone:

- **No machine translation in core.** `catalog::render` *selects and interpolates* an existing message;
  it never calls a model. (An extension may generate catalog entries via the AI gateway — out of scope.)
- **No TMS / per-string translation-management UI.** Catalog authoring (extraction, review, fuzzy match)
  is a later, separate tooling scope. This is the storage + render + fan-out primitive only.
- **No MF2.** The dialect is pinned to **MF1** to match `intl-messageformat`; icu4x's experimental
  MF2-leaning path is explicitly **not** used (silent plural/select divergence is the exact risk the
  one-source design exists to prevent).
- **No icu4x in this slice.** Localized month/day **names** and the icu4x plural engine stay a later
  swap behind the same `format::*` / `catalog::*` signatures (see *Resolved decisions* for why this is
  the long-term-correct order, not a shortcut).
- **No new datastore.** Override catalogs are ordinary SurrealDB records (rule #2). Built-ins + any CLDR
  are compiled in.
- **Beyond en/es is config, not code.** Adding `fr`/`de` is dropping a catalog file + its language enum
  value, not a code change. The mechanism must not assume exactly two languages.
- **RTL layout is the frontend's job.** `lb-prefs` exposes the resolved locale's base direction; applying
  it (mirroring) is out of this crate (flagged in the parent).

## Intent / approach

**A catalog is a workspace asset; rendering is a pure function of (key, args, resolved prefs).** Four
pieces, each one-responsibility-per-file under a new `catalog/` module in the existing `lb-prefs` crate:

1. **The MF1 parser/renderer** (`catalog/message.rs` + `catalog/plural.rs` + `catalog/interpolate.rs`) —
   pure, no I/O. A **hand-written MF1 subset parser** (~200 lines, no crate) — see *Resolved decisions*
   for why this beats icu4x/fluent long-term. It parses the **pinned closed subset** (defined exactly in
   *Pinned contracts* → MF1 grammar), selects the plural category for the resolved language via the
   hand-encoded en/es CLDR plural rules (full CLDR rules are the later icu4x swap, not this slice), and
   interpolates each placeholder by dispatching to the shipped `format::*`. **Reuse, do not re-derive:**
   date/number/quantity rendering already exists; this layer only routes placeholders to it.

2. **The built-in catalogs** — **MF1 text asset files** (`catalog/builtin/en.ftl`-style `.mf` files),
   compiled into the host via `include_str!` and **the single source** the client bundle is generated
   from (so drift is impossible by construction, not runtime-checked — see *Pinned contracts* → Storage
   & drift). Each catalog file carries a `catalog-version:` header line. Missing-key fallback lives here.

3. **The store layer** (`store/catalog_get.rs`, `store/catalog_set.rs`) — the sparse
   `message_catalog:[ws, locale]` record (composite id, SCHEMAFULL), MERGE upsert, workspace-scoped like
   `user_prefs`. The merge order (override → builtin → key) is a small pure fold.

4. **The boundary verbs** (`host/src/prefs/`): `message.render(key, args, recipient?)` and
   `prefs.catalog(locale)` and `message.set_catalog(patch)` (admin), wired into the existing prefs MCP
   dispatcher + gateway routes 1:1, exactly mirroring the shipped `prefs.*`/`format.*` wiring.

**Why the closed-axis renderer stays the formatting engine (not icu4x, this slice).** The shipped core
already produces `43,2 km/h` / `06/27/2026 08:30` correctly for en/es from the closed axes with **zero
CLDR data-size cost** — the Pi-profile constraint (README §10). icu4x only adds value for *localized
month/day names* and *full-CLDR plural rules across many locales*; for two seed languages the plural
rules are trivially encodable and MF1 needs no more. So Phase 2 ships MF1 catalogs on the existing
renderer; icu4x remains a drop-in behind `format::*`/`catalog::*` when a locale count or a
month-name requirement actually demands it. This is the long-term-correct sequencing: **the axis is the
contract, the renderer is replaceable** — we don't pay CLDR's data cost until a feature needs it.

**Server-side fan-out is a loop, not a new mechanism.** The outbox/inbox producer already has the
recipient list; it calls `message.render` once per recipient under **that recipient's** resolved prefs.
`message.render(recipient?)` resolves the named recipient's prefs (admin/service-gated — a producer
rendering *for* a user, distinct from a user reading their own) or the caller's own by default.

## Pinned contracts (implementation-ready — zero open questions)

Everything a coding session needs pinned so it never has to stop and ask. These are decisions, not
options.

**MF1 grammar — the exact closed subset.** The hand-written parser accepts exactly this and nothing else;
anything outside is a **catalog-authoring error caught by a catalog-lint test**, never a silent parse:

```
message    := (text | placeholder)*
placeholder:= "{" arg ("," fmt)? "}"
arg        := IDENT                       // e.g. name, ts, n
fmt        := "plural" "," plural_body    // {n, plural, one{…} other{…}}
            | "select" "," select_body    // {kind, select, alert{…} other{…}}
            | "date"                       // {ts, date}       → format::datetime
            | "number"                     // {n, number}      → format::number
            | "quantity" "," DIMENSION     // {v, quantity, wind_speed} → format::quantity
plural_body:= (CATEGORY "{" message "}")+  // CATEGORY ∈ {one, other} (+ exact "=0","=1" literals)
select_body:= (KEYWORD "{" message "}")+   // arbitrary keywords + mandatory "other"
# inside a plural arm renders the plural number via format::number.
```

- **Supported:** argument, `plural` (categories `one`/`other` + exact-value `=0`/`=1`), `select`
  (arbitrary keywords, `other` mandatory), one level of nesting (a `select`/`plural` arm may contain
  placeholders and one nested `plural`/`select`), the `#` count token, and the three typed placeholders
  above. **Not supported (lint rejects):** custom formatters, MF2 syntax, `offset:`, `selectordinal`,
  escape via `'…'` quoting beyond a literal `'{'`/`'}'`, deeper than one nest. Adding any of these is a
  deliberate grammar change, not ad-hoc.

**Plural rules — hand-encoded, en/es, per CLDR 44.** `catalog/plural.rs` encodes exactly:
- **en:** `n == 1 → one`, else `other`.
- **es:** `n == 1 → one`, else `other` (Spanish cardinal has the same two categories in CLDR).
- Exact-value arms (`=0`,`=1`) are matched **before** category rules. A language with more categories
  (e.g. `pl`, `ar`) is the trigger to swap in icu4x's CLDR plural engine — flagged, not hand-extended.

**Storage & drift — one source, generated client, no runtime handshake.** Built-in catalogs are the
`.mf` text files; a `gen_catalog` bin (twin of the shipped `gen_ts`) generates
`ui/src/lib/prefs/catalog.generated.ts` from them, and a **drift test re-runs the generator and asserts
the on-disk client file is byte-identical** — exactly the shipped `generated_ts_test` pattern. So host
and client can't diverge *by construction*; the `catalog-version:` header is a human-facing stamp echoed
in responses, **not** a runtime compatibility gate (there is nothing to gate — same bytes both sides).

**`message_catalog` record — flat map, pinned schema:**
```surql
DEFINE TABLE  message_catalog SCHEMAFULL;
DEFINE FIELD  ws       ON message_catalog TYPE string;
DEFINE FIELD  locale   ON message_catalog TYPE string;   -- "en" | "es" | …
DEFINE FIELD  messages ON message_catalog TYPE object;    -- flat: { "alert.threshold_crossed": "<MF1>" }
-- id = message_catalog:[ws, locale]  (composite, deterministic)
```
Keys are **flat dotted strings** (`alert.threshold_crossed`), never nested objects. A key present in
`messages` overrides that builtin key **for this workspace+locale only**; an absent key falls through to
builtin, then to the key literal. MERGE upsert; **LWW is per message-key** (each key's last write wins on
replay — a rolled-up snapshot record, so two offline edits to *different* keys both survive; same-key
conflict → later write wins, tie-break by actor id).

**Verb signatures — pinned DTOs:**
```rust
// message.render — gated mcp:message.render:call; recipient!=self needs the service/admin grant
struct MessageRenderReq { key: String, args: JsonMap, recipient: Option<UserId> } // ws from token
struct MessageRenderRes { text: String, locale_used: String, catalog_version: String }
// prefs.catalog — member-level; returns the MERGED (override-over-builtin) map for own ws
struct CatalogReq { locale: String }
struct CatalogRes  { locale: String, catalog_version: String, messages: JsonMap /*merged*/, has_override: bool }
// message.set_catalog — admin-gated; sparse patch merged into messages
struct SetCatalogReq { locale: String, messages: JsonMap } // ws from token
```
- `recipient` is a bare `UserId`; the workspace is always the caller's token ws (a producer can't render
  a foreign-ws recipient — the wall holds). Default (`None`) = the caller.
- `prefs.catalog` returns the **merged** map (not override-only); an unknown locale (`"fr"`) returns the
  **`en` builtin** with `has_override:false` (never 404, never empty — the no-block rule).

**Placeholder-failure contract — never abort, never panic.** If a placeholder's `format::*` call errors
(null/out-of-range `ts`, unknown unit), the renderer substitutes the literal `[<arg>]` (e.g. `[ts]`) and
continues; the message is always returned. The "never panics, never blank" guarantee covers *both* key
fallback and placeholder interpolation.

**"Catalog changed" motion — pinned key + payload** (mirrors the shipped "prefs changed" hint):
```
key:     ws/{ws}/prefs/catalog-changed
payload: { "locale": "es", "catalog_version": "…" }   // fire-and-forget; clients re-fetch prefs.catalog
```
Published by `message.set_catalog` after the store write. State stays in the store; this is only the hint.

**Placeholder-parity test — pinned meaning.** "Identical" = **byte-for-byte equal output string** between
`message.render` of a one-placeholder message and the direct `format::datetime`/`format::quantity` call
with the same resolved prefs, asserted across ≥10 locale/tz/style/dimension combinations. This is the
one-implementation guarantee: the message layer must dispatch to `format::*`, never fork it.

## How it fits the core

- **Tenancy / isolation:** `message_catalog:[ws, locale]` lives in the **workspace namespace** — ws-A's
  override never shadows ws-B's builtin. Built-ins are compiled-in (shared, not tenant data).
- **Capabilities:**
  - `message.set_catalog` (write a workspace override) — **admin-gated**, beside `prefs.set_default`.
  - `prefs.catalog` (read the merged catalog for own workspace) — **member-level** (a member must read
    it to render locally), mirroring the member-level `prefs.resolve`.
  - `message.render` — **decision: gated `mcp:message.render:call`, member-level for own-prefs render;
    rendering *for another recipient* (the fan-out path) requires the service/admin grant** the outbox
    producer already holds (it is producing content on the recipient's behalf, like `prefs.get(user)` of
    another user). *Not* grant-free like `format.*`: `format.*` touches no tenant data, but a catalog with
    workspace overrides **does** (override strings are tenant content), so render is gated. Deny is opaque.
- **Placement:** `either` — pure render + compiled-in builtins run on every node, fully offline; the
  override fetch is a store read the edge caches. No role branch.
- **MCP surface:** `message.render(key, args, recipient?)`, `prefs.catalog(locale)`,
  `message.set_catalog(patch)` — added beside the shipped 8 prefs verbs.
- **Data (SurrealDB):** `message_catalog` (SCHEMAFULL, composite id `[ws, locale]`, sparse key→message
  map). State. Built-in catalogs + plural rules are compiled in, not records.
- **Bus (Zenoh):** a **"catalog changed" hint** on `message.set_catalog` so open clients re-fetch +
  re-render — ordinary motion the verb publishes (same pattern as the shipped "prefs changed" hint),
  never this module using the bus as a store.
- **Sync / authority:** override catalogs are **cloud-authoritative** shared workspace assets (README
  §6.8) with an edge read-cache; the composite id makes an offline override edit idempotent on replay
  (LWW per key). An edge without the synced override falls back to builtin — never blocks.
- **Secrets:** none.

## Example flow

Extends the parent's wind-speed example, step 4 (the notification), made concrete:

1. A reading crosses a threshold; the workflow notifies a 2-member team (User A: `es`, `Europe/Madrid`,
   EU date; User B: `en`, `America/New_York`, USA date, wind override `knots`).
2. The outbox producer holds catalog key `alert.threshold_crossed` = (en) `"{name} exceeded {limit,
   quantity, wind_speed} at {ts, date}"`, (es) the Spanish translation, same placeholders.
3. **Per recipient**, it calls `message.render("alert.threshold_crossed", {name, limit: 12.0, ts},
   recipient: A)` → resolves A's prefs → **es** message, `format.quantity` → `43,2 km/h`,
   `format.datetime` → `27/06/2026 14:30` (Madrid). Then `recipient: B` → **en** message,
   `23,3 kn` (B's knots override), `06/27/2026 08:30` (New York).
4. **Two distinct renders from one canonical event** — no client localized either notification.
5. A workspace admin has set a `message_catalog:[ws, es]` override for `alert.threshold_crossed`
   ("brand voice" wording); A's render uses the override, B's `en` render is unaffected, and a *third*
   workspace's members are untouched.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` (real infra, seeded, no mocks — CLAUDE §9):

- **Capability deny** — `message.set_catalog` from a non-admin denied; `message.render` without its grant
  denied; **rendering for *another* recipient** without the service/admin grant denied (a member can
  render for *self* but not fan out for others); `prefs.catalog` of a foreign workspace denied. Deny
  leaks nothing (no key/existence signal).
- **Workspace isolation — specified, not generic.** The same key has a **different override** in ws-A and
  ws-B; assert `message.render`/`prefs.catalog` in ws-B returns ws-B's override and **never** reads
  ws-A's `message_catalog` (and vice versa); a ws-A override edit does not move ws-B's render. One-
  workspace tests are disallowed here. Across **store + MCP**.
- **Offline / sync** — an override edited offline applies locally at once, replays idempotently
  (composite-id upsert, LWW per key, no duplicate); an edge with no synced override falls back to builtin.

Key unit/integration cases:

- **MF1 plural/select in en and es** — correct category selection (`one`/`other` for en; es plural
  rules), nested select, and a placeholder rendered through `format.*` inside the message.
- **Fallback chain** — resolved language present → used; missing key in `es` → falls to `en`; missing in
  both → returns the **key** (never panic, never blank). A workspace override shadows the builtin **for
  that workspace only**.
- **Placeholder-failure honesty** — a null/out-of-range `ts` or unknown unit in a placeholder substitutes
  the literal `[<arg>]` and the message still renders (never panics, never aborts the whole string).
- **Catalog lint** — an authored message using an out-of-subset construct (custom formatter, MF2 syntax,
  deeper nest) fails a build-time catalog-lint test, not at render.
- **Server-side multi-recipient localization (the headline)** — one notification to a 2-member team whose
  members resolve to different languages produces **two distinct renders** (one es, one en), each in that
  member's tz/date/number style. Confirms per-member fan-out, not per-team.
- **Placeholder ↔ format parity** — a `{ts, date}` / `{v, quantity, dim}` inside a message renders
  **identically** to a direct `format.datetime` / `format.quantity` call with the same resolved prefs
  (the one-implementation guarantee — the message layer must not fork formatting).
- **Drift guard** — re-run `gen_catalog` and assert `ui/src/lib/prefs/catalog.generated.ts` is
  byte-identical on disk (the shipped `generated_ts` pattern): host and client catalogs are the *same
  bytes* by construction, so drift fails the test rather than reaching a release.

## Risks & hard problems

Most are inherited and **already resolved** in the parent (MF1 dialect pinned, source-of-truth = host,
Pi data-size handled by not pulling CLDR) or pinned in *Pinned contracts* above. What remains genuinely
live for this slice:

- **MF1 dialect fidelity — resolved by owning the subset.** Rather than *hoping* two parsers agree, the
  host and client render the **same generated catalog** and the subset grammar is one we define (see
  *Pinned contracts* → MF1 grammar), so `intl-messageformat` and the hand-written Rust parser only ever
  see constructs both implement. The placeholder-parity + plural tests are the standing contract. Not an
  open risk — a closed one, guarded by tests.
- **Plural rules for the seed languages — resolved, flagged for the swap.** en/es categories are
  hand-encoded per CLDR 44 (`n==1→one`, else `other`, both languages). Deliberately not full-CLDR: the
  first locale needing more categories (`pl`, `ar`) triggers the clean icu4x plural swap.
- **Catalog version drift — impossible by construction.** The client bundle is *generated* from the host
  `.mf` files; the byte-identity drift test fails before a skewed release exists. The `catalog-version:`
  header is a human stamp echoed in responses, not a runtime handshake (there's nothing to reconcile).
- **Override catalog vs offline.** An edge without the synced override must fall back to builtin, never
  block. Fallback order is load-bearing and tested.
- **Render grant vs `format.*` grant-free.** `format.*` is grant-free (no tenant data); `message.render`
  is **not**, because a workspace override *is* tenant content. Keep the boundary explicit so an audit
  doesn't assume render is free math.

## Resolved decisions

Inheriting the parent's resolved decisions (MF1 pinned, host source-of-truth, per-recipient fan-out,
sparse-override storage). New to this slice:

- **No new crate.** The catalog layer is a `catalog/` module inside the shipped `lb-prefs` crate — the
  record + resolution is already there; catalogs are the same asset/record model. (Not `lb-i18n`.)
- **MF1 engine: a hand-written subset parser (~200 lines), not a crate — the long-term-correct call.**
  No Rust crate matches `intl-messageformat`'s MF1 dialect: `icu4x`'s message path is MF2-leaning and
  experimental (the exact silent-divergence the design rejects, and it drags CLDR weight onto the Pi
  profile), and `fluent` is a *different* syntax (FTL), not MF1. Owning a small closed subset (grammar in
  *Pinned contracts*) means host and client parse **only constructs both implement**, carries **zero CLDR
  cost**, keeps **plural/select from day one** (alerts need "N items"), and leaves the icu4x plural engine
  a clean drop-in the first time a locale needs categories beyond `one`/`other`. Rejected: shipping
  plain-interpolation-only (can't pluralize) and pulling `icu_experimental` now (reintroduces the MF2
  risk + data weight before any feature needs it).
- **icu4x stays deferred — the long-term-correct order.** The shipped closed-axis renderer already
  formats en/es dates/numbers/quantities correctly with **zero CLDR data-size cost** (the Pi constraint).
  icu4x only earns its keep for localized month/day **names** and full-CLDR plural rules across many
  locales — neither of which MF1 en/es catalogs need. Shipping MF1 on the existing renderer now, behind
  the same `format::*`/`catalog::*` signatures, means the icu4x swap is a **pure internal replacement**
  the day a locale/month-name feature demands it — no contract change, no premature CLDR payload. Paying
  for icu4x now would add binary/data weight for capability we don't yet use. (This is the sequencing the
  user asked for: "do what's best long-term.")
- **`message.render` is gated (member for self, service/admin to fan out for others).** Distinct from the
  grant-free `format.*` because a catalog with workspace overrides carries tenant content, and rendering
  *for another recipient* is producing content on their behalf (parallel to `prefs.get(other)`).
- **Plural rules hand-encoded for en/es**, flagged as the icu4x swap point — not full CLDR this slice.

## Related

- [`user-prefs-scope.md`](./user-prefs-scope.md) — **the parent.** All the design decisions (axes,
  canonical storage, MF1 dialect, source-of-truth, fan-out, Pi data-size) are resolved there; this doc is
  the Phase-2 implementation scope for its deferred i18n half.
- [`flow-ts-display-scope.md`](./flow-ts-display-scope.md) — the first shipped consumer of `format.*`;
  the same `format::*` this slice routes MF1 placeholders through.
- `scope/inbox-outbox/` — the **primary consumer**: inbox items + outbox emails/notifications are the
  server-localized, per-recipient-fanned-out content surface this slice enables.
- `scope/frontend/collaboration-scope.md` — the client half: bundled catalogs, bootstrap locale
  (pre-auth), the settings surface, `prefs.catalog` override fetch, re-render on the "catalog changed"
  hint, and RTL layout application (the deferred UI named on the STATUS row alongside this).
- README **§6.5**/**§3.7** (MCP is the universal contract — `message.render` is a tool), **§6.8** (sync —
  override catalogs are cloud-authoritative shared assets), **§6.12** (docs/skills as shared workspace
  assets — the override-catalog model), **§7** (workspace = the wall), **§10** (minimal profile — why
  icu4x's CLDR stays deferred), **§3** (one datastore, state vs motion, canonical-not-formatted,
  capability-first).
- `key-stack.md` — the `lb-prefs` row already names icu4x + MessageFormat; no new row (this rides the
  existing crate). The icu4x cell stays accurate as the deferred swap-in.
