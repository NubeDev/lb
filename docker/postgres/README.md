# docker/postgres — TimescaleDB

Local TimescaleDB instance for time-series data. Runs on port **5433** (not the
default 5432) to avoid conflicts with any local PostgreSQL installation.

## Quick start

```bash
cp .env.example .env          # edit credentials if needed
docker compose up -d
./seed.sh                     # or: python3 seed.py
```

Connection string:

```
postgresql://lb:lb_secret@localhost:5433/lb
```

## Seed data

`seed.py` (wrapped by `seed.sh`) generates one year of randomized building
data at **5-minute intervals** — energy, water, gas, and HVAC telemetry across
**8 sites**, with proper per-meter randomness and Haystack tags on every
entity. ~**35 million readings** in total; ingest takes about 6 minutes on a
typical container.

### What the data looks like

* **8 sites** of different kinds: factory, office, warehouse, hospital, mall,
  apartments, airport terminal, data center. Each is in a UK city with a geo
  coord, timezone, area, and climate marker.
* **69 meters / equipment** rows — utility meters (energy/water/gas) AND HVAC
  equipment (AHU, RTU, chiller, boiler, FCU, CRAH). The Haystack `meter_tag`
  row says which is which (no schema distinction: this is how Haystack models
  entities).
* **332 points**: kWh, kW, L/min, m³, plus zone/supply/return temp, AC status,
  compressor status, fan status, mode (off/cooling/heating), valve/damper %.
* **Per-meter randomness.** Each meter id seeds its own RNG via md5, so re-
  runs reproduce the same dataset but two different meters produce genuinely
  different base load, phase, and noise.
* **Physically-consistent HVAC.** Each HVAC equipment is driven by one state
  machine across the year: outdoor air temperature drives the mode (heating
  below 14 °C, cooling above 19 °C), the schedule gates operation (office
  hours on weekdays, plus a per-equipment probability of weekend openings),
  zone temp drifts toward setpoint when running and toward outdoor when idle,
  compressor status fires only on a real cooling call, supply-air temp drops
  to ~14 °C in cooling and rises to ~32 °C in heating.

### Data model

```
site  ── site_tag   (Haystack tags)
 └── meter ── meter_tag
     └── point ── point_tag
         └── point_reading  (hypertable, time + point_id + value)
```

