#!/usr/bin/env python3
"""Seed TimescaleDB with one year of randomized building data at 5-min slots.

Generates, per the declarative catalog in ``inventory.py``:

* 8 sites (factory, office, warehouse, hospital, mall, apartments, airport, DC)
* ~70 meters / equipment rows (utility + AHU/RTU/chiller/boiler/FCU/CRAH)
* ~450 points (energy, demand, water, gas, zone/supply/return temp, AC /
  compressor / fan status, mode, damper)
* Haystack tags on every site / meter / point
* ~1 year of readings at 5-minute intervals (~50 million rows total)

Randomness is **per meter id**: each meter gets its own seeded RNG, so re-
running produces the same dataset while different meters produce different
noise, phase and drift. HVAC points are coupled through one state machine per
equipment so compressor / fan / mode / temps stay physically consistent.

Usage:
    ./seed.py                       # defaults from POSTGRES_* env
    ./seed.py --host localhost --port 5433 --user lb --password lb_secret --db lb
    ./seed.py --months 3            # generate only the last 3 months (faster smoke test)
    ./seed.py --interval 15         # 15-minute slots instead of 5 (≈1/3 the rows)

Re-running is idempotent: every seeded table is TRUNCATEd first.
"""
from __future__ import annotations

import argparse
import csv
import os
import shutil
import subprocess
import sys
import time
from datetime import datetime, timedelta, timezone
from typing import Iterable, Iterator

import generators as gen
import inventory as inv
import tags


# ── CLI ──────────────────────────────────────────────────────────────────────

def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--host", default=os.environ.get("POSTGRES_HOST", "localhost"))
    p.add_argument("--port", default=os.environ.get("POSTGRES_PORT", "5433"))
    p.add_argument("--user", default=os.environ.get("POSTGRES_USER", "lb"))
    p.add_argument("--db",   default=os.environ.get("POSTGRES_DB",   "lb"))
    p.add_argument("--password", default=os.environ.get("POSTGRES_PASSWORD", "lb_secret"))
    p.add_argument("--container", default=os.environ.get("POSTGRES_CONTAINER", "lb-timescaledb"))
    p.add_argument("--interval", type=int, default=5,
                   help="minutes per reading (default 5)")
    p.add_argument("--months", type=float, default=12.0,
                   help="how many months back from now to generate (default 12)")
    p.add_argument("--start", default=None,
                   help="pin the window start, e.g. 2025-01-01T00:00:00Z "
                        "(default: now - --months, snapped to --interval)")
    p.add_argument("--end", default=None,
                   help="pin the window end, e.g. 2026-01-01T00:00:00Z "
                        "(default: now, snapped to --interval)")
    p.add_argument("--no-readings", action="store_true",
                   help="only seed sites/meters/points/tags; skip point_reading")
    return p.parse_args()


# ── psql plumbing ────────────────────────────────────────────────────────────

class Psql:
    """Run SQL via local psql OR via `docker exec -i` if psql isn't installed."""

    def __init__(self, host: str, port: str, user: str, db: str,
                 password: str, container: str):
        env = os.environ.copy()
        env["PGPASSWORD"] = password
        self._env = env
        if shutil.which("psql"):
            self._base = ["psql", "-h", host, "-p", port, "-U", user, "-d", db]
            self._describe = f"psql -h {host} -p {port} -U {user} -d {db}"
        else:
            print(f"==> psql not found locally — using container '{container}'",
                  file=sys.stderr)
            self._base = ["docker", "exec", "-e", f"PGPASSWORD={password}",
                          "-i", container,
                          "psql", "-U", user, "-d", db]
            self._describe = f"docker exec -i {container} psql -U {user} -d {db}"

    def describe(self) -> str:
        return self._describe

    def run(self, sql: str, quiet: bool = False) -> subprocess.CompletedProcess:
        flags = ["-v", "ON_ERROR_STOP=1"]
        if quiet:
            flags += ["-q"]
        return subprocess.run(self._base + flags,
                              input=sql, text=True, env=self._env,
                              check=True, capture_output=True)

    def copy_in(self, copy_stmt: str, rows: Iterable[Iterable[object]],
                batch_size: int = 50_000) -> tuple[int, int]:
        """Stream ``rows`` as CSV into a single ``\\copy ... FROM STDIN``.

        Returns (rows_sent, msecs). Rows are streamed in chunks so memory stays
        flat even with tens of millions of rows. ``psql``'s own stdout/stderr
        are inherited so the user sees the ``COPY n`` line and any error from
        ``ON_ERROR_STOP`` directly.
        """
        proc = subprocess.Popen(self._base + ["-q", "-v", "ON_ERROR_STOP=1"],
                                stdin=subprocess.PIPE,
                                text=True, env=self._env, bufsize=1)
        assert proc.stdin is not None
        proc.stdin.write("BEGIN;\n")
        proc.stdin.write(copy_stmt + "\n")
        # csv.writer defaults to '\r\n' which psql CSV COPY rejects as a stray
        # newline inside an unquoted field; force unix line endings.
        writer = csv.writer(proc.stdin, lineterminator="\n")
        sent = 0
        for row in rows:
            writer.writerow(row)
            sent += 1
            if sent % batch_size == 0:
                proc.stdin.flush()
        proc.stdin.write("\\.\n")
        proc.stdin.write("COMMIT;\n")
        proc.stdin.close()
        rc = proc.wait()
        if rc != 0:
            raise RuntimeError(f"\\copy failed (rc={rc}); see psql output above")
        return sent, -1


