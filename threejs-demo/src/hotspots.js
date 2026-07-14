// Floating DOM badges that track a 3D anchor. Anchors are given as fractions of
// the model's fitted bounding box (layout.js) and resolved to world coords here.
// Each frame they're projected to screen space; badges behind the camera fade.
// SHIFT-CLICK the model to log a point's world XYZ + fraction (placement helper).

import * as THREE from "three";
import { LAYOUT } from "./layout.js";

export function createHotspots({ container, camera, canvas, scene, fit, onSelect }) {
  const v = new THREE.Vector3();

  // resolve a [fx,fy,fz] fraction into a world-space point inside the bbox
  const toWorld = ([fx, fy, fz]) =>
    new THREE.Vector3(
      THREE.MathUtils.lerp(fit.min.x, fit.max.x, fx),
      THREE.MathUtils.lerp(fit.min.y, fit.max.y, fy),
      THREE.MathUtils.lerp(fit.min.z, fit.max.z, fz),
    );

  const items = LAYOUT.map(({ id, frac }) => {
    const el = document.createElement("div");
    el.className = "hotspot";
    el.innerHTML = `
      <div class="hotspot__row">
        <span class="hotspot__led"></span>
        <span class="hotspot__name"></span>
      </div>
      <div class="hotspot__val">—</div>`;
    el.addEventListener("click", () => onSelect(id));
    container.appendChild(el);
    return {
      id,
      anchor: toWorld(frac),
      el,
      name: el.querySelector(".hotspot__name"),
      val: el.querySelector(".hotspot__val"),
    };
  });

  installPicker(canvas, camera, scene, fit);

  function update(snapshot) {
    for (const a of snapshot) {
      const it = items.find((i) => i.id === a.id);
      if (!it) continue;
      it.name.textContent = a.label;
      it.val.textContent = `${a.value.toFixed(1)} ${a.unit}`;
      it.el.className = `hotspot is-${a.status}`;
    }
  }

  function reproject() {
    const w = canvas.clientWidth, h = canvas.clientHeight;
    for (const it of items) {
      v.copy(it.anchor).project(camera);
      const behind = v.z > 1;
      it.el.style.opacity = behind ? "0" : "1";
      it.el.style.pointerEvents = behind ? "none" : "auto";
      it.el.style.left = `${(v.x * 0.5 + 0.5) * w}px`;
      it.el.style.top = `${(-v.y * 0.5 + 0.5) * h}px`;
    }
  }

  return { update, reproject };
}

// Shift-click a mesh: log the hit point as world XYZ and as a bbox fraction.
function installPicker(canvas, camera, scene, fit) {
  const ray = new THREE.Raycaster();
  const p = new THREE.Vector2();
  canvas.addEventListener("pointerdown", (ev) => {
    if (!ev.shiftKey) return;
    const r = canvas.getBoundingClientRect();
    p.x = ((ev.clientX - r.left) / r.width) * 2 - 1;
    p.y = -((ev.clientY - r.top) / r.height) * 2 + 1;
    ray.setFromCamera(p, camera);
    const hit = ray.intersectObjects(scene.children, true)[0];
    if (!hit) return;
    const w = hit.point;
    const frac = [
      (w.x - fit.min.x) / (fit.max.x - fit.min.x),
      (w.y - fit.min.y) / (fit.max.y - fit.min.y),
      (w.z - fit.min.z) / (fit.max.z - fit.min.z),
    ].map((n) => +n.toFixed(2));
    console.log(
      `[pick] world (${w.x.toFixed(1)}, ${w.y.toFixed(1)}, ${w.z.toFixed(1)})  frac [${frac.join(", ")}]`,
    );
  });
}
