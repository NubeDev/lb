---
name: prefs
description: >-
  Manage Lazybones user/workspace preferences and render canonical data through the backend formatting
  verbs over the node gateway. Read/write a principal's prefs (`prefs.get/set/resolve`), set a
  workspace default (`prefs.set_default`, admin), and format/convert canonical values with the
  grant-free `format.datetime/number/quantity` + `convert.unit` tools. Use when a task says "set a
  user's language/timezone/units", "resolve a principal's preferences", "format a timestamp/number/
  quantity for a user", "convert units", or "localize server-side content". Domain data is ALWAYS stored
  canonically (UTC instants, SI/base units, locale-neutral codes); presentation is resolved at the
  boundary — never a formatted string in a record.
---

# Managing preferences & backend formatting (`lb-prefs`)

Lazybones stores **all domain data canonically** — UTC instants, SI/base units, locale-neutral enums —
and **never** a formatted string. A user picks independent, separately-overridable **axes**
(`language`, `timezone`, `date_style`, `time_style`, `first_day_of_week`, `number_format`,
`unit_system`, `unit_overrides`), and presentation is resolved at the boundary from those prefs. The
backend exposes that resolution as MCP tools so a thin client (mobile app, email relay, webhook) never
re-implements timezone math, unit conversion, or date formatting.

The crate is `rust/crates/host/src/prefs/`; the record is `user_prefs:[ws,user]` /
`workspace_prefs:[ws]` (SCHEMAFULL, each axis nullable so "unset → inherit" is structural). Two call
styles, as with `dashboard-mcp` / `channels-inbox-outbox`:

1. **Dedicated REST routes** — the settings page's surface (`/prefs…`, `/format/…`, `/convert/unit`).
2. **The universal MCP bridge** — `POST /mcp/call {tool, args}` for any verb by dotted name.

Gated verbs derive **workspace + principal from the token** (the hard wall). The `format.*`/`convert.*`
verbs are a **grant-free utility tier** — pure CLDR/unit math over NO tenant data, so they carry no
capability; the caller passes the resolved prefs (or the individual axes) inline.

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Capabilities: `prefs.get`/`prefs.resolve` read **own** prefs; `prefs.set` writes **own**;
`prefs.set_default` is **admin-gated** (workspace default). `format.*`/`convert.*` need no grant. Deny
is opaque — without `prefs.get` you cannot even confirm another user's record exists.

## 2. The verbs

| Action | REST route | MCP bridge (`POST /mcp/call`) | Args |
|---|---|---|---|
| Read own prefs | `GET /prefs` | `{"tool":"prefs.get","args":{}}` | — |
| Write own prefs | `PUT /prefs` | `{"tool":"prefs.set","args":{"patch":{…}}}` | `patch` (a `Prefs` object) |
| Resolve the chain | `POST /prefs/resolve` | `{"tool":"prefs.resolve","args":{"override":{…}?}}` | `override?` |
| Set workspace default (admin) | `PUT /prefs/default` | `{"tool":"prefs.set_default","args":{"patch":{…}}}` | `patch` |
| Format datetime | `POST /format/datetime` | `{"tool":"format.datetime","args":{…}}` | `instant` + `prefs` \| `timezone,date_style,time_style` |
| Format number | `POST /format/number` | `{"tool":"format.number","args":{…}}` | `value` + `prefs` \| `number_format`, `max_frac?` |
| Format quantity | `POST /format/quantity` | `{"tool":"format.quantity","args":{…}}` | `value,from_unit,dimension,prefs`, `max_frac?` |
| Convert unit | `POST /convert/unit` | `{"tool":"convert.unit","args":{…}}` | `value,from,to` |
| Catalog (i18n) | `POST /prefs/catalog` | `{"tool":"prefs.catalog","args":{…}}` | see §5 |

`prefs.set`/`set_default` take a **partial** `Prefs` patch — only the axes you name change; the rest
stay unset (inherit). `prefs.resolve` folds the chain (request `override` → user → workspace default →
built-in fallback `en`/UTC/`iso`/`metric`) into a fully-populated `ResolvedPrefs` with every axis
decided.

## 3. The axes (the closed, pinned set)

```jsonc
{
  "language": "es",                 // seed for unset axes; NOT a lock on them
  "timezone": "Europe/Madrid",      // IANA id
  "date_style": "eu",               // eu | iso | usa
  "time_style": "h24",              // h12 | h24
  "first_day_of_week": "monday",
  "number_format": "eu",            // decimal/grouping style
  "unit_system": "metric",          // metric | imperial
  "unit_overrides": { "speed": "knot" }   // dimension → unit; per-quantity override
}
```

