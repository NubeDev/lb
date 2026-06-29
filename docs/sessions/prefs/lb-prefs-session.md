# prefs — the `lb-prefs` units + formatting core (session)

- Date: 2026-06-29
- Scope: ../../scope/prefs/user-prefs-scope.md
- Stage: a core crate (`lb-prefs`); the unit add-on rides S8, the record/resolution useful from S2 onward.
- Status: done (units + formatting core; i18n MessageFormat catalogs deferred as named follow-ups)

## Goal

Build the canonical-in / localized-out preferences + units + formatting library the dashboard
fieldConfig and every thin client depend on: the nullable per-(ws,user) + per-(ws) preference
records, a pure resolution fold over the chain, uom-backed unit conversion, locale/tz-aware
formatting, the 8 MCP verbs (4 gated + 4 grant-free), the gateway routes mirroring them 1:1, and the
generated closed-enum TS constants module. Exit gate: the mandatory deny + workspace-isolation tests
green against a real store + MCP, plus conversion-correctness (affine °C↔°F), axis-independence,
resolution-chain, locale-rendering (incl. DST), and the canonical guarantee — all on real infra, no
mocks.

## What changed

New crate `rust/crates/prefs/` (`lb-prefs`), one responsibility per file:
- `axis/` — the **closed axis set**, one file per axis: `dimension.rs` (8 dimensions), `unit.rs`
  (29 units, each tied to one dimension), `unit_system.rs` (+ the dimension→default-unit table),
  `date_style.rs`, `time_style.rs`, `number_format.rs`, `first_day.rs`, `language.rs` (the enabled
  en/es slice + fallback). `pub mod axis` so the generator and tests read the enums.
- `prefs.rs` — `Prefs` (all axes `Option`, the nullable record shape) + `ResolvedPrefs` (fully
  populated) + `display_unit()` (override → unit_system default).
- `resolve.rs` — the pure fold `resolve(&[Prefs]) -> ResolvedPrefs`, each axis decided
  independently, `unit_overrides` merged per-dimension, built-in fallback `(en, UTC, iso, h24, metric)`.
- `convert/` — `unit_convert.rs` (uom-backed same-dimension convert; cross-dimension rejected) +
  `quantity.rs` (canonical value → resolved display unit).
- `format/` — `number.rs` (locale separators from the `NumberFormat` axis), `datetime.rs` (tz over a
  UTC instant via `chrono-tz`, date/time styles from the axes), `quantity.rs` (convert + render, the
  chart bridge).
- `store/` — SCHEMAFULL `user_prefs:[ws,user]` + `workspace_prefs:[ws]`, composite-id MERGE upsert
  get/set/default/resolve_chain, namespace-scoped.
- `bin/gen_ts.rs` — `gen-prefs-ts`, emits `ui/src/lib/prefs/dimensions.generated.ts`.

Host wiring `rust/crates/host/src/prefs/`: `authorize.rs` (the gates), `verbs.rs` (gated
`prefs_get/set/resolve/set_default`, forcing `user = caller.sub()` for OWN verbs), `tool.rs`
(`call_prefs_tool` for `prefs.*`, `call_format_tool` for the grant-free `format.*`/`convert.*`).
`tool_call.rs`: `format.*`/`convert.*` dispatch **before** the `is_host_native` authorize gate (the
grant-free tier), `prefs.*` inside it (gated).

Gateway `role/gateway/src/routes/prefs.rs` + `server.rs`: `GET/PUT /prefs`, `POST /prefs/resolve`,
`PUT /prefs/default`, `POST /format/{datetime,number,quantity}`, `POST /convert/unit`.

Client `ui/src/lib/prefs/dimensions.generated.ts` — the generated closed vocabulary.

## Decisions & alternatives

