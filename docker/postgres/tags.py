"""Emit Haystack-style tags for every entity in the inventory.

Project Haystack models the world as **entities** (sites, equips, points)
each carrying a set of **tags**. A tag is either a bare ``marker`` (just
present, e.g. ``site``) or a name+value pair where the value is one of the
Haystack literals: ``ref`` (``@id``), ``str``, ``number`` (with optional unit),
``coord``, ``date``, ``time``, ``datetime``.

This module turns the inventory into three streams of rows:

    (site_id,  tag, kind, val)
    (meter_id, tag, kind, val)
    (point_id, tag, kind, val)

The DB stores them as ``site_tag`` / ``meter_tag`` / ``point_tag``. The exact
same entity can be reached by either its relational id (``meter.id``) or by
walking tags.

Tag reference (subset of the standard):
    site   marker
    dis    str         display name
    id     ref         self ref (= entity id, useful for grids)
    geoCity, geoCountry, tz, area, weatherRef
    equip  marker
    meter  marker      (utility meter)
    ahu/rtu/chiller/boiler/fcu/crah markers
    siteRef / equipRef refs
    elec, gas, water, thermal      markers (utility)
    energy, power, flow, volume    markers (quantity)
    his    marker      (historized)
    kind   str         "Number" | "Bool" | "Str"
    point  marker
    sensor, cmd, sp, run, fan, compressor, mode,
    zone, discharge, supply, return, outside,
    cooling, heating, temp
"""
from __future__ import annotations

from typing import Iterable

from inventory import Meter, Point, Site


# ── Tag row builders ─────────────────────────────────────────────────────────

def marker(tag: str) -> tuple[str, str, None]:
    return (tag, "marker", None)


def ref(tag: str, target_id: str, relationship: str | None = None) -> tuple[str, str, str]:
    val = f"@{target_id}" if relationship is None else f"@{target_id} {relationship}"
    return (tag, "ref", val)


def s(tag: str, value: str) -> tuple[str, str, str]:
    return (tag, "str", value)


def n(tag: str, value: float, unit: str = "") -> tuple[str, str, str]:
    val = f"{value:g}" if not unit else f"{value:g} {unit}"
    return (tag, "number", val)


def coord(lat: float, lon: float) -> tuple[str, str, str]:
    return ("geoCoord", "coord", f"{lat:g},{lon:g}")


# ── Site tags ────────────────────────────────────────────────────────────────

def site_tags(site: Site) -> Iterable[tuple[str, str, str, None | str]]:
    """Yield (site_id, tag, kind, val) rows."""
    yield (site.id, *marker("site"))
    yield (site.id, *s("dis", site.name))
    yield (site.id, *ref("id", site.id))
    yield (site.id, *s("geoCity", site.city))
    yield (site.id, *s("geoCountry", site.country))
    yield (site.id, *s("tz", site.tz))
    yield (site.id, *coord(*site.latlon))
    yield (site.id, *n("area", float(site.area_m2), "m2"))
    yield (site.id, *marker(site.climate))                    # cold | temperate | warm
    yield (site.id, *ref("weatherRef", f"{site.id}-weather"))  # placeholder station


# ── Meter / equip tags ───────────────────────────────────────────────────────

_KIND_TAG = {
    "energy":  ("meter", "elec"),
    "water":   ("meter", "water"),
    "gas":     ("meter", "gas"),
    "ahu":     ("equip", "ahu"),
    "rtu":     ("equip", "rtu"),
    "chiller": ("equip", "chiller"),
    "boiler":  ("equip", "boiler"),
    "fcu":     ("equip", "fcu"),
    "crah":    ("equip", "crah"),
}


def meter_tags(site: Site, meter: Meter) -> Iterable[tuple[str, str, str, None | str]]:
    """Yield (meter_id, tag, kind, val) rows."""
    kind_tag, type_tag = _KIND_TAG.get(meter.kind, ("equip", "equip"))
    yield (meter.id, *marker(kind_tag))                       # meter | equip
    yield (meter.id, *marker(type_tag))                       # ahu / rtu / elec / ...
    yield (meter.id, *s("dis", meter.name))
    yield (meter.id, *ref("id", meter.id))
    yield (meter.id, *ref("siteRef", site.id))
    # Operating-hours hint (Haystack Axon code reads these as sched refs).
    yield (meter.id, *s("operatingStart", f"{meter.hours[0]:02d}:00"))
    yield (meter.id, *s("operatingEnd",   f"{meter.hours[1]:02d}:00"))
    # Extra ad-hoc markers from inventory (e.g. hvac, thermal).
    for t in meter.tags:
        if t not in (kind_tag, type_tag):
            yield (meter.id, *marker(t))


# ── Point tags ───────────────────────────────────────────────────────────────

# Map a generator kind to the haystack kind literal + standard marker tags.
_POINT_PROFILE: dict[str, tuple[str, tuple[str, ...], tuple[str, ...]]] = {
    # generator_kind -> (haystack kind literal, base_markers, default_units)
    "energy_kwh":      ("Number", ("point", "his", "energy", "elec", "kwh"),       ("kWh",)),
    "demand_kw":       ("Number", ("point", "his", "power",  "elec", "demand"),    ("kW",)),
    "gas_kwh":         ("Number", ("point", "his", "energy", "gas", "thermal"),    ("kWh",)),
    "water_flow":      ("Number", ("point", "his", "flow",   "water"),             ("L/min",)),
    "water_total":     ("Number", ("point", "his", "volume", "water"),             ("m3",)),
    "hvac_zone_temp":  ("Number", ("point", "his", "sensor", "zone", "temp"),      ("degC",)),
    "hvac_supply_temp":("Number", ("point", "his", "sensor", "discharge", "supply", "temp"), ("degC",)),
    "hvac_return_temp":("Number", ("point", "his", "sensor", "return", "temp"),    ("degC",)),
    "hvac_ac_status":  ("Bool",   ("point", "his", "cmd",    "run"),               ("",)),
    "hvac_comp_status":("Bool",   ("point", "his", "cmd",    "compressor", "run"), ("",)),
    "hvac_fan_status": ("Bool",   ("point", "his", "cmd",    "fan", "run"),        ("",)),
    "hvac_mode":       ("Number", ("point", "his", "sp",     "mode"),              ("",)),
    "hvac_damper":     ("Number", ("point", "his", "cmd",    "damper"),            ("%",)),
}


def point_tags(site: Site, meter: Meter, point: Point
               ) -> Iterable[tuple[str, str, str, None | str]]:
    """Yield (point_id, tag, kind, val) rows."""
    profile = _POINT_PROFILE.get(point.kind)
    if profile is None:
        return
    haystack_kind, base_markers, (unit,) = profile
    yield (point.id, *marker("point"))
    yield (point.id, *s("dis", point.name))
    yield (point.id, *ref("id", point.id))
    yield (point.id, *ref("siteRef", site.id))
    yield (point.id, *ref("equipRef", meter.id))
    yield (point.id, *marker("his"))
    yield (point.id, *s("kind", haystack_kind))
    yield (point.id, *s("tz", site.tz))
    if unit:
        yield (point.id, *s("unit", unit))
    # Standard markers from the profile (dedupe).
    seen: set[str] = set()
    for m in (*base_markers, *point.tags):
        if m in ("point", "his") or m in seen:
            continue
        seen.add(m)
        yield (point.id, *marker(m))
