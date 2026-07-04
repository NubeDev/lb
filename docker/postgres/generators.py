"""Time-series generators.

Each generator yields ``(time, value)`` tuples for a single point over a half-
open ``[start, end)`` range at ``step`` minutes. Generators are pure functions
of:

* the per-point ``Point`` config (kind, unit),
* the per-meter ``Meter`` config (base, amplitude, hours, weekend_factor, ...),
* the meter id, used to seed a private RNG so the same meter always produces
  the same numbers but two different meters produce different numbers.

Design notes
------------
* Randomness is **deterministic per meter id**: a ``random.Random(meter_id)``
  is built once per meter. Re-running the seed reproduces the same dataset,
  but two meters get genuinely different noise, phase, base, and offsets.
* HVAC points share state across the equipment's lifetime (zone temp evolves
  day by day). The full year is generated *per equipment*, with all of an
  equipment's points advanced together so compressor / fan / mode / temps
  stay physically consistent.
"""
from __future__ import annotations

import hashlib
import math
import random
from dataclasses import dataclass
from datetime import datetime, timedelta
from typing import Iterator

from inventory import Meter, Point


SECONDS_PER_HOUR = 3600.0
MONTHS_PER_YEAR = 12.0


# ── Time axis ────────────────────────────────────────────────────────────────

def time_axis(start: datetime, end: datetime, step_min: int) -> Iterator[datetime]:
    """Yield ``start, start+step, ...`` up to (not including) ``end``."""
    t = start
    step = timedelta(minutes=step_min)
    while t < end:
        yield t
        t += step


# ── Outdoor air temperature model ────────────────────────────────────────────
# One OAT curve per site. Cold sites sit lower; warm sites higher. We add a
# per-meter ``climate_offset`` so a basement FCU starts colder than a rooftop
# RTU at the same site.

_CLIMATE_BAND = {
    "cold":      (-2.0, 18.0),    # winter low, summer high
    "temperate": (3.0, 24.0),
    "warm":      (12.0, 32.0),
}


def outdoor_temp(t: datetime, climate: str, climate_offset: float,
                 rng: random.Random) -> float:
    """Synthesize an outdoor-air temperature for a given moment."""
    low, high = _CLIMATE_BAND.get(climate, _CLIMATE_BAND["temperate"])
    # Annual wave: peak ~ day 200 (mid-July in northern hemisphere).
    day_of_year = t.timetuple().tm_yday
    annual = -math.cos(2 * math.pi * day_of_year / 365.0)        # -1 in winter, +1 mid-summer
    seasonal = (low + high) / 2 + (high - low) / 2 * annual
    # Diurnal wave: min ~5am, max ~15:00.
    hour_frac = t.hour + t.minute / 60.0
    diurnal = -math.cos(2 * math.pi * (hour_frac - 5.0) / 24.0) * 3.5
    noise = rng.gauss(0.0, 1.0)
    return round(seasonal + diurnal + climate_offset + noise, 2)


# ── Energy / water / gas generators (pure scalar per slot) ───────────────────

def _diurnal_window(hour_frac: float, hours: tuple[int, int]) -> float:
    """0..1 envelope: 0 outside the window, half-cosine bell inside."""
    lo, hi = hours
    if hi <= lo:
        return 1.0
    if hour_frac < lo or hour_frac > hi:
        return 0.0
    span = hi - lo
    return 0.5 - 0.5 * math.cos(2 * math.pi * (hour_frac - lo) / span)


def _seasonal_boost(t: datetime) -> float:
    """Extra load in deep winter / peak summer (HVAC drives it)."""
    doy = t.timetuple().tm_yday
    annual = -math.cos(2 * math.pi * doy / 365.0)
    return 1.0 + 0.18 * abs(annual)


def _meter_seed(meter_id: str) -> random.Random:
    """Per-meter RNG: same id -> same noise. Different id -> different noise.

    Uses md5 (not ``hash()``) so the seed is stable across Python invocations
    (``hash()`` is randomized via ``PYTHONHASHSEED`` by default).
    """
    digest = hashlib.md5(meter_id.encode("utf-8")).digest()
    seed = int.from_bytes(digest[:8], "little")
    return random.Random(seed)


