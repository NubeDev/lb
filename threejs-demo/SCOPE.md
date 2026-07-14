# threejs-demo — interactive 3D gas station (demo)

Just a standalone three.js demo. A 3D gas station you can orbit around, with clickable
hotspots on the assets (HVAC, EV chargers, solar, etc.) that show some fake live-ish
values. No backend, no build step, no dependencies to install.

## Files (one thing each)

```
threejs-demo/
├─ index.html         ← canvas + drawer + importmap (three.js from CDN)
├─ styles.css         ← dark styling
├─ models/README.md   ← drop a .glb here to use a real model
└─ src/
   ├─ main.js         ← wires everything together
   ├─ stage.js        ← renderer / scene / lights / camera / orbit
   ├─ station.js      ← builds the station (procedural, or loads models/station.glb)
   ├─ layout.js       ← list of assets + where each hotspot sits in 3D
   ├─ hotspots.js     ← floating badges that track their 3D point; click to open
   ├─ drawer.js       ← detail panel with a little sparkline
   └─ data.js         ← FAKE data — random walk, all made up
```

## Run

```
cd threejs-demo && python3 -m http.server 5180
# open http://localhost:5180
```

No install, no build.

## Using a real model

Download a `.glb` (e.g. the Small Red Gas Station from Sketchfab), save it as
`models/station.glb`, reload. `station.js` uses it instead of the procedural stand-in.