- **uom for conversion, but `chrono-tz` + axis-driven rendering for date/number/tz — NOT `icu4x` (v1).**
  The scope named `icu4x` for CLDR date/number/tz rendering. I kept **uom** exactly as scoped (it is
  the correctness-critical part: the affine °C↔°F transform is the named bug class, and uom's typed
  `ThermodynamicTemperature` encodes offset+scale — verified `0°C=32°F`, `100°C=212°F`, `-40` crossover).
  For *rendering* I deliberately did **not** pull icu4x in this slice:
  - The scope's concrete rendering requirements are fully captured by my **closed axes**:
    `NumberFormat` encodes the exact decimal/grouping choice (`43,2` vs `43.2`), `DateStyle`/`TimeStyle`
    the exact field order + 12/24h. Rendering from the axis enum is deterministic, locale-correct for
    the enabled set, and has **zero CLDR-data-size cost** — which directly satisfies the scope's
    Pi-profile "compile in only enabled locales" risk without any icu data-slicing config yet.
  - The load-bearing hard part — **tz/DST correctness over a UTC instant** — is delegated to
    `chrono-tz` (already a workspace dep), whose embedded IANA tz database carries the DST + historical
    rules. The DST-boundary test (US spring-forward 2026, EDT vs EST) passes.
  - **Why not icu4x now:** icu 2.x's date/number formatter API is large and evolving, and pulling it
    for *numeric* styles I can derive exactly from a closed enum is cost without benefit at v1. The
    place icu4x genuinely earns its keep is **localized month/day NAMES and ICU MessageFormat
    plurals** — both of which are Phase 2 (catalogs), explicitly deferred. So icu4x is recorded as the
    **Phase-2 swap-in behind the same `format::*` signatures** (the axis is the contract; the renderer
    is replaceable). Rejected: pulling icu4x now and rendering numbers through it — more data, more API
    surface, no behavioral gain over the axes for en/es.
- **`format.*`/`convert.*` are a grant-free utility tier**, dispatched in `tool_call.rs` *before* the
  host-native authorize gate (routing them through `authorize_tool` would wrongly require an
  `mcp:format.*:call` grant the scope says must not exist). They read no store; the caller passes
  resolved prefs/axes inline. Gateway still authenticates them for identity (a token is required) but
  needs no capability — proven by the "no prefs cap" route test.
- **OWN-scoping is structural, beyond the cap.** `prefs.get`/`prefs.set`/`prefs.resolve` force the
  target `user` to `principal.sub()` — there is no parameter to name another user — so even a holder
  of `prefs.get` can only read their own record. `prefs.set_default` is the only admin verb, gated by
  its own cap `mcp:prefs.set_default:call` (admin = whoever was granted it, matching how
  `teams.manage` gates the existing admin CRUD — the codebase gates on the cap string, not `role`).
- **Composite-id MERGE upsert for LWW + offline idempotency.** `user_prefs:[ws,user]` /
  `workspace_prefs:[ws]` deterministic ids mean a replayed offline edit upserts in place (one record,
  last value) and a patch merges (untouched axes preserved). `unit_overrides` is `option<object>`;
  the store returns an unset column as JSON `null`, so `Prefs` deserializes null→empty map (custom
  `null_as_empty_map`). Reads project the axis columns explicitly (NOT `SELECT *`) to avoid decoding
  the composite-id `RecordId` array into `serde_json::Value` (see Dead ends).
- **Generated TS constants** so the settings UI / fieldConfig unit picker can't disagree with the
  server: `bin/gen_ts` writes the closed dimension/unit/style vocabulary; a test asserts the
  checked-in file matches (drift fails CI). No prior codegen pattern existed in the repo.

## MessageFormat dialect resolution (the scope's Risk, resolved now even though catalogs are deferred)

Per the scope, the i18n MessageFormat **catalogs** (server-localized notification/email/inbox content)
are deferred to Phase 2 — but the **dialect drift risk** is resolved now so Phase 2 isn't blocked:

- **Pinned dialect: ICU MessageFormat 1 (MF1).** The client parser is **`intl-messageformat`** (the
  de-facto TS MF1 implementation — plurals, select, gendered select, embedded number/date skeletons).
- **Host side:** the host catalog source is authored in the **same MF1 syntax**; the host renders with
  an MF1-compatible Rust parser (`icu_messageformat`-style MF1 subset, or a small vetted MF1
  interpolator) — NOT icu4x's experimental/MF2-leaning path, which would silently diverge on
  plural/select. The rule recorded: **host and client must parse the identical MF1 dialect**, and any
  key that also appears in server-generated content treats the **host catalog as source of truth**
  (same as the catalog-version-drift mitigation). This is a Phase-2 build item; the dialect choice is
  now pinned so the catalog work starts unblocked.

## Tests

Mandatory categories + key cases, all on real infra (`mem://` store, real MCP via `lb_host::call_tool`,
real gateway router via `oneshot`), seeded through the real write path — no mocks, no `*.fake.ts`:

