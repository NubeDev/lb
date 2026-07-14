# Session — hard-remove weather from core (moved to an out-of-tree extension)

_2026-07-14_

## The ask

> "Can we move the weather into/as an extension — hard remove, no migration is needed. It needs to be
> made as an extension widget."

Weather had been built **directly into core** (a `weather.current` host-native MCP verb + a built-in
`weather` dashboard viz + a geo-search location control). Per the ask, this session **hard-removed** every
trace from `lb` (no migration, no deprecation) and rebuilt weather as a **standalone native (Tier-2)
extension** in `NubeIO/rubix-ai-extensions` (`extensions/weather/`) — the full build is documented in that
repo's [weather-session](https://github.com/NubeIO/rubix-ai-extensions/blob/master/docs/sessions/extensions/weather-session.md).

## Removed from core

**Host crate** — deleted `rust/crates/host/src/weather/` (the `current`/`tool`/`mod` files + the
`weather_tool_test`); removed `mod weather` + the `weather::*` re-exports from `lib.rs`, the `"weather."`
entry in `HOST_NATIVE_PREFIXES` + its dispatch arm in `tool_call.rs`, `mcp:weather.current:call` from
`VIEWER_CAPS` in `authz/builtin_roles.rs`, the `weather.current` entry in `system/catalog.rs`, and the
`weather` view from `dashboard/widget_catalog.json`.

**UI** — deleted `features/dashboard/views/weather/` (`WeatherPanel`, `wmoCode`, `observedLocal`, and the
`weather.gateway.test.tsx`), `panel-builder/options/defs/weather.ts`, and the **entire geo-search control
chain** weather was the *sole* consumer of: `panel-builder/options/controls/{GeoSearch.tsx, geocode.ts,
geoWrite.test.ts}`, the `geo-search` control kind (`types.ts`), `writeGeoPlace` + the `GeoPlace` usage
(`binding.ts`), and the `geo-search` branches in `Control.tsx` / `OptionSectionCard.tsx`. Dropped
`weather` from: the `View` union (`dashboard.types.ts`), `NO_FIELDCONFIG_VIEWS` (`registry.ts`),
`WIZARD_VIEWS` + the `weather` liveness block (`optionLiveness.ts`), `SOURCELESS_VIEWS`
(`usePanelEditor.ts`), the display read-views (`useDisplayOverride.ts`), the VizGallery/VizPicker cards
(+ the now-unused `CloudSun` import), and `usePanelData`'s `weatherSource` self-source path. Removed the
Open-Meteo weather stub + `LB_WEATHER_OPEN_METEO_BASE` env from `test/real-gateway.ts`. Updated
`VizGallery.test.tsx` (11→10 cards) and `registryRoundTrip.test.ts` (dropped the weather cases).

**Docs** — removed `docs/scope/weather/`, `docs/sessions/weather/` (the prior feed session),
`docs/debugging/weather/`, and `doc-site/content/public/weather/`; dropped the `weather/` bullet from
`docs/scope/README.md`; replaced the shipped-weather row in `docs/STATUS.md` with a "removed / moved to
rubix-ai-extensions" record; neutralized the now-dangling link in `docs/debugging/README.md` (entry kept
for its transient-network-blip / native-roots lesson).

## What stayed (correctly)

- `system_map_test.rs` and `TopMenuNav.test.tsx` use `"weather"` as a **generic remote/ext-slot fixture
  id** exercising the opaque-id path — not the built-in verb. Valid, and now the id happens to name the
  real extension.
- `secrets-scope.md` and the extension-authoring skill docs use `weather`/`weather-panel` as **illustrative
  example names** in unrelated tutorials — not the feature.

## Green

- `cargo build -p lb-host`: **zero** weather references remain; the crate is weather-clean.
- lb UI: `tsc --noEmit` clean (weather-free); the affected vitest suites pass — VizGallery (6),
  registryRoundTrip (15), TopMenuNav (8), and the dashboard + panel-builder suites (215 total).
- **Caveat:** a full `cargo build --workspace` is currently **blocked by a pre-existing, unrelated
  in-flight change** — a series rename/delete feature (untracked `rename.rs`/`delete.rs` in both
  `crates/ingest/` and `crates/host/src/ingest/`, plus modified `lib.rs`/`mod.rs`/`tool.rs`) with 3
  borrow-checker errors in `lb-ingest`. This session **did not touch** any ingest file; every build error
  is in those WIP files, none in a file this change edited. The weather removal itself is compile-clean.

## Rule 10

Core now has **no idea weather exists** — it is reached only through the generic native-sidecar dispatch,
the capability grammar, and the UI-federation widget seam, each treating the extension id as opaque data.
No core crate or UI shell branches on the id. A fresh proof of rule 10 by *removing* a core feature.
