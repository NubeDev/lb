// Keyless geocoding for the weather tile's Location search (weather scope). Same provider as the weather
// feed — Open-Meteo's geocoding API (`geocoding-api.open-meteo.com`) — so no API key, no secret, matching
// the "no key" ethos of the feature. Turns a city/place query into ranked matches carrying lat/lon + a
// human label; the `GeoSearch` control fills `options.lat/lon/label` from the picked one.
//
// This is a plain browser fetch to a public HTTP API (the app already talks HTTP to the gateway); it is
// NOT a node MCP verb — geocoding is a UI convenience, not a gated capability. One responsibility: query
// → `GeoPlace[]`.

/** One geocoding match: the coordinates the weather fetch needs + a display label. */
export interface GeoPlace {
  /** Display label, e.g. "Brisbane, Queensland, AU". */
  label: string;
  lat: number;
  lon: number;
}

const BASE = "https://geocoding-api.open-meteo.com/v1/search";

interface OpenMeteoResult {
  name: string;
  latitude: number;
  longitude: number;
  admin1?: string;
  country_code?: string;
  country?: string;
}

/** Compose a readable label from a result: "City, Region, CC" (regions/CC omitted when absent). */
function labelOf(r: OpenMeteoResult): string {
  return [r.name, r.admin1, r.country_code ?? r.country].filter(Boolean).join(", ");
}

/** Search places matching `query`. Returns [] for a blank/too-short query or any network/parse error —
 *  an honest empty result set, never a thrown error into the async-select (which would surface as a
 *  broken control). `signal` lets the caller abort a superseded keystroke's request. */
export async function searchPlaces(query: string, signal?: AbortSignal): Promise<GeoPlace[]> {
  const q = query.trim();
  if (q.length < 2) return [];
  try {
    const url = `${BASE}?name=${encodeURIComponent(q)}&count=8&language=en&format=json`;
    const resp = await fetch(url, { signal });
    if (!resp.ok) return [];
    const body = (await resp.json()) as { results?: OpenMeteoResult[] };
    return (body.results ?? []).map((r) => ({
      label: labelOf(r),
      lat: r.latitude,
      lon: r.longitude,
    }));
  } catch {
    return [];
  }
}
