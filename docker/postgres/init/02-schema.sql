-- ── Entities ────────────────────────────────────────────────────────────────
-- `meter` hosts BOTH utility meters and HVAC equipment; the kind is recorded
-- as Haystack tags on meter_tag (meter, ahu, rtu, chiller, boiler, fcu, ...).

CREATE TABLE IF NOT EXISTS site (
    id   TEXT PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS meter (
    id      TEXT PRIMARY KEY,
    site_id TEXT NOT NULL REFERENCES site(id) ON DELETE CASCADE,
    name    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS point (
    id        TEXT    NOT NULL,
    meter_id  TEXT    NOT NULL REFERENCES meter(id) ON DELETE CASCADE,
    name      TEXT    NOT NULL,
    PRIMARY KEY (id)
);

-- ── Time-series ─────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS point_reading (
    time     TIMESTAMPTZ NOT NULL,
    point_id TEXT        NOT NULL REFERENCES point(id) ON DELETE CASCADE,
    value    DOUBLE PRECISION NOT NULL
);

SELECT create_hypertable('point_reading', 'time', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_point_reading_point_id
    ON point_reading (point_id, time DESC);

-- ── Haystack-style tags ─────────────────────────────────────────────────────
-- Each row is one tag on one entity. `kind` is the Haystack value kind:
--   marker | ref | str | number | coord | date | time | datetime
-- `val` is NULL for a marker; otherwise the stringified Haystack literal:
--   ref  -> "@site-001"            (optionally "@site-001 weatherRef")
--   str  -> "Northside Factory"
--   num  -> "1234 m2"              (number + optional unit, space-separated)
--   coord-> "53.5,-2.2"
-- This is the project-haystack tag model (entities have tags, not columns).

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
