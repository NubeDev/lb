// The seeded office-floor demo scene (phase 3): walls, rooms with temp-tint bindings,
// doors, labels — the "floor breathing with simulator temperatures" shot.
//
// Envelope 400×256 (x −200..200, y −128..128), one closed outer wall chain, one
// horizontal + two vertical partitions → a 3×2 grid of rooms 101..106, each bound to
// its zone.* simulator channels. All coordinates are grid-aligned (tokens.grid.step = 8).

import type { SceneDoc } from "../scene.types";

export const floorplanDemo: SceneDoc = {
  v: 1,
  camera: "ortho-top",
  shapes: {
    // ---- walls ---------------------------------------------------------------
    "wall-outer": {
      type: "plan.wall",
      t: { x: 0, y: 0 },
      props: {
        points: [[-200, -128], [200, -128], [200, 128], [-200, 128], [-200, -128]],
        thickness: 8,
        label: "",
      },
    },
    // corridor spine: splits the floor into top / bottom rows
    "wall-mid": {
      type: "plan.wall",
      t: { x: 0, y: 0 },
      props: { points: [[-200, 0], [200, 0]], thickness: 8, label: "" },
    },
    // column partitions: 3 rooms per row
    "wall-v1": {
      type: "plan.wall",
      t: { x: 0, y: 0 },
      props: { points: [[-72, -128], [-72, 128]], thickness: 8, label: "" },
    },
    "wall-v2": {
      type: "plan.wall",
      t: { x: 0, y: 0 },
      props: { points: [[72, -128], [72, 128]], thickness: 8, label: "" },
    },
    // ---- rooms (3×2, top row 101-103, bottom row 104-106) ---------------------
    "room-101": {
      type: "plan.room",
      t: { x: -136, y: 64 },
      props: { w: 112, h: 112, name: "101", label: "" },
      bind: { temp: { channel: "zone.101.temp" }, occupied: { channel: "zone.101.occupied" } },
    },
    "room-102": {
      type: "plan.room",
      t: { x: 0, y: 64 },
      props: { w: 128, h: 112, name: "102", label: "" },
      bind: { temp: { channel: "zone.102.temp" }, occupied: { channel: "zone.102.occupied" } },
    },
    "room-103": {
      type: "plan.room",
      t: { x: 136, y: 64 },
      props: { w: 112, h: 112, name: "103", label: "" },
      bind: { temp: { channel: "zone.103.temp" }, occupied: { channel: "zone.103.occupied" } },
    },
    "room-104": {
      type: "plan.room",
      t: { x: -136, y: -64 },
      props: { w: 112, h: 112, name: "104", label: "" },
      bind: { temp: { channel: "zone.104.temp" }, occupied: { channel: "zone.104.occupied" } },
    },
    "room-105": {
      type: "plan.room",
      t: { x: 0, y: -64 },
      props: { w: 128, h: 112, name: "105", label: "" },
      bind: { temp: { channel: "zone.105.temp" }, occupied: { channel: "zone.105.occupied" } },
    },
    "room-106": {
      type: "plan.room",
      t: { x: 136, y: -64 },
      props: { w: 112, h: 112, name: "106", label: "" },
      bind: { temp: { channel: "zone.106.temp" }, occupied: { channel: "zone.106.occupied" } },
    },
    // ---- doors (hinges sit ON the wall line; r orients along the wall) --------
    "door-101": {
      type: "plan.door",
      t: { x: -160, y: 0 },
      props: { width: 32, swing: "left", label: "" },
    },
    "door-102": {
      type: "plan.door",
      t: { x: -24, y: 0 },
      props: { width: 32, swing: "left", label: "" },
    },
    "door-103": {
      type: "plan.door",
      t: { x: 112, y: 0 },
      props: { width: 32, swing: "left", label: "" },
    },
    // between 104 and 105, on the vertical partition
    "door-105": {
      type: "plan.door",
      t: { x: -72, y: -88, r: Math.PI / 2 },
      props: { width: 32, swing: "left", label: "" },
    },
    // ---- labels ---------------------------------------------------------------
    "label-title": {
      type: "plan.label",
      t: { x: 0, y: 152 },
      props: { text: "Level 1", size: 16, label: "" },
    },
    "label-z101": {
      type: "plan.label",
      t: { x: -136, y: 152 },
      props: { text: "Z101", size: 12, label: "" },
      bind: { value: { channel: "zone.101.temp" } },
    },
  },
};
