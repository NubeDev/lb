// Where each asset's hotspot sits, as a FRACTION of the model's fitted bounding
// box: [fx, fy, fz] each in 0..1  (0 = min corner, 1 = max corner).
//
// Tuning: SHIFT-CLICK a part — the console logs world XYZ + the [fx,fy,fz] to
// paste here. These values are read off the rendered model (shop building on the
// left, big red fuel canopy on the right, pumps beneath it).

export const LAYOUT = [
  // left building
  { id: "car_wash",      frac: [0.20, 0.55, 0.30] }, // roller-door end of the shop building
  { id: "shop",          frac: [0.27, 0.45, 0.62] }, // shop body / entrance
  { id: "hvac",          frac: [0.30, 0.72, 0.45] }, // AC unit on the shop roof
  { id: "ev_chargers",   frac: [0.16, 0.30, 0.80] }, // charger post at the front corner
  { id: "switchboard",   frac: [0.36, 0.25, 0.70] }, // board by the shop wall

  // fuel canopy (right)
  { id: "fuel_canopy",   frac: [0.60, 0.35, 0.55] }, // pump island under the canopy
  { id: "solar",         frac: [0.68, 0.62, 0.45] }, // canopy roof
  { id: "refrigeration", frac: [0.82, 0.45, 0.40] }, // plant at the far canopy end
];