def _scalar_for_kind(meter: Meter, point: Point, t: datetime,
                     rng: random.Random) -> float | None:
    """Return the reading for a scalar point kind, or None to skip the slot.

    Returning None is used for the legacy "office closed overnight" pattern
    on the office energy meter: data is sparse rather than zero-filled.
    """
    kind = point.kind
    hour_frac = t.hour + t.minute / 60.0
    weekend = t.weekday() >= 5
    env = _diurnal_window(hour_frac, meter.hours)
    weekend_mult = meter.weekend_factor if weekend else 1.0

    if kind == "energy_kwh":
        # base + amplitude * bell + seasonal + noise. Tiny load even when closed.
        load = meter.base * 0.15 + meter.amplitude * env * weekend_mult
        load *= _seasonal_boost(t)
        noise = rng.gauss(0.0, meter.sigma * meter.base)
        return round(max(0.05, load + noise), 3)

    if kind == "demand_kw":
        # Demand is kWh converted to instantaneous kW: 5-min slot -> x12.
        return None  # filled in afterwards from the kWh twin

    if kind == "gas_kwh":
        # Gas is winter-heavy (heating). Almost zero in summer.
        doy = t.timetuple().tm_yday
        annual = -math.cos(2 * math.pi * doy / 365.0)            # +1 in winter
        winter_factor = max(0.05, (annual + 1) / 2)              # 0.05..1
        load = (meter.base * 0.2 + meter.amplitude * env * weekend_mult) * winter_factor
        noise = rng.gauss(0.0, meter.sigma * meter.base)
        return round(max(0.0, load + noise), 3)

    if kind == "water_flow":
        # Always allow a tiny overnight trickle (night-time toilet flushes,
        # cleaning crews, small leaks). Daytime flow follows the diurnal bell.
        flow = meter.base * 0.05 + meter.amplitude * env * weekend_mult
        noise = rng.gauss(0.0, meter.sigma * meter.base)
        return round(max(0.0, flow + noise), 2)

    if kind == "water_total":
        return None  # filled in from the flow twin (cumulative)

    return None


def generate_scalar_series(meter: Meter, start: datetime, end: datetime,
                           step_min: int) -> dict[str, list[tuple[datetime, float]]]:
    """Generate every scalar point on ``meter`` over ``[start, end)``.

    Returns ``{point_id: [(time, value), ...]}``. Derived series (``demand_kw``
    from ``energy_kwh``, ``water_total`` from ``water_flow``) are computed
    inside the same pass so they stay consistent with their twins.
    """
    rng = _meter_seed(meter.id)
    series: dict[str, list[tuple[datetime, float]]] = {
        p.id: [] for p in meter.points
    }
    kwh_pt = next((p for p in meter.points if p.kind == "energy_kwh"), None)
    kw_pt  = next((p for p in meter.points if p.kind == "demand_kw"),  None)
    flow_pt = next((p for p in meter.points if p.kind == "water_flow"),  None)
    tot_pt  = next((p for p in meter.points if p.kind == "water_total"), None)

    cumulative = 0.0
    slot_minutes = step_min
    for t in time_axis(start, end, step_min):
        for p in meter.points:
            v = _scalar_for_kind(meter, p, t, rng)
            if v is None:
                continue
            series[p.id].append((t, v))
            if p is kwh_pt and kw_pt is not None:
                # kW = kWh over slot / hours = kWh * 60 / slot_min
                kw = v * 60.0 / slot_minutes + rng.gauss(0.0, 0.2)
                series[kw_pt.id].append((t, round(max(0.0, kw), 2)))
            if p is flow_pt and tot_pt is not None:
                cumulative += v * slot_minutes / 1000.0          # L/min * min / 1000 = m3
                series[tot_pt.id].append((t, round(cumulative, 4)))
    return series


# ── HVAC state machine (per equipment, evolves across the year) ──────────────

@dataclass
class _HvacState:
    zone_temp: float = 21.0
    supply_temp: float = 21.0
    return_temp: float = 21.0


def _mode_for(outdoor: float) -> str:
    if outdoor < 14.0:
        return "heating"
    if outdoor > 19.0:
        return "cooling"
    return "off"


def _mode_code(mode: str) -> float:
    return {"off": 0.0, "cooling": 1.0, "heating": 2.0, "auto": 3.0}[mode]


