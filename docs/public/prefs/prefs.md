# Preferences, units & formatting (`lb-prefs`) — shipped

The canonical-in / localized-out boundary. Domain data is stored **canonically** (UTC instants,
SI/base units, locale-neutral enums) and **never** as a formatted string; presentation is resolved at
the boundary from a principal's resolved preferences and exposed as MCP tools so thin clients don't
re-implement timezone math, unit conversion, or date formatting.

Status: **shipped** — units + formatting core. The i18n MessageFormat catalogs (server-localized
notifications/emails) are deferred as named follow-ups (dialect already pinned, see below).

Scope: [`../../scope/prefs/user-prefs-scope.md`](../../scope/prefs/user-prefs-scope.md) ·
Session: [`../../sessions/prefs/lb-prefs-session.md`](../../sessions/prefs/lb-prefs-session.md)

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

## Deferred (named follow-ups)

i18n MessageFormat catalogs (server-localized notifications/emails/inbox, per-recipient fan-out, the
workspace override-catalog asset), the icu4x swap-in (localized unit/month names + MessageFormat
engine + en/es CLDR data-slicing), and the client settings/bootstrap-locale UI. The MessageFormat
**dialect is already pinned — ICU MF1, `intl-messageformat` on the client** — so catalog work starts
unblocked; the host MF1 parser is the remaining Phase-2 build item.
