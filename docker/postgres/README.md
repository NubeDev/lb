# docker/postgres — TimescaleDB

Local TimescaleDB instance for time-series data. Runs on port **5433** (not the
default 5432) to avoid conflicts with any local PostgreSQL installation.

## Quick start

```bash
cp .env.example .env          # edit credentials if needed
docker compose up -d
```

Connection string:

```
postgresql://lb:lb_secret@localhost:5433/lb
```

## Seed data

`seed.sh` populates one year of fake energy and water meter readings at
15-minute intervals (~490 000 rows total).

### Data model

```
site
└── meter  (site_id FK)
    └── point  (meter_id FK)          ← name + unit label
        └── point_reading  (point_id FK, time, value)   ← hypertable
```

### Fixture layout

| Site | Meters | Points |
|------|--------|--------|
| Northside Factory | Main Energy, HVAC Energy, Water | kWh, kW demand, Flow L/min, Total m³ |
| Southbank Office | Main Energy, Water | kWh, kW demand, Flow L/min, Total m³ |
| Eastfield Warehouse | Main Energy, Irrigation Water | kWh, kW demand, Flow L/min, Total m³ |

### Running the seed

```bash
# Start the DB first
docker compose up -d

# Wait for healthy, then seed
./seed.sh

# Override connection details
./seed.sh --host localhost --port 5433 --user lb --password lb_secret --db lb
```

The script is **idempotent** — re-running it replaces the existing year of data.

## Files

| File | Purpose |
|------|---------|
| `docker-compose.yml` | Service definition — TimescaleDB on port 5433 |
| `.env.example` | Template for credentials; copy to `.env` |
| `init/01-timescaledb.sql` | Enables the `timescaledb` extension on first start |
| `init/02-schema.sql` | Creates `site`, `meter`, `point`, `point_reading` tables |
| `seed.sh` | Inserts sites, meters, points, and one year of readings |

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

# Wipe data volume
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