`site` / `meter` / `point` are the entity tables. The `*_tag` tables are the
Haystack tag model: each row is one `(entity_id, tag, kind, val)` triple,
where `kind` ∈ `marker|ref|str|number|coord` and `val` is NULL for markers.
This is the [Project Haystack](https://project-haystack.org/) tag model —
entities have tags, not columns, and tags are walked to navigate.

Sample tag rows for a point:

| point_id      | tag       | kind   | val             |
|---------------|-----------|--------|-----------------|
| meter-050-zt  | point     | marker | *(NULL)*        |
| meter-050-zt  | dis       | str    | Zone Temp       |
| meter-050-zt  | siteRef   | ref    | @site-002       |
| meter-050-zt  | equipRef  | ref    | @meter-050      |
| meter-050-zt  | kind      | str    | Number          |
| meter-050-zt  | unit      | str    | degC            |
| meter-050-zt  | his       | marker | *(NULL)*        |
| meter-050-zt  | sensor    | marker | *(NULL)*        |
| meter-050-zt  | zone      | marker | *(NULL)*        |
| meter-050-zt  | temp      | marker | *(NULL)*        |

Tag-based navigation (the Haystack way):

```sql
-- Every discharge-air temp sensor on an RTU at a temperate-climate site.
SELECT pt.id, pt.name
FROM point pt
JOIN meter m ON m.id = pt.meter_id
JOIN site  s ON s.id = m.site_id
WHERE EXISTS (SELECT 1 FROM point_tag t WHERE t.point_id = pt.id AND t.tag='discharge')
  AND EXISTS (SELECT 1 FROM point_tag t WHERE t.point_id = pt.id AND t.tag='temp')
  AND EXISTS (SELECT 1 FROM meter_tag t WHERE t.meter_id = m.id AND t.tag='rtu')
  AND EXISTS (SELECT 1 FROM site_tag  t WHERE t.site_id  = s.id AND t.tag='temperate');
```

### Fixture layout

| Site | Kind | Meters / equipment |
|------|------|--------------------|
| Northside Factory | industrial 24/7 | Main + HVAC energy, water, gas, 4×AHU, chiller, boiler |
| Southbank Office | office 7-19 | Main energy, water, 2×AHU, 2×RTU |
| Eastfield Warehouse | logistics | Main energy, irrigation water, gas, 2×RTU, unit heater |
| Westend Hospital | healthcare 24/7 | Main + HVAC energy, water, gas, 3×AHU, 2×chiller, boiler |
| Central Mall | retail 10-21 + weekends | Main + HVAC energy, water, 4×AHU, 2×RTU, 2×chiller |
| Lakeside Apartments | residential (cold) | Main energy, water, gas, 3×FCU |
| Airport Terminal T2 | transport 24/7 | Main + HVAC energy, water, 6×AHU, 2×chiller |
| Riverside Data Center | DC 24/7 | Main + HVAC energy, water, 4×CRAH, 2×chiller |

### Running the seed

```bash
# Start the DB first
docker compose up -d
./seed.sh                      # full year @ 5-min, ~6 minutes

# Pin the window for byte-identical re-runs
./seed.py --start 2025-01-01T00:00:00Z --end 2025-02-01T00:00:00Z

# Fast smoke test
./seed.py --months 1 --interval 15      # ~1M rows, ~10s

# Skip the readings (just refresh sites/meters/points/tags)
./seed.py --no-readings

# Connection overrides
./seed.sh --host localhost --port 5433 --user lb --password lb_secret --db lb
```

The script is **idempotent**: re-running truncates every seeded table first.
When the window is unpinned (default), readings end at "now"; when pinned with
`--start`/`--end`, the same window produces byte-identical data on every run.

## Files

| File | Purpose |
|------|---------|
| `docker-compose.yml` | Service definition — TimescaleDB on port 5433 |
| `.env.example` | Template for credentials; copy to `.env` |
| `init/01-timescaledb.sql` | Enables the `timescaledb` extension on first start |
| `init/02-schema.sql` | Creates entity, reading, and `*_tag` tables |
| `seed.sh` | Thin wrapper around `seed.py` (kept for backward compat) |
| `seed.py` | Entrypoint: arg parsing, psql/docker-exec detection, `\copy` ingest |
| `inventory.py` | Declarative catalog: which sites, meters, points, and their params |
| `generators.py` | Time-series generators (scalar energy/water/gas + HVAC state machine) |
| `tags.py` | Haystack tag emission (`marker` / `ref` / `str` / `number` / `coord`) |

## Configuration

Override defaults via `.env` or environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `POSTGRES_USER` | `lb` | Database user |
| `POSTGRES_PASSWORD` | `lb_secret` | Database password |
| `POSTGRES_DB` | `lb` | Database name |

## Common commands

```bash
# Start
docker compose up -d

# Stop (data persisted in volume)
docker compose down

# Wipe data volume (forces init scripts to re-run on next up)
docker compose down -v

# Tail logs
docker compose logs -f timescaledb

# psql shell
docker exec -it lb-timescaledb psql -U lb -d lb
```

## Notes

- Data is persisted in a named Docker volume (`timescaledb_data`).
- The `timescaledb` extension is enabled automatically on first start via the
  init script.
- Port 5433 is fixed in `docker-compose.yml`; change the left-hand side of the
  port mapping if you need a different host port.
- If a container has been running for a while (i.e. was started before a schema
  change), `seed.py` will `CREATE TABLE IF NOT EXISTS` the tag tables itself —
  but you should still `docker compose down -v && docker compose up -d` after
  schema changes for a clean baseline.
