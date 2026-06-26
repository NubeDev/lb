# User preferences — canonical data in, localized presentation out

Status: **TODO** (stub). Promoted from `scope/prefs/user-prefs-scope.md` when the slice ships.

Per-(workspace, user) preferences resolved through a chain (request → user → workspace default →
built-in): **language** (English/Spanish to start), **timezone**, **date/time display style** (EU
`DD/MM/YYYY` · ISO `YYYY-MM-DD` · USA `MM/DD/YYYY`, 12h/24h), **number format**, and a **unit system**
(metric/imperial with per-quantity overrides). The platform stores everything **canonically** — UTC
instants, SI/base units, locale-neutral codes — and **never** persists a formatted string. The host
exposes presentation as MCP tools (`prefs.get/set/resolve`, `format.datetime/number/quantity`,
`convert.unit`) so thin clients (mobile, email, webhooks) never re-implement timezone, unit, or date
logic. Built on `icu4x` (CLDR formatting) + `uom` (unit conversion); unit *source* comes from the
`unit:` tag (`scope/tags/`). Server-generated content (emails, notifications, inbox renders) is
localized server-side in the recipient's language via ICU MessageFormat catalogs.

Filled in on ship with: the `user_prefs`/`workspace_prefs` record model + resolution chain, the
`prefs.*`/`format.*`/`convert.*` MCP verbs, the catalog layering, and the green deny + two-workspace
isolation + offline-replay + conversion-correctness + locale-rendering tests.

See `scope/prefs/user-prefs-scope.md` for the ask.
