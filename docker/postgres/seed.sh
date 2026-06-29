#!/usr/bin/env bash
# Seed one year of fake energy and water meter readings into TimescaleDB.
# Usage: ./seed.sh [--host HOST] [--port PORT] [--user USER] [--db DB] [--password PW]
#
# Defaults match docker-compose.yml / .env.example.
set -euo pipefail

HOST="${POSTGRES_HOST:-localhost}"
PORT="${POSTGRES_PORT:-5433}"
USER="${POSTGRES_USER:-lb}"
DB="${POSTGRES_DB:-lb}"
PASSWORD="${POSTGRES_PASSWORD:-lb_secret}"

while [[ $# -gt 0 ]]; do
  case $1 in
    --host)     HOST="$2";     shift 2 ;;
    --port)     PORT="$2";     shift 2 ;;
    --user)     USER="$2";     shift 2 ;;
    --db)       DB="$2";       shift 2 ;;
    --password) PASSWORD="$2"; shift 2 ;;
    *) echo "Unknown arg: $1"; exit 1 ;;
  esac
done

export PGPASSWORD="$PASSWORD"

# Prefer local psql; fall back to running inside the container.
if command -v psql &>/dev/null; then
  PSQL="psql -h $HOST -p $PORT -U $USER -d $DB"
else
  CONTAINER="${POSTGRES_CONTAINER:-lb-timescaledb}"
  echo "==> psql not found locally — using container '$CONTAINER'"
  PSQL="docker exec -e PGPASSWORD=$PASSWORD -i $CONTAINER psql -U $USER -d $DB"
fi

echo "==> Connecting to $USER@$HOST:$PORT/$DB"
$PSQL -c "SELECT version();" > /dev/null

# ── Sites ────────────────────────────────────────────────────────────────────
echo "==> Inserting sites"
$PSQL <<'SQL'
INSERT INTO site (id, name) VALUES
  ('site-001', 'Northside Factory'),
  ('site-002', 'Southbank Office'),
  ('site-003', 'Eastfield Warehouse')
ON CONFLICT (id) DO NOTHING;
SQL

# ── Meters ───────────────────────────────────────────────────────────────────
echo "==> Inserting meters"
$PSQL <<'SQL'
INSERT INTO meter (id, site_id, name) VALUES
  ('meter-001', 'site-001', 'Main Energy Meter'),
  ('meter-002', 'site-001', 'HVAC Energy Meter'),
  ('meter-003', 'site-001', 'Water Meter'),
  ('meter-004', 'site-002', 'Main Energy Meter'),
  ('meter-005', 'site-002', 'Water Meter'),
  ('meter-006', 'site-003', 'Main Energy Meter'),
  ('meter-007', 'site-003', 'Irrigation Water Meter')
ON CONFLICT (id) DO NOTHING;
SQL

# ── Points ───────────────────────────────────────────────────────────────────
echo "==> Inserting points"
$PSQL <<'SQL'
INSERT INTO point (id, meter_id, name) VALUES
  ('pt-001', 'meter-001', 'Energy kWh'),
  ('pt-002', 'meter-001', 'Demand kW'),
  ('pt-003', 'meter-002', 'Energy kWh'),
  ('pt-004', 'meter-002', 'Demand kW'),
  ('pt-005', 'meter-004', 'Energy kWh'),
  ('pt-006', 'meter-004', 'Demand kW'),
  ('pt-007', 'meter-006', 'Energy kWh'),
  ('pt-008', 'meter-006', 'Demand kW'),
  ('pt-009', 'meter-003', 'Flow L/min'),
  ('pt-010', 'meter-003', 'Total m3'),
  ('pt-011', 'meter-005', 'Flow L/min'),
  ('pt-012', 'meter-005', 'Total m3'),
  ('pt-013', 'meter-007', 'Flow L/min'),
  ('pt-014', 'meter-007', 'Total m3')
ON CONFLICT (id) DO NOTHING;
SQL

# ── Readings: one year at 15-minute intervals ─────────────────────────────────
echo "==> Generating one year of readings (15-min intervals, ~35 040 rows × 14 points)"
echo "    This may take a minute..."

$PSQL <<'SQL'
DO $$
DECLARE
  v_start TIMESTAMPTZ := NOW() - INTERVAL '1 year';
  v_end   TIMESTAMPTZ := NOW();
