"""The SQLite sink for the demo-building seeder (sqlite-datasource-demo scope).

Writes the SAME schema + rows as the TimescaleDB path into one `.db` file via
Python's stdlib ``sqlite3`` — one dataset definition (``inventory``/``generators``/
``tags``), two sinks. The file is then registered as a first-class ``kind:"sqlite"``
datasource (the federation sidecar's ``source/sqlite.rs`` engine); the "DSN" is the
file path, resolved on the node running the sidecar.

Idempotent: every table is dropped and recreated (the TRUNCATE-equivalent), so
re-running with the same window + interval produces identical row counts.
"""
from __future__ import annotations

import sqlite3
from typing import Iterable

# The Timescale-specific pieces (hypertable, TIMESTAMPTZ) map to plain SQLite types;
# readings keep the same ISO-8601 UTC text timestamps the postgres path emits.
SCHEMA = """
CREATE TABLE site (
    id   TEXT PRIMARY KEY,
    name TEXT NOT NULL
);
CREATE TABLE meter (
    id      TEXT PRIMARY KEY,
    site_id TEXT NOT NULL REFERENCES site(id) ON DELETE CASCADE,
    name    TEXT NOT NULL
);
CREATE TABLE point (
    id       TEXT PRIMARY KEY,
    meter_id TEXT NOT NULL REFERENCES meter(id) ON DELETE CASCADE,
    name     TEXT NOT NULL
);
CREATE TABLE point_reading (
    time     TEXT NOT NULL,
    point_id TEXT NOT NULL REFERENCES point(id) ON DELETE CASCADE,
    value    REAL NOT NULL
);
CREATE INDEX idx_point_reading_point_id ON point_reading (point_id, time DESC);
CREATE TABLE site_tag (
    site_id TEXT NOT NULL REFERENCES site(id) ON DELETE CASCADE,
    tag     TEXT NOT NULL,
    kind    TEXT NOT NULL,
    val     TEXT,
    PRIMARY KEY (site_id, tag)
);
CREATE TABLE meter_tag (
    meter_id TEXT NOT NULL REFERENCES meter(id) ON DELETE CASCADE,
    tag      TEXT NOT NULL,
    kind     TEXT NOT NULL,
    val      TEXT,
    PRIMARY KEY (meter_id, tag)
);
CREATE TABLE point_tag (
    point_id TEXT NOT NULL REFERENCES point(id) ON DELETE CASCADE,
    tag      TEXT NOT NULL,
    kind     TEXT NOT NULL,
    val      TEXT,
    PRIMARY KEY (point_id, tag)
);
"""

TABLES = ("point_reading", "point_tag", "meter_tag", "site_tag",
          "point", "meter", "site")

BATCH = 50_000


class SqliteSink:
    """Open (or reset) the demo `.db` file and stream rows into it in batches."""

    def __init__(self, path: str):
        self.path = path
        self.conn = sqlite3.connect(path)
        # A seeder-owned file: durability shortcuts are fine and 10x faster.
        self.conn.execute("PRAGMA journal_mode = OFF")
        self.conn.execute("PRAGMA synchronous = OFF")
        self._reset()

    def describe(self) -> str:
        return f"sqlite3 {self.path}"

    def _reset(self) -> None:
        for t in TABLES:
            self.conn.execute(f"DROP TABLE IF EXISTS {t}")
        self.conn.executescript(SCHEMA)
        self.conn.commit()

    def copy_in(self, table: str, cols: tuple[str, ...],
                rows: Iterable[Iterable[object]]) -> int:
        """Batched INSERT of ``rows`` into ``table``; returns rows written."""
        placeholders = ", ".join("?" for _ in cols)
        stmt = f"INSERT INTO {table} ({', '.join(cols)}) VALUES ({placeholders})"
        sent = 0
        batch: list[tuple] = []
        for row in rows:
            batch.append(tuple(row))
            if len(batch) >= BATCH:
                self.conn.executemany(stmt, batch)
                self.conn.commit()
                sent += len(batch)
                batch.clear()
        if batch:
            self.conn.executemany(stmt, batch)
            self.conn.commit()
            sent += len(batch)
        return sent

    def summary(self) -> str:
        cur = self.conn.execute(
            """SELECT s.name, m.name, p.name, COUNT(*)
                 FROM point_reading pr
                 JOIN point p ON p.id = pr.point_id
                 JOIN meter m ON m.id = p.meter_id
                 JOIN site  s ON s.id = m.site_id
                GROUP BY s.name, m.name, p.name
                ORDER BY s.name, m.name, p.name"""
        )
        lines = [f"{site} | {meter} | {point} | {n}"
                 for site, meter, point, n in cur.fetchall()]
        return "\n".join(lines)

    def close(self) -> None:
        self.conn.commit()
        self.conn.close()