# ── SQL fragments ────────────────────────────────────────────────────────────

ENSURE_SCHEMA = """
-- Tag tables (idempotent; matches init/02-schema.sql for already-running DBs).
CREATE TABLE IF NOT EXISTS site_tag (
    site_id TEXT NOT NULL REFERENCES site(id) ON DELETE CASCADE,
    tag     TEXT NOT NULL,
    kind    TEXT NOT NULL,
    val     TEXT,
    PRIMARY KEY (site_id, tag)
);
CREATE TABLE IF NOT EXISTS meter_tag (
    meter_id TEXT NOT NULL REFERENCES meter(id) ON DELETE CASCADE,
    tag      TEXT NOT NULL,
    kind     TEXT NOT NULL,
    val      TEXT,
    PRIMARY KEY (meter_id, tag)
);
CREATE TABLE IF NOT EXISTS point_tag (
    point_id TEXT NOT NULL REFERENCES point(id) ON DELETE CASCADE,
    tag      TEXT NOT NULL,
    kind     TEXT NOT NULL,
    val      TEXT,
    PRIMARY KEY (point_id, tag)
);
"""

TRUNCATE = """
TRUNCATE point_reading, point_tag, meter_tag, site_tag, point, meter, site
  RESTART IDENTITY CASCADE;
"""


# ── Row generators (CSV-friendly; NULLs become empty) ────────────────────────

def site_rows() -> Iterator[tuple[str, str]]:
    for s in inv.sites():
        yield (s.id, s.name)


def meter_rows() -> Iterator[tuple[str, str, str, str]]:
    for s in inv.sites():
        for m in s.meters:
            yield (m.id, s.id, m.name)


def point_rows() -> Iterator[tuple[str, str, str]]:
    for m, p in inv.all_points():
        yield (p.id, m.id, p.name)


def tag_rows_site() -> Iterator[tuple[str, str, str, str]]:
    for s in inv.sites():
        for site_id, tag, kind, val in tags.site_tags(s):
            yield (site_id, tag, kind, val if val is not None else "")


def tag_rows_meter() -> Iterator[tuple[str, str, str, str]]:
    for s in inv.sites():
        for m in s.meters:
            for meter_id, tag, kind, val in tags.meter_tags(s, m):
                yield (meter_id, tag, kind, val if val is not None else "")


def tag_rows_point() -> Iterator[tuple[str, str, str, str]]:
    for s in inv.sites():
        for m in s.meters:
            for p in m.points:
                for point_id, tag, kind, val in tags.point_tags(s, m, p):
                    yield (point_id, tag, kind, val if val is not None else "")


# ── Reading rows: built per meter, yielded lazily ────────────────────────────

ISO_FMT = "%Y-%m-%dT%H:%M:%S%z"


def _fmt_time(t: datetime) -> str:
    # TimescaleDB ingests this; force a Z suffix so it's clearly UTC.
    return t.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S+00:00")


def reading_rows(start: datetime, end: datetime, step_min: int
                 ) -> Iterator[tuple[str, str, float]]:
    """Yield (time, point_id, value) for every point on every meter."""
    for s in inv.sites():
        for m in s.meters:
            if m.kind in ("energy", "water", "gas"):
                series = gen.generate_scalar_series(m, start, end, step_min)
            else:
                series = gen.generate_hvac_series(m, s.climate, start, end, step_min)
            for point_id, samples in series.items():
                for t, v in samples:
                    yield (_fmt_time(t), point_id, v)