BEGIN

  DELETE FROM point_reading
   WHERE time >= v_start AND time <= v_end;

  -- Energy kWh per 15-min slot
  INSERT INTO point_reading (time, point_id, value)
  SELECT
    t,
    pt,
    ROUND((
      CASE
        WHEN pt IN ('pt-001','pt-003') THEN
          2.0
          + 1.5 * SIN(PI() * (EXTRACT(HOUR FROM t) - 6) / 12.0)
          + 0.5 * (1 - EXTRACT(DOW FROM t) / 6.0)
          + (RANDOM() * 1.2)
        WHEN pt = 'pt-005' THEN
          0.8
          + 0.6 * SIN(PI() * (EXTRACT(HOUR FROM t) - 8) / 10.0)
          + (RANDOM() * 0.4)
        WHEN pt = 'pt-007' THEN
          1.5
          + 0.8 * SIN(PI() * (EXTRACT(HOUR FROM t) - 7) / 11.0)
          + (RANDOM() * 0.6)
        ELSE 0.0
      END
    )::NUMERIC, 3)
  FROM
    generate_series(v_start, v_end, INTERVAL '15 minutes') AS t,
    unnest(ARRAY['pt-001','pt-003','pt-005','pt-007']) AS pt
  WHERE
    NOT (EXTRACT(HOUR FROM t) BETWEEN 0 AND 4 AND pt = 'pt-005');

  -- kW demand derived from kWh × 4
  INSERT INTO point_reading (time, point_id, value)
  SELECT
    time,
    CASE point_id
      WHEN 'pt-001' THEN 'pt-002'
      WHEN 'pt-003' THEN 'pt-004'
      WHEN 'pt-005' THEN 'pt-006'
      WHEN 'pt-007' THEN 'pt-008'
    END,
    ROUND((value * 4.0 + (RANDOM() * 2 - 1))::NUMERIC, 2)
  FROM point_reading
  WHERE point_id IN ('pt-001','pt-003','pt-005','pt-007')
    AND time >= v_start AND time <= v_end;

  -- Water flow L/min
  INSERT INTO point_reading (time, point_id, value)
  SELECT
    t,
    pt,
    ROUND(GREATEST(0.0,
      CASE
        WHEN pt = 'pt-009' THEN
          12.0
          + 8.0 * SIN(PI() * (EXTRACT(HOUR FROM t) - 6) / 12.0)
          + (RANDOM() * 5 - 1)
        WHEN pt = 'pt-011' THEN
          2.0
          + 1.5 * SIN(PI() * (EXTRACT(HOUR FROM t) - 8) / 10.0)
          + (RANDOM() * 1.5 - 0.3)
        WHEN pt = 'pt-013' THEN
          20.0 * GREATEST(0.0, SIN(PI() * (EXTRACT(HOUR FROM t) - 10) / 5.0))
          + (RANDOM() * 3)
        ELSE 0.0
      END
    )::NUMERIC, 2)
  FROM
    generate_series(v_start, v_end, INTERVAL '15 minutes') AS t,
    unnest(ARRAY['pt-009','pt-011','pt-013']) AS pt;

  -- Water total m³ (cumulative sum of flow * 15min / 1000)
  INSERT INTO point_reading (time, point_id, value)
  SELECT
    time,
    CASE point_id
      WHEN 'pt-009' THEN 'pt-010'
      WHEN 'pt-011' THEN 'pt-012'
      WHEN 'pt-013' THEN 'pt-014'
    END,
    ROUND((
      SUM(value * 15.0 / 1000.0)
        OVER (PARTITION BY point_id ORDER BY time)
    )::NUMERIC, 4)
  FROM point_reading
  WHERE point_id IN ('pt-009','pt-011','pt-013')
    AND time >= v_start AND time <= v_end;

END $$;
SQL

echo ""
echo "==> Done. Row counts:"
$PSQL -c "
SELECT s.name AS site, m.name AS meter, p.name AS point, COUNT(*) AS readings
FROM point_reading pr
JOIN point p ON p.id  = pr.point_id
JOIN meter m ON m.id  = p.meter_id
JOIN site  s ON s.id  = m.site_id
GROUP BY s.name, m.name, p.name
ORDER BY s.name, m.name, p.name;
"
