# Preferences, units & formatting (`lb-prefs`) — shipped

The canonical-in / localized-out boundary. Domain data is stored **canonically** (UTC instants,
SI/base units, locale-neutral enums) and **never** as a formatted string; presentation is resolved at
the boundary from a principal's resolved preferences and exposed as MCP tools so thin clients don't
re-implement timezone math, unit conversion, or date formatting.

Status: **shipped** — units + formatting core **and** the Phase-2 i18n MessageFormat catalogs
(server-localized notifications/emails, per-recipient fan-out). icu4x remains a named swap-in.

Scope: [`../../scope/prefs/user-prefs-scope.md`](../../scope/prefs/user-prefs-scope.md),
[`../../scope/prefs/i18n-catalogs-scope.md`](../../scope/prefs/i18n-catalogs-scope.md) ·
Sessions: [`lb-prefs`](../../sessions/prefs/lb-prefs-session.md),
[`i18n-catalogs`](../../sessions/prefs/i18n-catalogs-session.md)

## Records (SurrealDB, state)

- `user_prefs:[ws, user]` — per-(workspace, user) preferences. SCHEMAFULL, **every axis nullable** so
  "unset → inherit" is structural. The same global user has a distinct record per workspace (the hard
  wall holds structurally).
- `workspace_prefs:[ws]` — the per-workspace defaults (admins set them).

Composite, deterministic ids → an offline edit replays idempotently (upsert in place, last-writer-wins
on a contested axis). A `set` is a **MERGE patch**: present axes overwrite, absent axes are untouched.

## The closed axis set

Each axis is independently overridable (the decouple-the-axes design): `language` (enabled: en/es;
unknown → `en`), `timezone` (IANA id), `date_style` (`eu`|`iso`|`usa`), `time_style` (`h12`|`h24`),
`first_day_of_week` (`monday`|`sunday`), `number_format` (`dot_comma`|`comma_dot`|`space_comma`),
`unit_system` (`metric`|`imperial`), and `unit_overrides` (a **closed** `dimension → unit` map).

Dimensions (8): temperature, speed, distance, mass, pressure, data, percent, time.
Units (29) each belong to exactly one dimension. The vocabulary is generated to the client as
`ui/src/lib/prefs/dimensions.generated.ts` (regenerate: `cargo run -p lb-prefs --bin gen-prefs-ts`),
so the settings UI and the fieldConfig unit picker can't disagree with the server. A test fails on
drift.

## Resolution (pure)

```
request override  →  user pref  →  workspace default  →  built-in fallback (en, UTC, iso, h24, metric)
```

`resolve(&[Prefs]) -> ResolvedPrefs` folds the chain, **each axis independently** (a base-locale seed
fills only unset axes; `unit_overrides` merges per-dimension). A per-call request override (e.g.
"preview in es") wins for this response only and never writes the record. Pure: no I/O, no clock — the
same fold runs on edge and cloud, fully offline.

## Conversion & formatting

- **Conversion** is `uom`-backed (typed dimensional analysis): °C↔°F is **affine** (offset+scale, not a
  ratio — the classic bug uom prevents); a cross-dimension convert (temperature→speed) is rejected.
- **Formatting** renders from the resolved axes: number separators from `number_format`, date order +
  12/24h from `date_style`/`time_style`, and **timezone application over a UTC instant** (incl. DST)
  via `chrono-tz`'s embedded IANA database. `format.quantity` composes uom conversion + number render
  for the chart bridge (e.g. `12 m/s` → `43,2 km/h` for an es user, `23.3 kn` for an en+knots user).

> **icu4x note:** `uom` is the conversion engine as scoped. Date/number/tz rendering ships on the
> closed axes + `chrono-tz` (deterministic, locale-correct for en/es, zero CLDR data-size cost — which
> satisfies the Pi-profile constraint directly). `icu4x` is the **Phase-2 swap-in behind the same
> `format::*` signatures** for localized month/day NAMES and the ICU MessageFormat plural/select
> engine. See the session doc for the rationale.

## MCP surface

Gated tenant verbs (capability-checked, workspace-first, opaque deny):
- `prefs.get` — read OWN (`mcp:prefs.get:call`). Target forced to the caller's `sub`.
- `prefs.set` — write OWN (`mcp:prefs.set:call`). Forced to the caller's `sub`.
- `prefs.resolve` — read OWN (`mcp:prefs.resolve:call`), with an optional self-scoped override.
- `prefs.set_default` — **admin** (`mcp:prefs.set_default:call`).

Grant-free utility tier (pure CLDR/unit math over no tenant data → **no capability**; the caller passes
resolved prefs/axes inline):
- `format.datetime(instant, …)`, `format.number(value, …)`, `format.quantity(value, from_unit,
  dimension, …)`, `convert.unit(value, from, to)`.

## Gateway routes (1:1 mirror)

```
GET  /prefs            -> prefs.get          POST /format/datetime  -> format.datetime
PUT  /prefs            -> prefs.set          POST /format/number    -> format.number
POST /prefs/resolve    -> prefs.resolve      POST /format/quantity  -> format.quantity
PUT  /prefs/default    -> prefs.set_default  POST /convert/unit     -> convert.unit
```

