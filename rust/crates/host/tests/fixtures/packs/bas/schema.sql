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
