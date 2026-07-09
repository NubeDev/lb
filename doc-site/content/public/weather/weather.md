# Weather

> **TODO** — placeholder. Fills on ship from
> [`docs/scope/weather/weather-feed-scope.md`](../../../../docs/scope/weather/weather-feed-scope.md).

A compile-optional (`weather` cargo feature, off by default) native extension that pulls current
weather from a **free, keyless** feed (Open-Meteo), on a 30-minute durable job with a **Run now**
button, optionally persisting readings into the series plane, shown on a shadcn dashboard widget.

**Install (dev):** `make dev WEATHER=1` — compiles the feature, builds + supervises the sidecar,
and pre-approves the Open-Meteo connect. No API key. A plain `make dev` builds no weather code.