- **Capability deny** (`crates/host/tests/prefs_deny_test.rs`, 5): `set_default` from a non-admin
  denied (and nothing written); `get` reads only the caller's OWN record (no param to name another);
  `set` cannot write another user's record; no-cap is opaque `Denied`; cross-workspace resolve denied.
- **Workspace isolation — specified** (`crates/prefs/tests/isolation_test.rs`, 2): the SAME global user
  with different `user_prefs` in ws-A and ws-B → resolve in ws-B never reads ws-A; a ws-A default
  change does not move ws-B. Plus the gateway two-session variant (`prefs_routes_test.rs`).
- **Offline/sync** (`store_test.rs::offline_edit_replays_idempotently`): same edit twice → one record,
  LWW.
- **Conversion correctness** (`convert_test.rs`, 7): affine °C↔°F (0/100/-40, K), m/s↔km/h↔knots,
  m↔ft/mi, percent↔ratio, round-trip stability, cross-dimension rejected, every-unit round-trip.
- **Axis independence + resolution chain** (`resolve_test.rs`, 6): es + usa + metric + wind=knots
  independent; each chain level wins in order; base-locale seed fills only unset; overrides merge.
- **Locale rendering** (`format_test.rs`, 8): `43,2` vs `43.2`; eu/iso/usa; 12h/24h; tz over a UTC
  instant; **DST boundary** (US spring-forward, EDT vs EST); the es-metric vs en-knots quantity bridge.
- **Canonical guarantee** (`store_test.rs`): store→read returns the canonical value unchanged; the raw
  row carries only neutral enums/ids — no formatted string persisted.
- **MCP + gateway end-to-end** (`prefs_mcp_test.rs` 3, `prefs_routes_test.rs` 5): grant-free
  format/convert, gated set→resolve round-trip, set_default denied without admin cap.
- **Generated TS** (`generated_ts_test.rs` 2): checked-in file matches the generator; enum ALL counts.

### Green output

```
lb-prefs:        convert 7, format 8, resolve 6, store 5, isolation 2, generated_ts 2  = 30 ok
lb-host:         prefs_deny 5, prefs_mcp 3  (+ all 60 existing host suites still green)
lb-role-gateway: prefs_routes 5            (+ all existing gateway suites still green)
cargo build --workspace: Finished (node + all roles)
cargo fmt: clean
```

(Full `cargo test -p lb-prefs -p lb-host -p lb-role-gateway` run pasted in the task log — every suite
`test result: ok`, 0 failed.)

## Debugging

No `debugging/` entry opened — the two snags hit (RecordId→Value decode, null `option<object>`) were
caught and fixed within the same write/test loop before any commit, with regression coverage baked into
`store_test.rs` (explicit-column projection + the canonical-guarantee read). See Dead ends.

## Public / scope updates

- Promoted to `docs/public/prefs/prefs.md` (the shipped surface + the icu4x/chrono-tz decision).
- Scope status line updated to "shipped (units + formatting core); i18n catalogs deferred".
- `docs/key-stack.md` already carries the `lb-prefs`/icu4x/uom row (unchanged; the icu4x-vs-chrono-tz
  nuance is recorded here + in public, not by rewriting the stack map).

## Dead ends / surprises

- `SELECT *` on a composite-id record returns the `id` as a SurrealDB `RecordId` whose array id-part
  fails `serde_json::Value` decode (`invalid type: enum`). Fix: project the axis columns explicitly
  (`PREFS_COLUMNS`) on every read — never select `id`.
- An unset `option<object>` column comes back as JSON `null`; deserializing null into a
  `BTreeMap` errors (`expected a map`). Fix: `null_as_empty_map` on `unit_overrides` (the only
  non-`Option` field).

## Follow-ups

- **Phase 2 — i18n MessageFormat catalogs** (server-localized notification/email/inbox content,
  per-recipient fan-out, the workspace override catalog asset). Dialect pinned above (MF1 /
  `intl-messageformat`); host parser still to build.
- **icu4x swap-in** behind `format::*` for localized month/day NAMES + the actual MessageFormat
  plural/select engine, plus the en/es CLDR data-slice build config for the Pi profile.
- **Localized unit display names** (currently locale-stable abbreviations) via icu4x units.
- **Settings UI + bootstrap-locale + re-render-on-change** — the client half (frontend scope), consumes
  the generated constants + the `prefs.*`/`format.*` verbs shipped here.
- STATUS.md updated: prefs core slice = shipped.
```