def generate_hvac_series(meter: Meter, climate: str, start: datetime,
                         end: datetime, step_min: int
                         ) -> dict[str, list[tuple[datetime, float]]]:
    """Walk an equipment's points forward through time as one coupled system."""
    rng = _meter_seed(meter.id)
    st = _HvacState(zone_temp=21.5, supply_temp=21.0, return_temp=22.0)

    series: dict[str, list[tuple[datetime, float]]] = {p.id: [] for p in meter.points}
    pmap = {p.kind: p for p in meter.points}

    # Per-equipment setpoints drift a little (someone bumps the thermostat).
    cool_sp = 23.0 + rng.uniform(-0.5, 0.5)
    heat_sp = 20.5 + rng.uniform(-0.5, 0.5)
    # When this unit's weekend "open" roll hits, the day is occupied.
    weekend_open_today: dict[int, bool] = {}

    for t in time_axis(start, end, step_min):
        hour_frac = t.hour + t.minute / 60.0
        weekend = t.weekday() >= 5
        day_key = t.date().toordinal()

        # Weekend occupancy is rolled once per day (sticky for the whole day).
        if weekend and day_key not in weekend_open_today:
            weekend_open_today[day_key] = rng.random() < meter.weekend_open
        occupied_window = _diurnal_window(hour_frac, meter.hours) > 0.05
        schedule_on = occupied_window and (not weekend or weekend_open_today.get(day_key, False))

        outdoor = outdoor_temp(t, climate, meter.climate_offset, rng)
        mode = _mode_for(outdoor)
        if not schedule_on or mode == "off":
            ac_on = False
        else:
            # Hysteresis: turn on if zone is on the wrong side of setpoint.
            if mode == "cooling":
                ac_on = st.zone_temp > cool_sp - 0.5
            else:
                ac_on = st.zone_temp < heat_sp + 0.5

        # Evolve zone temp toward target (running) or outdoor (idle).
        if ac_on:
            target = cool_sp if mode == "cooling" else heat_sp
            st.zone_temp += (target - st.zone_temp) * 0.18 + rng.gauss(0.0, 0.08)
            supply_target = 14.0 if mode == "cooling" else 32.0
            st.supply_temp += (supply_target - st.supply_temp) * 0.4 + rng.gauss(0.0, 0.25)
            st.return_temp += (st.zone_temp + 1.5 - st.return_temp) * 0.2 + rng.gauss(0.0, 0.1)
        else:
            # Drift toward outdoor; supply relaxes toward zone; return follows.
            st.zone_temp += (outdoor - st.zone_temp) * 0.04 + rng.gauss(0.0, 0.12)
            st.supply_temp += (st.zone_temp - st.supply_temp) * 0.05 + rng.gauss(0.0, 0.1)
            st.return_temp += (st.zone_temp + 0.5 - st.return_temp) * 0.05 + rng.gauss(0.0, 0.1)

        compressor_on = ac_on and (mode == "cooling") and (st.zone_temp > cool_sp)
        fan_on = schedule_on                        # supply fan runs whenever occupied
        # Boiler/chiller plant "run" follows mode + schedule.
        plant_on = ac_on

        for kind, p in pmap.items():
            if kind == "hvac_zone_temp":
                series[p.id].append((t, round(st.zone_temp, 2)))
            elif kind == "hvac_supply_temp":
                series[p.id].append((t, round(st.supply_temp, 2)))
            elif kind == "hvac_return_temp":
                series[p.id].append((t, round(st.return_temp, 2)))
            elif kind == "hvac_ac_status":
                run = plant_on if meter.kind in ("chiller", "boiler") else ac_on
                series[p.id].append((t, 1.0 if run else 0.0))
            elif kind == "hvac_comp_status":
                series[p.id].append((t, 1.0 if compressor_on else 0.0))
            elif kind == "hvac_fan_status":
                series[p.id].append((t, 1.0 if fan_on else 0.0))
            elif kind == "hvac_mode":
                series[p.id].append((t, _mode_code(mode if schedule_on else "off")))
            elif kind == "hvac_damper":
                # Valve/damper %: open when running, modulate otherwise.
                pos = 80.0 if ac_on else (20.0 + rng.uniform(0, 15.0))
                series[p.id].append((t, round(pos, 1)))
    return series