The workspace + caps come from the session token, never the body. The utility tier is authenticated
(a token is required) but needs no capability.

## Tenancy, capabilities, sync

- **Workspace wall:** records live in the workspace namespace, keyed `[ws,user]`/`[ws]`. A resolve in
  ws-B can structurally never read ws-A.
- **Capabilities:** OWN reads/writes gated and self-scoped; `set_default` admin-gated; `format.*`/
  `convert.*` grant-free. Deny is opaque (no existence signal).
- **Sync/authority:** cloud-authoritative shared data with an edge read-cache; deterministic composite
  ids make offline edits idempotent on replay (LWW).

## i18n catalogs (Phase 2 — shipped)

Server-side, per-recipient localization of app-generated content (inbox items, emails, push), on the
**same renderer** the client uses.

- **MF1 dialect, one source.** A hand-written **ICU MessageFormat 1 subset** parser/renderer in
  `lb-prefs` (`catalog::render(key, args, override, resolved)`): argument, `plural` (`one`/`other` +
  exact `=0`/`=1`), `select` (arbitrary keywords + mandatory `other`), typed `{ts,date}` /
  `{n,number}` / `{v,quantity,<dim>}` placeholders (dispatched to the shipped `format::*`, never
  re-derived), one level of nesting, the `#` count token. en/es plural per CLDR 44. Anything outside
  the subset is a **catalog-lint error** (build-time test + rejected on write), never a silent parse.
- **Built-in en/es catalogs**, compiled in (`include_str!` of `catalog/builtin/{en,es}.mf`), version-
  stamped. The client bundle (`ui/src/lib/prefs/catalog.generated.ts`) is **generated** from them
  (`cargo run -p lb-prefs --bin gen-prefs-catalog`) and byte-identity drift-tested — host and client
  can't diverge by construction. The client renders the same MF1 with `intl-messageformat` (a
  cross-check test asserts byte-identical output).
- **Per-workspace override catalog.** `message_catalog:[ws, locale]` (SCHEMAFULL, flat dotted keys →
  MF1). A sparse override shadows a builtin key **for that workspace+locale only**. MERGE is per
  message-key (two offline edits to different keys both survive; same-key LWW; composite id →
  idempotent replay). Fallback: **override → builtin(lang) → builtin(en) → the key literal** — never
  blank, never panics; a placeholder failure renders the honest literal `[<arg>]`.
- **Per-recipient fan-out.** A notification to an N-member team renders **N times**, once per member's
  resolved prefs (a team has no language of its own).

### Catalog MCP surface + routes

```
message.render(key, args, recipient?)   POST /message/render    -> { text, locale_used, catalog_version }
prefs.catalog(locale)                   POST /prefs/catalog     -> { locale, catalog_version, messages, has_override }
message.set_catalog(locale, messages)   PUT  /message/catalog   -> 204  (+ "catalog changed" hint)
```

- **Capabilities:** `message.render` is member-level for the caller's OWN render (`mcp:message.
  render:call`); rendering **for another recipient** (the fan-out path) additionally requires
  `mcp:message.render_recipient:call` (the service/admin grant a producer holds, parallel to
  `prefs.get(other)`). `prefs.catalog` is member-level (read the merged map for own ws).
  `message.set_catalog` is **admin** (`mcp:message.set_catalog:call`), beside `prefs.set_default`.
  Render is gated (unlike grant-free `format.*`) because a catalog with overrides carries tenant data.
  Deny is opaque. An out-of-subset override is a `400` (authoring error), distinct from a `403`.
- **Motion:** `message.set_catalog` publishes `ws/{ws}/prefs/catalog-changed`
  `{ locale, catalog_version }` so open clients re-fetch — fire-and-forget; the store holds the state.

## Settings UI (shipped)

The client **settings surface** the scope named is shipped: a dedicated **Settings** nav surface
(`ui/src/features/settings/`) whose **Preferences** tab edits **all eight axes** — language, timezone,
date/time style, first-day-of-week, number format, unit system, and the closed dimension→unit overrides
— through `prefs.set` (own record) and, for an admin holding `mcp:prefs.set_default:call`, the workspace
default via `prefs.set_default` (a "My preferences / Workspace defaults" scope switch). The editor reads
`prefs.get` (only the explicitly-set axes; an unset axis shows "Inherit — <resolved>" via `prefs.resolve`
as the ghost), and the unit-override picker is generated from the SAME `dimensions.generated.ts`
vocabulary the server enforces (client and server can't disagree on the allowed set). The Settings
surface is always in the nav — every member may edit their own prefs; the workspace-default control is
cap-gated per-control (server-enforced). See `../external-agent/external-agent.md` for the sibling
**Agent** tab.

## Deferred (named follow-ups)

The **icu4x swap-in** (localized unit/month **names** + the full-CLDR plural engine for locales beyond
`one`/`other`, e.g. `pl`/`ar` + en/es CLDR data-slicing) behind the same `format::*`/`catalog::*`
signatures, and the **bootstrap-locale (pre-auth)** path + RTL layout application (the settings *editor*
is shipped; language before a principal exists is still deferred). MF2 is explicitly out (the dialect is
pinned to MF1 to match `intl-messageformat`).