def _parse_iso(s: str) -> datetime:
    """Parse an ISO-8601 timestamp; assume UTC if no zone is given."""
    if s.endswith("Z"):
        s = s[:-1] + "+00:00"
    dt = datetime.fromisoformat(s)
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt.astimezone(timezone.utc)


def _snap_to_interval(t: datetime, step_min: int) -> datetime:
    """Floor ``t`` to the nearest interval boundary (UTC)."""
    epoch = datetime(1970, 1, 1, tzinfo=timezone.utc)
    secs = int((t - epoch).total_seconds())
    snapped = secs - (secs % (step_min * 60))
    return epoch + timedelta(seconds=snapped)


# ── Main ─────────────────────────────────────────────────────────────────────

def main() -> int:
    args = parse_args()
    stats = inv.stats()
    if args.end is not None:
        end = _parse_iso(args.end)
    else:
        end = _snap_to_interval(datetime.now(timezone.utc), args.interval)
    if args.start is not None:
        start = _parse_iso(args.start)
    else:
        raw_start = end - timedelta(days=int(args.months * 30.5))
        start = _snap_to_interval(raw_start, args.interval)
    slots = int((end - start).total_seconds() // 60) // args.interval
    est_rows = stats["points"] * slots

    print(f"==> Inventory: {stats['sites']} sites, {stats['meters']} meters, "
          f"{stats['points']} points")
    print(f"==> Window: {start.isoformat()} -> {end.isoformat()}")
    print(f"==> Interval={args.interval}m -> ~{slots:,} slots/point, "
          f"~{est_rows:,} readings total")
    print(f"==> Estimated runtime: a few minutes (depends on disk / container)")

    psql = Psql(args.host, args.port, args.user, args.db, args.password, args.container)
    print(f"==> Connecting via: {psql.describe()}")
    psql.run("SELECT version();")
    print("==> Connected.")

    print("==> Truncating + ensuring schema ...")
    psql.run(ENSURE_SCHEMA + TRUNCATE)

    print("==> Inserting sites / meters / points ...")
    psql.copy_in(_copy_stmt_for("site",  ("id", "name")),             site_rows())
    psql.copy_in(_copy_stmt_for("meter", ("id", "site_id", "name")),  meter_rows())
    psql.copy_in(_copy_stmt_for("point", ("id", "meter_id", "name")), point_rows())

    print("==> Inserting Haystack tags ...")
    psql.copy_in(_copy_stmt_for("site_tag",  ("site_id", "tag", "kind", "val")), tag_rows_site())
    psql.copy_in(_copy_stmt_for("meter_tag", ("meter_id","tag", "kind", "val")), tag_rows_meter())
    psql.copy_in(_copy_stmt_for("point_tag", ("point_id","tag", "kind", "val")), tag_rows_point())

    if args.no_readings:
        print("==> --no-readings set; skipping point_reading")
    else:
        print("==> Streaming readings via \\copy ... (this is the slow part)")
        t0 = time.time()
        stmt = _copy_stmt_for("point_reading", ("time", "point_id", "value"))
        sent, _ = psql.copy_in(stmt, reading_rows(start, end, args.interval))
        dur = time.time() - t0
        rate = sent / dur if dur else 0
        print(f"==> Wrote {sent:,} readings in {dur:.1f}s ({rate:,.0f} rows/s)")

    print()
    print("==> Done. Counts per site/meter/point:")
    summary = psql.run(SUMMARY_SQL).stdout
    print(summary, end="" if summary.endswith("\n") else "\n")
    return 0


def _copy_stmt_for(table: str, cols: tuple[str, ...]) -> str:
    col_list = ", ".join(cols)
    return f"\\copy {table} ({col_list}) FROM STDIN WITH (FORMAT csv, NULL '')"


SUMMARY_SQL = """
SELECT s.name  AS site,
       m.name  AS meter,
       p.name  AS point,
       COUNT(*) AS readings
  FROM point_reading pr
  JOIN point  p ON p.id  = pr.point_id
  JOIN meter  m ON m.id  = p.meter_id
  JOIN site   s ON s.id  = m.site_id
 GROUP BY s.name, m.name, p.name
 ORDER BY s.name, m.name, p.name;
"""


if __name__ == "__main__":
    sys.exit(main())