Axes are **independent** — "Spanish text WITH USA dates AND metric units except wind in knots" is
expressible. A base `language` only seeds sensible defaults for the axes you leave unset.

- **Dimensions** (`snake_case`): `temperature`, `speed`, `distance`, `mass`, `pressure`, `data`,
  `percent`, `time`.
- **Units** (`snake_case` tokens): `celsius`/`fahrenheit`/`kelvin`, `meter_per_second`/
  `kilometer_per_hour`/`mile_per_hour`/`knot`, `meter`/`kilometer`/`foot`/`mile`,
  `kilogram`/`gram`/`pound`/`ounce`, `pascal`/`hectopascal`/`bar`/`psi`,
  `byte`/`kilobyte`/`megabyte`/`gigabyte`, `ratio`/`percent`, `second`/`minute`/`hour`/`day`.

## 4. Formatting canonical values

The `format.*`/`convert.*` verbs are pure: pass the canonical value + either a full resolved `prefs`
object or the explicit axes. A UTC instant in, a wall-clock string out; a base unit in, the user's
unit out.

```bash
# resolve a principal's prefs, then format a UTC instant + a wind reading through them
RES=$(curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d '{"tool":"prefs.resolve","args":{}}' | jq -c .resolved)

# a stored UTC instant (ms) rendered in the resolved timezone/date+time style
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d "{\"tool\":\"format.datetime\",
  \"args\":{\"instant\":1719800000000,\"prefs\":$RES}}"          # → {"text":"01/07/2026 05:33"}

# a canonical 12.0 m/s wind speed rendered per the user's unit_overrides.speed
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d "{\"tool\":\"format.quantity\",
  \"args\":{\"value\":12.0,\"from_unit\":\"meter_per_second\",\"dimension\":\"speed\",\"prefs\":$RES}}"
# → {"text":"23.3 kn","value":23.3,"unit":"kn"}

# a pure unit conversion (dimensionally checked by `uom` — °C↔°F is affine, not a factor)
curl -s -X POST http://127.0.0.1:8080/mcp/call -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"tool":"convert.unit","args":{"value":100,"from":"celsius","to":"fahrenheit"}}'   # → {"value":212.0,…}
```

`format.datetime` accepts EITHER `prefs` (a resolved object) OR explicit `timezone` + `date_style` +
`time_style`. `format.number` similarly accepts `prefs` or an explicit `number_format` (+ `max_frac`).

## 5. i18n catalogs (`prefs.catalog`)

Server-generated content (emails, notifications, inbox render strings) is localized **server-side** in
the *recipient's* resolved language via **ICU MessageFormat 1 (MF1)** catalogs. `prefs.catalog`:

- `render` — `{key, args?, recipient?}` → `{text, locale_used}` (renders a catalog message; falls back
  built-in → key, never panics).
- `set_catalog` — `{locale, messages}` (a flat `key → MF1` map) → a per-workspace **override** that
  shadows the built-in for that workspace only.

## Gotchas

- **Canonical storage, always** — a formatted string in a domain record is a bug. Store UTC instants,
  SI/base units, locale-neutral codes; format only at the boundary.
- **Axes are independent** — a base `language` seeds only *unset* axes; it never overrides a set one.
- **`prefs.set` is a partial patch** — naming one axis leaves the rest inherited (nullable = inherit).
- **`set_default` is admin-only**; `get`/`set` act on the caller's OWN record — you can't read or write
  another user's prefs.
- **`format.*`/`convert.*` carry no capability** but need a *truthful* `from_unit`/`from` — an unknown
  unit is a hard error (never a passthrough); a cross-dimension convert (temperature→speed) is rejected.
- **Same user, per-workspace prefs** — the same global identity has a *different* `user_prefs` record in
  ws-A and ws-B; resolution in ws-B never reads ws-A's record.
- **Request override is per-call and self-scoped** — `prefs.resolve {override}` renders *this* response
  differently (a share link, "preview in es"); it never writes the stored record.

## Related

- Scope + shipped doc: `docs/scope/prefs/user-prefs-scope.md`, `doc-site/content/public/prefs/prefs.mdx`,
  `docs/sessions/prefs/lb-prefs-session.md`; i18n: `docs/scope/prefs/i18n-catalogs-scope.md`.
- Source-unit provenance (the `unit:`/`dimension:` tag `format.quantity` reads): `docs/scope/tags/`.
- Capability/workspace/canonical rules: `README.md` §3, §6.5, §6.8, §7.
- Gateway routes: `rust/role/gateway/src/server.rs`; the crate: `rust/crates/host/src/prefs/`.
