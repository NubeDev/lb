# Building Automation — domain context

This workspace runs a **building automation system (BAS)**. The domain vocabulary is a
three-level tree:

- **site** — a building (e.g. "Northside Factory"). Carries an `area` tag (floor area in m²),
  used to normalize energy into intensity (kWh/m²).
- **equip** — a piece of equipment on a site: an **AHU** (air handler), **chiller**, or utility
  **meter**. Tagged by kind on `meter_tag` (`ahu`, `chiller`, `meter`, …).
- **point** — a sensor/actuator on an equip. Point ids follow a suffix convention the FDD rules
  bind by: `-zt` = zone temp, `-cmp` = compressor state, `-fan` = fan state, `-flow` = water flow.
  A point named `Energy kWh` is the building's electric meter.

## The data

Time-series live in the `demo-buildings` datasource (a sqlite federation source), table
`point_reading (time, point_id, value)`, joined `point → meter → site`. Query it with
`federation.query` / the datasource's SQL surface. The dialect is the **DataFusion ∩ SQLite**
intersection — avoid `datetime()`, CTEs, and `strftime()`.

## Insights (FDD)

Faults are raised as insights under the dedup-key grammar `fdd:{issue}:{equip}` — e.g.
`fdd:sensor-flatline:meter-020`, `fdd:cooling-failure:meter-014`. Severities are `warning` and
`critical`. The issues this pack detects:

- **sensor-flatline** — a zone-temp sensor whose last-day spread is ~0 (frozen/unwired).
- **cooling-failure** — a compressor running but not cooling.
- **short-cycling** — a compressor toggling too often.
- **after-hours** — a fan running outside its schedule.
- **night-water** — water flow overnight (a leak signal).
- **energy-drift** — a meter's energy trending up week-over-week.
- **energy-intensity** — a building over its kWh/m² budget (`energy-intensity-high:{building}`).

When a user asks "what's wrong with plant X", read the open insights for that site's equips,
then pull the underlying trend from `point_reading` to explain the raise.
