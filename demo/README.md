# Lazybones Hardware Lab

A reusable **Three.js hardware viewer** designed for AI agents to generate, explore, and visualize hardware product assemblies. The library provides a component catalog of common BMS/industrial controller parts and a product builder that composes them into real-world devices — all viewable in the browser with orbit/zoom/explode/selection.

Built as a demo and testbed. The long-term goal is to teach AI models to **author hardware products**: given a spec (ports, form factor, enclosure), an AI selects components from the catalog and composes them onto a PCB layout, generating a 3D product definition the viewer renders immediately.

---

## Quick start

```bash
cd demo
pnpm install
pnpm dev
```

Opens `http://localhost:5173` with the first product loaded.

---

## Stack

| Layer | Technology |
|-------|-----------|
| 3D engine | Three.js 0.160 |
| Build | Vite (vanilla TS) |
| Style | Static CSS, dark theme |
| Deps | None outside Three.js |

---

## Architecture

```
demo/src/
  core/           Stage + Product base class + materials — shared viewer plumbing
  components/     Reusable 3D primitives (ETH, RS485, P1P2, power, USB-C, CM4, PCB, box)
  products/       Concrete product definitions (BMS controller, …)
  builder/        Product index + compose/dismantle API — the AI-authoring surface
  main.ts         Entry: mount Stage, wire panel UI, load first product
```

### Core layer (`core/`)

- **Stage** — Three.js renderer, scene, lights, camera, orbit controls, raycaster selection.
- **Product** — abstract base. Owns a `THREE.Group`, a parts registry (selectable + explodable), and `frame()` for per-frame animation.
- **materials** — shared material factory (pcb, pad, steel, plastic, led, black, …).

### Component catalog (`components/`)

Every component is a **pure function** that takes an options bag and returns a `THREE.Group`. Naming follows the physical part, not the label (e.g., `Ethernet`, `RS485`, `P1P2`, `PowerSupply`, `USB`, `CM4`). Components set `userData.label` and `userData.meta` so the viewer panel can display them.

### Product layer (`products/`)

A product extends `Product` and calls `build()` — arranging components on a PCB inside an enclosure, registering parts for selection and explode. The first product is **LB-MINI-BMS**, a 2× PCB building-automation controller in a black aluminum enclosure.

### Builder (`builder/`)

The AI-authoring surface. The builder holds a **product index** (name → factory) and exposes `compose(definition) → Product` so an agent (or a UI form) can generate new products from a declarative spec.

---

## First product: LB-MINI-BMS

A dual-PCB BMS controller in a black anodized-aluminum DIN-rail enclosure.

```
 ┌──────────────────────────────────────────────────┐
 │               TOP (enclosure face)                │
 │  ┌──────┐  ┌──────────────────────────────┐       │
 │  │ ETH  │  │  CM4 (Raspberry Pi Compute    │       │
 │  │RJ45  │  │  Module 4 with WiFi + BLE)   │       │
 │  └──────┘  └──────────────────────────────┘       │
 ├──────────────────────────────────────────────────┤
 │              BOTTOM (enclosure face)               │
 │ ┌────┐┌────┐┌────┐┌────┐┌──────────┐┌──────────┐ │
 │ │ETH ││ETH ││485 ││485 ││  P1P2    ││ 24V     │ │
 │ │#2  ││#3  ││ A  ││ B  ││pluggable ││AC/DC IN │ │
 │ └────┘└────┘└────┘└────┘└──────────┘└──────────┘ │
 │  ┌──────┐                                        │
 │  │USB-C │                                        │
 │  └──────┘                                        │
 └──────────────────────────────────────────────────┘
```

### Port inventory

- **Top**: 1× Ethernet RJ45, 1× CM4 (WiFi + dual HDMI, on DIP mezzanine)
- **Bottom**: 2× Ethernet RJ45, 2× RS-485 (pluggable terminal), 1× P1P2 (BACnet MS/TP), 1× 24V AC/DC power supply terminal, 1× USB-C (device/OTG)

---

## For AI agents: product schema

An AI can describe a product like this and the builder composes it:

```json
{
  "name": "LB-MINI-BMS",
  "enclosure": { "width": 10.4, "depth": 8.2, "height": 2.4, "color": "#0e1116" },
  "pcbs": [
    { "width": 9.6, "depth": 7.4, "thickness": 0.12, "color": 0x1b6e3a, "y": 0.24 },
    { "width": 9.6, "depth": 7.4, "thickness": 0.12, "color": 0x1b6e3a, "y": 1.56 }
  ],
  "bottom": {
    "ports": [
      { "type": "eth", "x": -3.6, "label": "ETH-2", "speed100": false },
      { "type": "eth", "x": -1.8, "label": "ETH-3", "speed100": false },
      { "type": "rs485", "x": 0.0, "label": "RS485-A" },
      { "type": "rs485", "x": 1.8, "label": "RS485-B" },
      { "type": "p1p2", "x": 3.6, "label": "P1P2" },
      { "type": "power24", "x": 5.4, "label": "24V AC/DC" },
      { "type": "usbc", "x": -4.2, "label": "USB-C", "z": 3.0 }
    ]
  },
  "top": {
    "ports": [
      { "type": "eth", "x": -2.8, "label": "ETH-1" },
      { "type": "cm4", "x": 2.2, "label": "CM4" }
    ]
  }
}
```

The `compose` function maps `type` → component factory, `x` → position along the edge, and handles orientation per face.

---

## Roadmap

- [x] Core viewer (Stage + Product + materials)
- [x] Component catalog (ETH, RS485, PCB, box geom)
- [x] First product (LB-MINI-BMS)
- [ ] P1P2 BACnet MS/TP connector component
- [ ] 24V AC/DC power terminal component
- [ ] USB-C port component
- [ ] CM4 module component (DIP form factor)
- [ ] Product builder + compose API
- [ ] Entry point + panel UI

---

## Conventions

- One responsibility per file, ≤400 lines hard limit.
- Components export a single factory function (`Options → Group`).
- Products extend `Product` and override `build()` only.
- No mocks, no fake backends — Three.js renders real geometry.
- Materials are shared via `core/Product.materials`.