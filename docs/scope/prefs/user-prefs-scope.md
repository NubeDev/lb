# User-prefs scope — canonical data in, localized presentation out

Status: **shipped (units + formatting core)** — `lb-prefs` crate + host verbs + gateway routes +
generated client constants are built, tested (deny + specified workspace-isolation + offline-replay +
affine-conversion + axis-independence + resolution-chain + locale-rendering incl. DST + canonical
guarantee, all on real infra), and promoted to [`public/prefs/prefs.md`](../../public/prefs/prefs.md).
See [`sessions/prefs/lb-prefs-session.md`](../../sessions/prefs/lb-prefs-session.md). The **i18n
MessageFormat catalogs** (server-localized notifications/emails) remain deferred — Phase 2, **now scoped
in [`i18n-catalogs-scope.md`](./i18n-catalogs-scope.md)** — but the
MessageFormat **dialect is now resolved/pinned** (see Resolved decisions). Stage: a **core crate**
(`lb-prefs`) — the preference record + resolution + locale/timezone/date/number formatting lands
alongside server-generated content (useful from **S2** inbox onward, naturally bundled into the
**S9** collaboration UI where a real principal can finally *own* a preference); the **unit-conversion
add-on** leans on the `unit:` tag from `scope/tags/` and so rides with **S8**.

One paragraph: a user picks a **language** (English/Spanish to start), a **timezone**, a **date/time
display style** (EU `DD/MM/YYYY`, ISO/International `YYYY-MM-DD`, USA `MM/DD/YYYY`; 12h/24h), and a
**unit system** (metric/imperial, with per-quantity overrides for temperature, wind, distance, …). The
platform stores all domain data **canonically** — UTC instants, SI/base units, locale-neutral enums —
and **never** stores a formatted string. Presentation is resolved at the boundary from the principal's
resolved preferences, and the **backend offers that resolution as MCP tools** so a thin client (a mobile
app, a webhook formatter, a server-sent email) does not re-implement timezone math, unit conversion, or
date formatting. Rich clients may still format locally from canonical data + the resolved prefs; the
point is that *no client is forced to*.

## Goals

- **One preference record per (workspace, user), resolved through a chain.** Request override → user
  pref → **workspace default** → built-in fallback (`en`, UTC, ISO, metric). Admins set workspace
  defaults; users override their own axes. Resolution is a pure function over the chain. **The request
  override is per-call and self-scoped** — a caller may pass `locale`/`tz`/`view=display` on a `format.*`
  or read verb to render *this response* differently (e.g. a public share link, or "preview in es"); it
  never writes the stored record and never sets another principal's presentation. Any verb accepting an
  override states it explicitly; absent one, the resolved prefs apply.
- **Decouple the axes — this is the core design insight.** `language`, `timezone`, `date_style`,
  `time_style` (12/24h), `first_day_of_week`, `number_format`, `unit_system`, and `unit_overrides`
  are **independent** fields, each separately overridable. "Spanish language **with** USA date format
  **and** metric units except wind in knots" must be expressible. A base locale (e.g. `es-ES`) only
  seeds sensible defaults for the unset axes; it never locks them together.
- **Canonical storage, always.** UTC instants (`datetime`), SI/base units, locale-neutral codes. The
  store and the bus carry canonical values; formatting happens only at presentation. A formatted string
  in a domain record is a bug.
- **Backend-mediated presentation as MCP tools** so every client is identical and thin clients are free:
  `format.datetime`, `format.number`, `format.quantity`, `convert.unit`, and the `prefs.get/set/resolve`
  verbs. Same contract for the UI, agents, extensions, and external clients (README §6.5/§3.7).
- **Server-generated content is localized server-side.** Emails, push notifications, and inbox-item
  render strings are produced in the *recipient's* resolved language, not the sender's — because there is
  no client to localize them. The message catalog is the same one clients use.
- **i18n catalogs are workspace assets, layered.** Built-in app catalogs (en/es) ship with each client
  and with the host; an optional per-workspace override catalog (in the store, a shared asset like
  docs/skills, README §6.12) lets a workspace localize app/extension-specific strings without a rebuild.

## Non-goals

- **No machine translation in core.** `lb-prefs` selects and interpolates an existing catalog message;
  it does not call a model to translate. (An extension may, via the AI gateway, produce catalog entries —
  out of this crate.)
