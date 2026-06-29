CREATE TABLE IF NOT EXISTS site (
    id   TEXT PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS meter (
    id      TEXT PRIMARY KEY,
    site_id TEXT NOT NULL REFERENCES site(id),
    name    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS point (
    id        TEXT    NOT NULL,
    meter_id  TEXT    NOT NULL REFERENCES meter(id),
    name      TEXT    NOT NULL,
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS point_reading (
    time     TIMESTAMPTZ NOT NULL,
    point_id TEXT        NOT NULL REFERENCES point(id),
    value    DOUBLE PRECISION NOT NULL
);

SELECT create_hypertable('point_reading', 'time', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_point_reading_point_id ON point_reading (point_id, time DESC);
