// Wires everything together: stage + station + hotspots + drawer + fake data.

import { createStage } from "./stage.js";
import { buildStation } from "./station.js";
import { createHotspots } from "./hotspots.js";
import { createDrawer } from "./drawer.js";
import { createData } from "./data.js";

const canvas = document.getElementById("stage");
const stage = createStage(canvas);
const data = createData();
const drawer = createDrawer({ data });

const fit = await buildStation(stage.scene);

const hotspots = createHotspots({
  container: document.getElementById("hotspots"),
  camera: stage.camera,
  canvas,
  scene: stage.scene,
  fit,
  onSelect: (id) => drawer.open(id),
});

// frame the camera on the fitted model
stage.controls.target.set(fit.center.x, fit.size.y * 0.45, fit.center.z);
stage.camera.position.set(fit.size.x * 0.9, fit.size.y * 1.2, fit.size.z * 1.1);
stage.controls.update();

// badges follow their 3D anchors every frame
stage.onFrame.add(hotspots.reproject);

// fake live data → badges + open drawer
data.subscribe((snapshot) => {
  hotspots.update(snapshot);
  drawer.refresh();
});
data.start(1500);

// topbar clock
const clock = document.getElementById("clock");
function tickClock() {
  clock.textContent = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}
tickClock();
setInterval(tickClock, 10_000);

// fade the hint once the user starts orbiting
canvas.addEventListener("pointerdown", () => {
  document.getElementById("hint").classList.add("fade");
}, { once: true });
