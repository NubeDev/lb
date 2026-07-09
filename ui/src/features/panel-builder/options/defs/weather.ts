// The `weather` view options (weather scope). A weather cell is SELF-SOURCED from the keyless
// `weather.current` verb — it takes no user-picked datasource; instead these options carry the LOCATION
// the fetch uses. `usePanelData.weatherSource()` reads `options.lat`/`options.lon` (with a Brisbane
// default) to build the call, and `WeatherPanel` shows `options.label` (falling back to the returned
// "lat,lon") as the header. All three live under `options.*` (no fieldConfig — weather is in
// NO_FIELDCONFIG_VIEWS). One responsibility: the weather option catalog.
//
// Open-Meteo's `current` endpoint is COORDINATE-based (no geocoding), so location is entered as
// latitude/longitude here. A city/country name → coordinates picker is the honest follow-up (it needs a
// geocoding verb); until then the free-text `label` names the place a human reads on the tile.

import type { OptionDef } from "../types";

const WEATHER = ["weather" as const];
const GROUP = "Location";

export const WEATHER_OPTIONS: OptionDef[] = [
  {
    id: "geo",
    label: "Search location",
    group: GROUP,
    // Reads the current label to show as the selection; picking a place writes label + lat/lon together
    // (writeGeoPlace), so its storage `path` is the label it displays.
    scope: "options",
    path: "label",
    views: WEATHER,
    control: { kind: "geo-search" },
    default: "",
    keywords: ["search", "city", "country", "place", "geocode", "location", "town"],
  },
  {
    id: "label",
    label: "Location label",
    group: GROUP,
    scope: "options",
    path: "label",
    views: WEATHER,
    control: { kind: "text", placeholder: "e.g. Brisbane, AU" },
    default: "",
    keywords: ["name", "city", "place", "title", "country"],
  },
  {
    id: "lat",
    label: "Latitude",
    group: GROUP,
    scope: "options",
    path: "lat",
    views: WEATHER,
    control: { kind: "number", min: -90, max: 90, step: 0.01, placeholder: "-27.47" },
    default: -27.47,
    keywords: ["latitude", "lat", "coordinate", "location", "city", "country"],
  },
  {
    id: "lon",
    label: "Longitude",
    group: GROUP,
    scope: "options",
    path: "lon",
    views: WEATHER,
    control: { kind: "number", min: -180, max: 180, step: 0.01, placeholder: "153.02" },
    default: 153.02,
    keywords: ["longitude", "lon", "lng", "coordinate", "location", "city", "country"],
  },
];