- **No per-string translation-management UI / TMS.** Catalog *authoring* (extraction, review, fuzzy
  matching) is a tooling concern, not this storage+resolution primitive. A later, separate scope if ever.
- **No new locale/units datastore.** Catalogs and prefs are SurrealDB records/assets like everything else
  (rule #2). CLDR data is compiled into the binary via `icu4x`, not a runtime service.
- **No forcing all payloads through a formatter.** Canonical stays on the wire by default; formatting is
  opt-in per call. Auto-formatting every response would corrupt the canonical principle and break offline
  arithmetic on the client.
- **No SDK/WIT change.** Format/convert/prefs verbs are host MCP tools like any other; extensions consume
  them through the existing tool boundary.
- **Beyond two languages is config, not scope.** English + Spanish are the seed catalogs; adding `fr`,
  `de`, … is dropping a catalog file, not a code change. The *mechanism* must not assume exactly two.

## Intent / approach

**A preference is state; formatting is a pure function of (canonical value, resolved prefs).** Three
pieces:

1. **The record.** `user_prefs:[ws, user]` and `workspace_prefs:[ws]` (composite IDs, deterministic) —
   SCHEMAFULL, each axis nullable so "unset → inherit" is structural, not sentinel-encoded. `prefs.resolve`
   folds the chain into a fully-populated `ResolvedPrefs` with every axis decided. **The axis set is
   closed and pinned here — the canonical list, referenced everywhere else in this doc:** `language`,
   `timezone` (IANA id), `date_style` (`eu` | `iso` | `usa`), `time_style` (`h12` | `h24`),
   `first_day_of_week`, `number_format`, `unit_system` (`metric` | `imperial`), and `unit_overrides`
   (a closed map: `dimension → unit`, dimensions a named enum — temperature, wind_speed, distance, mass,
   pressure, … — never open free text). Adding an axis is a deliberate schema change, not an ad-hoc field.

2. **The formatting library** (pure, no I/O, no model) — `icu4x` (Unicode's official CLDR-backed Rust
   i18n: locale-aware date/time/number/list formatting and message interpolation) for *rendering*, and
   **`uom`** (compile-time dimensional analysis) for the *correctness* of unit conversion. Clean split:
   `uom` guarantees `m/s → km/h → knots` is dimensionally sound; `icu4x` renders the result with the
   right decimal separator, digit grouping, and localized unit display name. Timezone application uses
   `icu4x` time zones (CLDR tz) over the stored UTC instant — IANA id in, wall-clock out.

3. **The boundary verbs** wrap (1)+(2) as MCP tools. Because the library is pure and compiled into every
   node, the *same* formatting runs on edge and cloud (symmetric) and **fully offline** — no round-trip
   to the hub to format a date.

**Why backend conversion (the user's instinct, confirmed) — but opt-in.** A mobile app, an email relay,
and a webhook do not each carry CLDR data or unit tables; making the host expose `format.*`/`convert.*`
means one correct implementation, not N drifting ones. But the *default wire format stays canonical*: the
client asks for a formatted view when it wants one (a `format.*` call, or a `?view=display` projection on
a read), and rich clients that already bundle the same `icu4x` data may format locally. This satisfies
"clients shouldn't have to" without making them *unable* to.

**Language: backend-aware, client-rendered, server-localized where there is no client.** The "normal"
split — UI strings live in client bundles keyed by the resolved locale; the backend stores the *choice*
and owns the *same catalogs* for content it generates itself (notifications/emails/inbox renders). The
catalog format is **ICU MessageFormat** (the portable standard: plurals, gender, embedded
number/date skeletons) so one message source can serve Rust host and TS client. **The exact dialect is
pinned, not assumed** — `intl-messageformat` on the client implements MF1, and `icu4x`'s MessageFormat
support has historically been experimental; host and client must parse the *same* dialect or
plural/select handling silently diverges (see Risks). Resolve to a verified shared parser before build.

**Units tie into tags, not a parallel system.** A value's *source* unit is whatever the data already
declares — for ingest series it is the `tag:['unit','celsius']` from `scope/tags/`; for a domain record
the producing extension states the quantity's unit. `format.quantity(value, from_unit, dimension)`
reads `to` from the resolved `unit_system`/`unit_overrides`. No new place to record "what unit is this."

**Rejected alternatives:**
- *Store localized/formatted values.* Rejected — un-reconvertible, un-sortable, un-summable, and wrong
  the moment a user changes a preference or a second user reads the same record. Canonical-only is
  non-negotiable.
- *Client-only localization (no backend formatting).* Rejected — forces every client (mobile, email,
  webhook, agent) to re-implement tz/unit/date logic and drift; defeats the user's explicit ask.
- *A separate i18n/units microservice or third-party locale API.* Rejected — violates one-datastore /
  symmetric-nodes and breaks offline; `icu4x` compiles CLDR in, `uom` is a pure crate.
- *Fold language into a single `locale` field that also dictates date/units.* Rejected — the axes must be
  independent (the core insight); a single locale is only a *seed* for unset axes.
- *Roll our own conversion factors.* Rejected — `uom`'s typed dimensional analysis prevents the classic
  bug (adding a temperature *offset* like a *scale* factor); hand-rolled tables get °C↔°F wrong.

## How it fits the core

- **Tenancy / isolation:** `user_prefs` and `workspace_prefs` live in the **workspace namespace** keyed
  `[ws, user]` / `[ws]` — a user's prefs in ws-A are a different record from the same user's prefs in
  ws-B (the same global identity, distinct per-workspace preference; the hard wall holds structurally).
  Override catalogs are workspace assets, namespace-scoped.
- **Capabilities:** `prefs.get`/`prefs.resolve` (read **own**), `prefs.set` (write own), `prefs.set_default`
  (workspace default — **admin-gated**), and the stateless `format.*`/`convert.*` (a low/no-grant utility
  tier — they touch no tenant data, only CLDR/unit math). Deny is opaque: without `prefs.get` you cannot
  read another user's prefs or even confirm they exist. The mandatory deny-test covers each verb,
  especially `set_default` from a non-admin and `get` of another user's record.
- **Placement:** `either` — a core crate compiled into every node; pure formatting runs anywhere,
  including fully offline. No role branch.
- **MCP surface:** `prefs.get(user?)`, `prefs.set(patch)`, `prefs.resolve(user?)`,
  `prefs.set_default(patch)` (admin), `format.datetime(instant, opts?)`, `format.number(n, opts?)`,
  `format.quantity(value, from_unit, dimension, opts?)`, `convert.unit(value, from, to)`. The universal
  contract — UI, agents, extensions, external clients call it identically.
- **Data (SurrealDB):** `user_prefs` (SCHEMAFULL, composite id `[ws,user]`, nullable axes),
  `workspace_prefs` (composite id `[ws]`), and the optional `message_catalog` override entries as a
  workspace asset (store-backed). All **state**. Built-in catalogs + CLDR are compiled in, not records.
- **Bus (Zenoh):** none directly — prefs are state. A "prefs changed" hint so open clients re-render is
  ordinary motion the caller publishes (or a `LIVE SELECT` store convenience), never this crate using the
  bus as a store. State-vs-motion stays clean.
- **Sync / authority:** user/workspace prefs are **shared workspace/identity data → cloud-authoritative**
  with an edge read-cache (README §6.8); the deterministic composite ids make an offline pref edit
  idempotent on replay (last-writer-wins on the rare contested field). A user changing prefs offline sees
  it locally at once and it syncs on reconnect.
- **Secrets:** none.

## Example flow

A wind-speed reading is stored once and rendered for two different users:

1. An ingest producer stores a sample canonically: value `12.0`, with `tag:['unit','meter_per_second']`
   and `tag:['dimension','wind_speed']` on the series (the tags scope's `unit:` tag is the source unit).
   Nothing formatted is stored.
2. **User A** (`prefs: es-ES seed → language es, tz Europe/Madrid, date EU, units metric`) opens a
   dashboard. The client calls `format.quantity(12.0, "meter_per_second", "wind_speed")`; the host
   resolves A's prefs → metric → renders **`43,2 km/h`** (comma decimal, localized unit name via `icu4x`,
   conversion via `uom`).
3. **User B** (`prefs: language en, tz America/New_York, date USA, units imperial, wind override knots`)
   views the same series. `format.quantity(12.0, "meter_per_second", "wind_speed")` resolves B's
   `unit_overrides.wind_speed = knots` → **`23.3 kn`**. Same canonical `12.0`; two presentations; zero
   client-side conversion code.
4. The reading crosses a threshold and the workflow notifies both. **Server-side**, the host renders the
   inbox/email body from the **ICU MessageFormat** catalog: User A receives the Spanish message with the
   instant formatted `27/06/2026 14:30` in `Europe/Madrid`; User B receives the English message with
   `06/27/2026 08:30` in `America/New_York`. No client was involved in localizing the notification.
5. A thin **mobile app** with no CLDR bundle shows the same dashboard purely by calling `format.*` — it
   ships no timezone/unit/date logic of its own.

## How the UI handles this (the client half of the contract)

This crate owns the **contract**; the client owns the **consumer**. That half is otherwise orphaned —
`frontend-scope.md`, `ui-design-scope.md`, and `collaboration-scope.md` say nothing about locale today —
so the client responsibilities are named here and handed off via a forward-reference from those docs.
Detailed UI work lands in the **frontend scope**; this section pins what it must cover so nothing falls
between the two.

- **Bootstrap locale (pre-auth).** Login screens, error pages, and a mobile app's first paint need a
  language *before any principal exists*, so they can't read `prefs.resolve`. Resolution order at
  bootstrap: explicit device/app setting → browser `Accept-Language` → an anonymously-fetched workspace
  default (if the workspace is known pre-login) → built-in `en`. Once authenticated, the principal's
  resolved prefs take over and the UI re-renders.
- **Catalog bundling + override fetch/merge.** Built-in catalogs (en/es) ship in the client bundle for
  zero-latency first paint; the per-workspace **override** catalog is a server asset. A rich client that
  formats locally fetches it via a `prefs.catalog(workspace, locale)` verb (or the synced-asset path of
  §6.12), merges override-shadows-builtin, and refreshes on the "catalog changed" hint. An edge that has
  not synced overrides falls back to the built-in (then the key) — never blocks.
- **Version / source-of-truth.** Built-in catalogs ship in *two* places (client bundle + host), so they
  can drift across releases and give one user two different strings for the same event (UI vs the
  server-rendered notification). The catalog carries a **version stamp**; host and client assert a
  compatible version, and the **host catalog is the source of truth** for any string that also appears in
  server-generated content. (Added to Risks.)
- **The settings surface.** Where the user edits prefs: the timezone picker validates against the IANA
  list, and the unit-override controls are generated from the **same closed dimension enum** the server
  enforces (shared via a small generated constants module, not hand-duplicated) so client and server
  can't disagree on the allowed set.
- **Re-render on change.** `prefs.set` emits the "prefs changed" hint (the bus motion / `LIVE SELECT`
  noted under *Bus*); the client subscribes and re-resolves + re-renders, so a preference change is live
  on the same device and on the user's other open sessions, not stuck until reconnect.
- **RTL is a UI-layer responsibility, flagged now.** en/es are both LTR so direction is invisible today,
  but the "must not assume exactly two languages" promise extends to direction: adding `ar`/`he` later is
  a `dir=rtl` *layout* concern, not just a catalog drop. `lb-prefs` exposes the resolved locale's base
  direction; **applying it (mirroring layout) is the frontend scope's job**, explicitly out of this crate.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny** — each verb refused without its grant: `prefs.get` of *another* user's prefs denied;
  `prefs.set_default` from a non-admin denied; `prefs.set` cannot write a different user's record. Deny
  leaks nothing (no existence signal).
- **Workspace isolation — specified, not generic.** The same global user has `user_prefs` in **both**
  ws-A and ws-B with **different** values; assert a `prefs.resolve` in ws-B returns ws-B's values and
  **never** reads ws-A's record (and vice versa), and that a workspace-default change in ws-A does not
  move ws-B's resolution. A test using one workspace would pass even with a leak — disallowed. Across
  **store + MCP**.
- **Offline / sync** — a pref edited offline applies locally immediately, then replays idempotently to
  the hub (composite-id upsert, LWW on a contested field; no duplicate record).

Key unit/integration cases:

- **Axis independence** — `language=es` + `date_style=usa` + `unit_system=metric` +
  `unit_overrides.wind_speed=knots` all resolve independently; a base-locale seed fills *only* unset axes
  and never overrides a set one.
- **Resolution chain** — request override > user > workspace default > built-in fallback, at each level.
- **Conversion correctness (the bug-prone part)** — °C↔°F uses offset *and* scale (0 °C = 32 °F,
  100 °C = 212 °F), m/s↔km/h↔knots, m↔ft/mi; round-trip stability within tolerance; `uom` rejects a
  cross-dimension convert (temperature→speed) at the type level.
- **Locale rendering** — decimal/grouping separators per locale (`43,2` vs `43.2`), date styles
  (EU/ISO/USA), 12h/24h, and tz application over a stored UTC instant (incl. a DST boundary).
- **i18n catalog** — ICU plural/format selection in en and es; missing-key falls back to `en` (then to
  the key itself), never panics; a workspace override entry shadows the built-in for that workspace only.
- **Server-side multi-recipient localization** — a single notification to a 2-member team whose members
  resolve to *different* languages produces **two distinct renders** (one es, one en), each in that
  member's tz/format — confirming server-generated content fans out per-member, not per-team.
- **Canonical guarantee** — a round-trip store→read returns the canonical value unchanged; assert no
  formatted string is persisted in a domain record (a lint/test on the example path).

## Risks & hard problems

- **Temperature is the classic conversion trap.** °C↔°F is an **affine** transform (offset + scale), not
  a ratio — a "convert by multiplying a factor" shortcut is wrong for temperature and right for speed,
  which is exactly why hand-rolled tables fail. `uom`'s typed quantities encode this; do **not** bypass it
  with a factor map. This is a correctness bug if gotten wrong, not a nicety.
- **`icu4x` / CLDR data size on the Pi profile.** Full CLDR is large; the minimal profile (README §10)
  can't carry every locale. Decision of this slice: compile in **only the enabled languages' data**
  (en/es today) via `icu4x` data slicing, and treat adding a language as a build-config change that also
  pulls its CLDR slice — not a runtime download.
- **Catalog layering vs offline.** The override catalog is a synced workspace asset; an edge that hasn't
  synced it must fall back to the built-in (then the key), never block or error. Fallback order is
  load-bearing and tested.
- **Client/host catalog version drift.** Built-in catalogs ship in *two* copies — the client bundle and
  the host — so a release skew can render the same key two different ways (the UI string vs the
  server-generated notification for the same event). Each catalog carries a **version stamp**; host and
  client assert compatibility, and **the host catalog is the source of truth** for any key that also
  appears in server-generated content. Same class of cross-implementation drift as the MessageFormat
  dialect risk below — don't let two copies silently diverge.
- **MessageFormat dialect drift (verify before build).** "One message source, host + client" only holds
  if both parse the *same* MessageFormat dialect. `intl-messageformat` (TS) is MF1; `icu4x` MessageFormat
  support has been experimental and MF2-leaning. A mismatch makes plural/select rules diverge silently.
  Pin a verified shared dialect/parser (or a tested subset) **before** committing the catalog format —
  this is exactly the drift the whole design is trying to prevent, so it can't be left implicit.
- **Source-unit provenance.** `format.quantity` is only correct if the caller's `from_unit` is truthful.
  For ingest this is the `unit:` tag; for arbitrary records the producer must declare it. A wrong source
  unit yields a confidently-wrong conversion — so the contract requires an explicit `from_unit`/
  `dimension`, never a guess, and an unknown unit is a hard error, not a passthrough.
- **Timezone correctness across DST and historical zones.** Storing UTC is the easy half; rendering wall
  time needs current CLDR tz data and DST rules. Pin to `icu4x`'s tz database; a stale binary renders a
  recently-changed zone wrong — flag tz data as part of the same data-update story as CLDR.
- **"Resolved prefs" caching vs change latency.** Resolution is cheap but called often; if cached per
  session, a `prefs.set` must invalidate or clients render stale until reconnect. Lean: resolve per
  request (it's pure + fast) and skip a cache until measured need.
- **Per-quantity override sprawl.** `unit_overrides` is a map (dimension → unit); keep the dimension set
  closed and named (temperature, wind_speed, distance, mass, pressure, …) so it stays a small enum, not
  an open free-text map that drifts.

## Resolved decisions

No blocking open questions — these are the long-term answers the build follows.

- **Crate name: `lb-prefs`.** The record + resolution is the durable part; formatting (`icu4x`/`uom`) is its
  library. (Not `lb-locale` — preferences are the spine, formatting the consumer.)
- **`format.*`/`convert.*` are a grant-free utility tier.** They touch no tenant data (pure CLDR/unit math),
  so they need no capability; `prefs.get`/`resolve`/`set` (which read/write a tenant record) stay gated.
  Revisit only if a blanket `mcp:format:call` proves necessary for audit consistency.
- **Catalog storage: per-locale asset blob for built-ins, sparse record overrides for workspace
  customizations.** Built-in en/es ship as compiled-in/asset catalogs; a workspace override is a sparse
  `message_catalog` record shadowing specific keys.
- **Canonical unit per dimension: store the source unit as a tag and convert on read.** Lossless and honest
  about provenance; do **not** normalize ingest to one unit at write time. Revisit only if read-time
  conversion cost shows up in ingest dashboards.
- **`number_format` is its own axis, seeded by the base locale** (the decouple-the-axes goal) — a user can
  have Spanish text with English-style `.` decimals.
- **`first_day_of_week` and the 12h/24h default are settable, region-seeded** — an unset axis derives from
  the region locale; a set one is honored independently.
- **Server-generated content fans out per recipient member to each member's resolved language** — a team has
  no language of its own; a notification to a 2-member team renders twice (once per member's prefs).
- **Formatting library split, as built (icu4x deferred to Phase 2).** `uom` is the conversion engine
  exactly as scoped (it carries the affine-temperature correctness). For *rendering*, the shipped core
  derives number separators + date order + 12/24h **from the closed axes** and applies the timezone over
  a UTC instant (incl. DST) via **`chrono-tz`** — deterministic, locale-correct for the enabled en/es
  set, and with **zero CLDR data-size cost**, which satisfies the Pi-profile "compile in only enabled
  locales" risk directly without any icu data-slicing config yet. `icu4x` is the Phase-2 **swap-in
  behind the same `format::*` signatures** where it genuinely earns its keep: localized month/day
  **names** and the ICU MessageFormat plural/select engine. The axis is the contract; the renderer is
  replaceable.
- **MessageFormat dialect resolved/pinned (the Risk, closed before catalog work).** The portable
  dialect is **ICU MessageFormat 1 (MF1)**; the client parser is **`intl-messageformat`** (the de-facto
  TS MF1 implementation). The host catalog is authored in the same MF1 syntax and rendered with an
  MF1-compatible Rust parser — **not** icu4x's experimental/MF2-leaning path (which would silently
  diverge on plural/select). For any key that also appears in server-generated content, the **host
  catalog is the source of truth**. Catalogs themselves are Phase-2 work, but the dialect is no longer
  an open risk — Phase 2 starts unblocked.

## Related

- [`flow-ts-display-scope.md`](./flow-ts-display-scope.md) — the first CONSUMER of `format.datetime` in
  the viz layer: a flow node's canonical epoch-seconds `ts` rendered in the viewer's resolved prefs via
  the field-config bridge (the "when lb-prefs ships, format.ts becomes the real dispatch" swap). Also
  made `prefs.get`/`resolve`/`set` member-level (a member must resolve their own prefs to render).
- README **§6.5** (MCP is the universal contract — format/convert/prefs are tools), **§3.7** (same),
  **§6.8** (sync/authority — prefs are cloud-authoritative shared data), **§6.12** (docs/skills as shared
  workspace assets — the override-catalog model), **§7** (workspace = the wall; user is a global identity
  with per-workspace prefs), **§10** (minimal profile — CLDR data-size constraint), **§3** (one
  datastore, state vs motion, canonical-not-formatted, capability-first).
- `scope/tags/tags-scope.md` — the `unit:` / `dimension:` tag is the **source-unit provenance** that
  `format.quantity`/`convert.unit` read; units reuse the tag model rather than a parallel one.
- `scope/ingest/ingest-scope.md` — the primary producer of canonical, unit-tagged `series` that this
  crate renders per-user.
- `scope/inbox-outbox/` — inbox items + outbox emails/notifications are the **server-localized** content
  surface (rendered in the recipient's language, not the sender's).
- `scope/frontend/collaboration-scope.md` — the S9 UI that surfaces a real principal who can own a
  preference, and the client side of the language catalogs.
- `key-stack.md` — adds the `lb-prefs` crate, `icu4x` (CLDR i18n/formatting), and `uom` (unit conversion)
  rows.
