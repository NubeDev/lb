// FAKE data. All made up — a little random walk so the demo feels alive.
// No backend, no network. Swap this file if you ever want real data.

const ASSETS = [
  { id: "solar",         label: "Solar System",       unit: "kWh", base: 142, kind: "gen" },
  { id: "hvac",          label: "HVAC",               unit: "°C",  base: 22,  kind: "env" },
  { id: "fuel_canopy",   label: "Fuel Canopy",        unit: "kW",  base: 186, kind: "load" },
  { id: "ev_chargers",   label: "EV Chargers",        unit: "kW",  base: 74,  kind: "load" },
  { id: "refrigeration", label: "Refrigeration",      unit: "kW",  base: 12.8, kind: "load" },
  { id: "car_wash",      label: "Car Wash",           unit: "kW",  base: 3,   kind: "load" },
  { id: "shop",          label: "Shop",               unit: "kW",  base: 28,  kind: "load" },
  { id: "switchboard",   label: "Main Switchboard",   unit: "kW",  base: 186, kind: "load" },
];

function statusFor(id, v, base) {
  // a couple of assets misbehave on purpose, like the mockup
  if (id === "refrigeration") return "critical";
  if (id === "ev_chargers") return "warning";
  return Math.abs(v - base) > base * 0.35 ? "warning" : "normal";
}

export function createData() {
  const state = {};
  for (const a of ASSETS) {
    // seed 60 points of history so sparklines have something to draw
    const hist = [];
    let v = a.base;
    for (let i = 0; i < 60; i++) {
      v += (Math.random() - 0.5) * a.base * 0.06;
      hist.push(Math.max(0, v));
    }
    state[a.id] = { value: v, hist };
  }

  const listeners = new Set();

  function tick() {
    for (const a of ASSETS) {
      const s = state[a.id];
      s.value = Math.max(0, s.value + (Math.random() - 0.5) * a.base * 0.05);
      s.hist.push(s.value);
      if (s.hist.length > 60) s.hist.shift();
    }
    const snapshot = ASSETS.map((a) => sample(a));
    for (const cb of listeners) cb(snapshot);
  }

  function sample(a) {
    const v = state[a.id].value;
    return {
      id: a.id,
      label: a.label,
      unit: a.unit,
      kind: a.kind,
      value: v,
      status: statusFor(a.id, v, a.base),
    };
  }

  return {
    assets: () => ASSETS.map((a) => sample(a)),
    history: (id) => state[id]?.hist.slice() ?? [],
    subscribe(cb) {
      listeners.add(cb);
      cb(ASSETS.map((a) => sample(a)));
      return () => listeners.delete(cb);
    },
    start(ms = 1500) {
      return setInterval(tick, ms);
    },
  };
}
