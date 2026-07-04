"""Declarative inventory: which sites, equipment and points to seed.

This module holds no logic — it is the *catalog* that ``seed.py`` walks and
``generators.py`` turns into time-series. Each entity carries the parameters
its generator needs (base load, hours of operation, climate, ...) so the
random values come out *different per meter* but *reproducible per meter id*
(the generator seeds its RNG from the meter id).

Conventions
-----------
* ``meter`` rows hold BOTH utility meters AND HVAC equipment (AHU, RTU,
  chiller, ...). The Haystack tags on ``meter_tag`` say which is which.
* Legacy ids (``site-001/002/003``, ``meter-001..007``, ``pt-001..014``) are
  kept stable so any existing queries against the old seed still resolve.
* HVAC ``hours`` are weekdays by default; ``weekend_factor`` and
  ``weekend_open`` control Saturday/Sunday behaviour.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable


# ── Value types ──────────────────────────────────────────────────────────────

@dataclass(frozen=True)
class Point:
    """One point (a time-series). ``kind`` selects the generator."""

    id: str
    name: str
    kind: str               # energy_kwh | demand_kw | water_flow | water_total
                            # hvac_zone_temp | hvac_supply_temp | hvac_return_temp
                            # hvac_ac_status | hvac_comp_status | hvac_fan_status
                            # hvac_mode | hvac_damper | hvac_sp | gas_kwh
    unit: str = ""          # haystack unit literal: "kWh","kW","m3","degC","%"
    tags: tuple[str, ...] = ()   # extra haystack markers (zone, discharge, ...)


@dataclass(frozen=True)
class Meter:
    """A meter OR a piece of HVAC equipment (Haystack treats them the same)."""

    id: str
    name: str
    kind: str               # energy | water | gas | ahu | rtu | chiller | boiler | fcu | crah
    base: float             # baseline magnitude for the generator
    amplitude: float        # diurnal swing amplitude
    sigma: float = 0.15     # gaussian noise sigma
    hours: tuple[int, int] = (0, 24)   # operating window (local clock)
    weekend_factor: float = 1.0        # multiplier Sat/Sun
    weekend_open: float = 0.0          # probability the unit runs on a weekend day
    climate_offset: float = 0.0        # +/- degC shift for outdoor-air model
    points: tuple[Point, ...] = ()
    tags: tuple[str, ...] = ()


@dataclass(frozen=True)
class Site:
    id: str
    name: str
    city: str
    country: str
    tz: str
    climate: str            # cold | temperate | warm  (drives outdoor temp band)
    latlon: tuple[float, float]
    area_m2: int
    meters: tuple[Meter, ...] = ()


# ── HVAC point sets (reused across equipment types) ──────────────────────────

# DX-style unit with a compressor (RTU / FCU / CRAH): 7 points each.
_DX_POINTS = (
    Point("{id}-zt",  "Zone Temp",        "hvac_zone_temp",   "degC",  ("zone","temp","sensor")),
    Point("{id}-sat", "Supply Air Temp",  "hvac_supply_temp", "degC",  ("discharge","supply","temp","sensor")),
    Point("{id}-rat", "Return Air Temp",  "hvac_return_temp", "degC",  ("return","temp","sensor")),
    Point("{id}-run", "AC Status",        "hvac_ac_status",   "",      ("run","cmd","equipRef")),
    Point("{id}-cmp", "Compressor Status","hvac_comp_status", "",      ("compressor","run")),
    Point("{id}-fan", "Fan Status",       "hvac_fan_status",  "",      ("fan","run")),
    Point("{id}-mod", "Mode",             "hvac_mode",        "",      ("mode","cooling","heating")),
)

# Hydronic unit without a compressor (AHU served by chiller/boiler): no comp.
_HYD_POINTS = (
    Point("{id}-zt",  "Zone Temp",        "hvac_zone_temp",   "degC",  ("zone","temp","sensor")),
    Point("{id}-sat", "Supply Air Temp",  "hvac_supply_temp", "degC",  ("discharge","supply","temp","sensor")),
    Point("{id}-rat", "Return Air Temp",  "hvac_return_temp", "degC",  ("return","temp","sensor")),
    Point("{id}-run", "AC Status",        "hvac_ac_status",   "",      ("run","cmd","equipRef")),
    Point("{id}-fan", "Fan Status",       "hvac_fan_status",  "",      ("fan","run")),
    Point("{id}-mod", "Mode",             "hvac_mode",        "",      ("mode","cooling","heating")),
    Point("{id}-vlv", "Valve Position",   "hvac_damper",      "%",     ("damper","cmd")),
)

# Chiller / boiler plant: water-side, no zone temp.
_PLANT_POINTS = (
    Point("{id}-st",  "Supply Temp",      "hvac_supply_temp", "degC",  ("supply","temp","sensor","leaving")),
    Point("{id}-rt",  "Return Temp",      "hvac_return_temp", "degC",  ("return","temp","sensor","entering")),
    Point("{id}-run", "Plant Status",     "hvac_ac_status",   "",      ("run","cmd","equipRef")),
    Point("{id}-fan", "Pump Status",      "hvac_fan_status",  "",      ("pump","run")),
    Point("{id}-mod", "Mode",             "hvac_mode",        "",      ("mode","cooling","heating")),
)


def _expand(template_points: tuple[Point, ...], prefix: str) -> tuple[Point, ...]:
    """Fill in the ``{id}`` placeholder in point ids/tags with a meter prefix."""
    out = []
    for p in template_points:
        pid = p.id.replace("{id}", prefix)
        tags = tuple(t.replace("{id}", prefix) for t in p.tags if t != "equipRef")
        out.append(Point(pid, p.name, p.kind, p.unit, tags))
    return tuple(out)


def _energy_meter(mid: str, base: float, amp: float) -> Meter:
    pts = (
        Point(f"{mid}-kwh", "Energy kWh", "energy_kwh", "kWh", ("energy","elec")),
        Point(f"{mid}-kw",  "Demand kW",  "demand_kw",  "kW",  ("power","demand","elec")),
    )
    return Meter(mid, "Main Energy Meter", "energy", base, amp, sigma=0.12,
                 hours=(0, 24), weekend_factor=0.75, points=pts,
                 tags=("elec","meter","energy","meter"))


def _hvac_energy_meter(mid: str, base: float, amp: float) -> Meter:
    pts = (
        Point(f"{mid}-kwh", "HVAC Energy kWh", "energy_kwh", "kWh", ("energy","elec","hvac")),
        Point(f"{mid}-kw",  "HVAC Demand kW",  "demand_kw",  "kW",  ("power","demand","elec","hvac")),
    )
    return Meter(mid, "HVAC Energy Meter", "energy", base, amp, sigma=0.12,
                 hours=(6, 20), weekend_factor=0.25, points=pts,
                 tags=("elec","meter","energy","hvac","meter"))


def _water_meter(mid: str, base: float, amp: float, name: str = "Water Meter") -> Meter:
    pts = (
        Point(f"{mid}-flow", "Flow L/min", "water_flow",   "L/min", ("flow","water")),
        Point(f"{mid}-tot",  "Total m3",   "water_total",  "m3",    ("volume","water")),
    )
    return Meter(mid, name, "water", base, amp, sigma=0.25,
                 hours=(6, 22), weekend_factor=0.55, points=pts,
                 tags=("water","meter","meter"))


def _gas_meter(mid: str, base: float, amp: float) -> Meter:
    pts = (
        Point(f"{mid}-kwh", "Gas kWh", "gas_kwh", "kWh", ("energy","gas","thermal")),
    )
    return Meter(mid, "Gas Meter", "gas", base, amp, sigma=0.18,
                 hours=(5, 23), weekend_factor=0.6, points=pts,
                 tags=("gas","meter","energy","meter"))


def _equip(mid: str, name: str, kind: str, points_tpl: tuple[Point, ...],
           *, base: float, amp: float, hours=(7, 19), weekend_factor=0.05,
           weekend_open=0.15, climate_offset=0.0) -> Meter:
    pts = _expand(points_tpl, mid)
    extra_tags = (kind, "equip")
    return Meter(mid, name, kind, base, amp, sigma=0.2,
                 hours=hours, weekend_factor=weekend_factor,
                 weekend_open=weekend_open, climate_offset=climate_offset,
                 points=pts, tags=extra_tags)


# ── Sites ────────────────────────────────────────────────────────────────────

_SITES: list[Site] = []


def _add(site: Site) -> Site:
    _SITES.append(site)
    return site


# 1. Northside Factory — 24/7 industrial, heavy base load, 4 AHUs + chiller + boiler
_add(Site(
    "site-001", "Northside Factory", "Manchester", "UK", "London",
    climate="temperate", latlon=(53.48, -2.24), area_m2=18000,
    meters=(
        _energy_meter("meter-001", base=14.0, amp=4.5),
        _hvac_energy_meter("meter-002", base=3.5, amp=1.8),
        _water_meter("meter-003", base=10.0, amp=6.0),
        _gas_meter("meter-008", base=6.0, amp=3.0),
        *[_equip(f"meter-0{20+i}", f"AHU-{i+1}", "ahu", _HYD_POINTS,
                 base=0.0, amp=0.0, hours=(5, 22), weekend_factor=0.4,
                 weekend_open=0.3)
          for i in range(4)],
        _equip("meter-030", "Chiller-1", "chiller", _PLANT_POINTS,
               base=0.0, amp=0.0, hours=(5, 22), weekend_factor=0.3),
        _equip("meter-031", "Boiler-1", "boiler", _PLANT_POINTS,
               base=0.0, amp=0.0, hours=(5, 22), weekend_factor=0.3),
    ),
))

# 2. Southbank Office — 9-5 office, 2 AHUs + 2 RTUs (legacy ids kept)
_add(Site(
    "site-002", "Southbank Office", "London", "UK", "London",
    climate="temperate", latlon=(51.51, -0.12), area_m2=6500,
    meters=(
        _energy_meter("meter-004", base=2.4, amp=1.4),
        _water_meter("meter-005", base=2.0, amp=1.8),
        *[_equip(f"meter-0{40+i}", f"AHU-{i+1}", "ahu", _HYD_POINTS,
                 base=0.0, amp=0.0, hours=(7, 19), weekend_factor=0.0,
                 weekend_open=0.1)
          for i in range(2)],
        *[_equip(f"meter-0{50+i}", f"RTU-{i+1}", "rtu", _DX_POINTS,
                 base=0.0, amp=0.0, hours=(7, 19), weekend_factor=0.0,
                 weekend_open=0.12)
          for i in range(2)],
    ),
))

# 3. Eastfield Warehouse — logistics, 2 RTUs + unit heater, mostly weekday
_add(Site(
    "site-003", "Eastfield Warehouse", "Birmingham", "UK", "London",
    climate="temperate", latlon=(52.48, -1.89), area_m2=12000,
    meters=(
        _energy_meter("meter-006", base=3.8, amp=1.6),
        _water_meter("meter-007", base=4.0, amp=2.5, name="Irrigation Water Meter"),
        _gas_meter("meter-009", base=4.0, amp=2.5),
        *[_equip(f"meter-0{60+i}", f"RTU-{i+1}", "rtu", _DX_POINTS,
                 base=0.0, amp=0.0, hours=(6, 18), weekend_factor=0.0,
                 weekend_open=0.05)
          for i in range(2)],
        _equip("meter-062", "Unit Heater", "ahu", _HYD_POINTS,
               base=0.0, amp=0.0, hours=(5, 10), weekend_factor=0.0,
               weekend_open=0.0, climate_offset=-1.0),
    ),
))

# 4. Westend Hospital — 24/7 healthcare, 3 AHUs + 2 chillers + boiler
_add(Site(
    "site-004", "Westend Hospital", "Leeds", "UK", "London",
    climate="temperate", latlon=(53.80, -1.55), area_m2=22000,
    meters=(
        _energy_meter("meter-100", base=18.0, amp=5.0),
        _hvac_energy_meter("meter-101", base=6.0, amp=2.0),
        _water_meter("meter-102", base=14.0, amp=5.0),
        _gas_meter("meter-103", base=8.0, amp=4.0),
        *[_equip(f"meter-1{10+i}", f"AHU-{i+1}", "ahu", _HYD_POINTS,
                 base=0.0, amp=0.0, hours=(0, 24), weekend_factor=0.9,
                 weekend_open=1.0)
          for i in range(3)],
        *[_equip(f"meter-1{20+i}", f"Chiller-{i+1}", "chiller", _PLANT_POINTS,
                 base=0.0, amp=0.0, hours=(0, 24), weekend_factor=0.9,
                 weekend_open=1.0)
          for i in range(2)],
        _equip("meter-123", "Boiler-1", "boiler", _PLANT_POINTS,
               base=0.0, amp=0.0, hours=(0, 24), weekend_factor=0.9,
               weekend_open=1.0),
    ),
))

# 5. Central Mall — retail, 10-21 weekdays + weekends open, 4 AHUs + 2 RTUs + 2 chillers
_add(Site(
    "site-005", "Central Mall", "Manchester", "UK", "London",
    climate="temperate", latlon=(53.48, -2.24), area_m2=28000,
    meters=(
        _energy_meter("meter-200", base=11.0, amp=5.0),
        _hvac_energy_meter("meter-201", base=4.0, amp=2.0),
        _water_meter("meter-202", base=8.0, amp=4.0),
        *[_equip(f"meter-2{10+i}", f"AHU-{i+1}", "ahu", _HYD_POINTS,
                 base=0.0, amp=0.0, hours=(10, 21), weekend_factor=0.9,
                 weekend_open=0.95)
          for i in range(4)],
        *[_equip(f"meter-2{20+i}", f"RTU-{i+1}", "rtu", _DX_POINTS,
                 base=0.0, amp=0.0, hours=(10, 21), weekend_factor=0.9,
                 weekend_open=0.95)
          for i in range(2)],
        *[_equip(f"meter-2{30+i}", f"Chiller-{i+1}", "chiller", _PLANT_POINTS,
                 base=0.0, amp=0.0, hours=(10, 21), weekend_factor=0.9,
                 weekend_open=0.9)
          for i in range(2)],
    ),
))

# 6. Lakeside Apartments — residential, smaller load, morning/evening peaks
_add(Site(
    "site-006", "Lakeside Apartments", "Glasgow", "UK", "London",
    climate="cold", latlon=(55.86, -4.25), area_m2=4200,
    meters=(
        _energy_meter("meter-300", base=1.5, amp=0.9),
        _water_meter("meter-301", base=1.2, amp=0.8),
        _gas_meter("meter-302", base=3.0, amp=1.5),
        *[_equip(f"meter-3{10+i}", f"FCU-{i+1}", "fcu", _DX_POINTS,
                 base=0.0, amp=0.0, hours=(6, 9), weekend_factor=1.0,
                 weekend_open=0.5, climate_offset=-1.5)
          for i in range(3)],
    ),
))

# 7. Airport Terminal T2 — 24/7 transport, 6 AHUs + 2 chillers
_add(Site(
    "site-007", "Airport Terminal T2", "Manchester", "UK", "London",
    climate="temperate", latlon=(53.35, -2.28), area_m2=45000,
    meters=(
        _energy_meter("meter-400", base=28.0, amp=8.0),
        _hvac_energy_meter("meter-401", base=10.0, amp=3.0),
        _water_meter("meter-402", base=18.0, amp=6.0),
        *[_equip(f"meter-4{10+i}", f"AHU-{i+1}", "ahu", _HYD_POINTS,
                 base=0.0, amp=0.0, hours=(0, 24), weekend_factor=0.95,
                 weekend_open=1.0)
          for i in range(6)],
        *[_equip(f"meter-4{20+i}", f"Chiller-{i+1}", "chiller", _PLANT_POINTS,
                 base=0.0, amp=0.0, hours=(0, 24), weekend_factor=0.95,
                 weekend_open=1.0)
          for i in range(2)],
    ),
))

# 8. Riverside Data Center — 24/7 high-intensity, CRAHs + chillers, no heating
_add(Site(
    "site-008", "Riverside Data Center", "London", "UK", "London",
    climate="temperate", latlon=(51.49, -0.13), area_m2=8000,
    meters=(
        _energy_meter("meter-500", base=55.0, amp=6.0),    # IT load dominates, flat
        _hvac_energy_meter("meter-501", base=18.0, amp=3.0),
        _water_meter("meter-502", base=4.0, amp=1.5),
        *[_equip(f"meter-5{10+i}", f"CRAH-{i+1}", "crah", _DX_POINTS,
                 base=0.0, amp=0.0, hours=(0, 24), weekend_factor=1.0,
                 weekend_open=1.0)
          for i in range(4)],
        *[_equip(f"meter-5{20+i}", f"Chiller-{i+1}", "chiller", _PLANT_POINTS,
                 base=0.0, amp=0.0, hours=(0, 24), weekend_factor=1.0,
                 weekend_open=1.0)
          for i in range(2)],
    ),
))


# ── Public accessors ─────────────────────────────────────────────────────────

def sites() -> Iterable[Site]:
    return _SITES


def all_points() -> Iterable[tuple[Meter, Point]]:
    for site in _SITES:
        for m in site.meters:
            for p in m.points:
                yield m, p


def stats() -> dict[str, int]:
    n_meters = sum(len(s.meters) for s in _SITES)
    n_points = sum(len(m.points) for s in _SITES for m in s.meters)
    return {"sites": len(_SITES), "meters": n_meters, "points": n_points}
