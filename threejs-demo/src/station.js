// Builds the site. Loads the Small Red Gas Station glTF if present, else a
// procedural stand-in. Either way it returns a `fit` describing the model's
// bounding box, so hotspots can be placed as fractions of it (see layout.js).

import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";

// Drop-in real model (folder from Sketchfab, unzipped). Change if you use another.
const MODEL_URL = "./models/small_red_gas_station/scene.gltf";
const TARGET_WIDTH = 36; // fit the model to this many world units on its longest side

export async function buildStation(scene) {
  addGround(scene);

  const gltf = await tryLoad(MODEL_URL);
  if (gltf) {
    const fit = fitToGround(gltf.scene, TARGET_WIDTH);
    gltf.scene.traverse((o) => { if (o.isMesh) o.castShadow = o.receiveShadow = true; });
    scene.add(gltf.scene);
    console.log("[station] loaded real model. bbox:", fit.size, "center:", fit.center);
    return fit;
  }

  const g = proceduralStation();
  scene.add(g);
  return fitBox(g);
}

async function tryLoad(url) {
  try {
    return await new GLTFLoader().loadAsync(url);
  } catch (e) {
    console.warn("[station] no real model, using procedural stand-in:", e.message);
    return null;
  }
}

/** Center on origin, drop onto y=0, scale longest horizontal side to `target`. */
function fitToGround(obj, target) {
  let box = new THREE.Box3().setFromObject(obj);
  const size = box.getSize(new THREE.Vector3());
  const s = target / Math.max(size.x, size.z);
  obj.scale.setScalar(s);

  box = new THREE.Box3().setFromObject(obj);
  const c = box.getCenter(new THREE.Vector3());
  obj.position.x -= c.x;
  obj.position.z -= c.z;
  obj.position.y -= box.min.y; // feet on the ground

  return fitBox(obj);
}

/** Bounding box of `obj` in world space, plus its center & size (post-transform). */
function fitBox(obj) {
  const box = new THREE.Box3().setFromObject(obj);
  const size = box.getSize(new THREE.Vector3());
  const center = box.getCenter(new THREE.Vector3());
  return { box, size, center, min: box.min.clone(), max: box.max.clone() };
}

/* ---------- procedural stand-in (used only if the model is missing) ---------- */

const MAT = {
  concrete: new THREE.MeshStandardMaterial({ color: 0x3a4150, roughness: 0.95 }),
  canopy:   new THREE.MeshStandardMaterial({ color: 0xd8dde6, roughness: 0.4, metalness: 0.3 }),
  redTrim:  new THREE.MeshStandardMaterial({ color: 0xc9402f, roughness: 0.5 }),
  wall:     new THREE.MeshStandardMaterial({ color: 0x8d94a3, roughness: 0.8 }),
  glass:    new THREE.MeshStandardMaterial({ color: 0x6fb7ff, roughness: 0.1, metalness: 0.2, transparent: true, opacity: 0.45 }),
  pump:     new THREE.MeshStandardMaterial({ color: 0xe3e7ee, roughness: 0.4 }),
  charger:  new THREE.MeshStandardMaterial({ color: 0x2fbf71, roughness: 0.4 }),
  solar:    new THREE.MeshStandardMaterial({ color: 0x143a6b, roughness: 0.25, metalness: 0.6 }),
  steel:    new THREE.MeshStandardMaterial({ color: 0x9aa2b1, roughness: 0.3, metalness: 0.8 }),
  dark:     new THREE.MeshStandardMaterial({ color: 0x22262f, roughness: 0.7 }),
};

function box(w, h, d, mat, x, y, z) {
  const m = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), mat);
  m.position.set(x, y, z);
  m.castShadow = m.receiveShadow = true;
  return m;
}

function proceduralStation() {
  const g = new THREE.Group();
  g.add(box(0.5, 4.6, 0.5, MAT.steel, -6, 2.3, 2));
  g.add(box(0.5, 4.6, 0.5, MAT.steel,  3, 2.3, 2));
  g.add(box(0.5, 4.6, 0.5, MAT.steel, -6, 2.3, 8));
  g.add(box(0.5, 4.6, 0.5, MAT.steel,  3, 2.3, 8));
  g.add(box(13, 0.7, 10, MAT.canopy, -1.5, 5.0, 5));
  g.add(box(13.2, 0.35, 10.2, MAT.redTrim, -1.5, 4.6, 5));
  for (let i = 0; i < 4; i++) {
    const p = box(2.6, 0.1, 8.5, MAT.solar, -6 + i * 3, 5.65, 5);
    p.rotation.z = -0.18;
    g.add(p);
  }
  for (const px of [-4, 1]) {
    g.add(box(3.4, 0.25, 7, MAT.concrete, px, 0.13, 5));
    for (const pz of [3, 7]) {
      g.add(box(0.9, 1.7, 0.7, MAT.pump, px, 0.85, pz));
      g.add(box(0.7, 0.5, 0.1, MAT.dark, px, 1.35, pz + 0.41));
    }
  }
  g.add(box(12, 3.4, 7, MAT.wall, 13, 1.7, -9));
  g.add(box(11, 2.2, 0.2, MAT.glass, 12.5, 1.3, -5.4));
  g.add(box(12.4, 0.4, 7.4, MAT.redTrim, 13, 3.6, -9));
  g.add(box(1.8, 1.0, 1.4, MAT.steel, 13.5, 4.3, -9));
  g.add(box(3, 2.4, 3, MAT.canopy, 17, 1.2, -6));
  g.add(box(6, 0.2, 4, MAT.concrete, -14, 0.1, 8));
  for (let i = 0; i < 3; i++) g.add(box(0.7, 1.9, 0.5, MAT.charger, -16 + i * 2, 1.05, 7));
  g.add(box(7, 3.4, 5, MAT.wall, -16, 1.7, 0));
  g.add(box(7.4, 0.4, 5.4, MAT.dark, -16, 3.6, 0));
  g.add(box(0.2, 2.6, 4, MAT.dark, -12.6, 1.3, 0));
  g.add(box(1.6, 1.9, 0.7, MAT.dark, 4, 0.95, -12));
  g.add(box(1.4, 0.9, 0.05, MAT.steel, 4, 1.1, -11.6));
  g.add(box(0.4, 6, 0.4, MAT.steel, -20, 3, 14));
  g.add(box(3, 2.4, 0.5, MAT.redTrim, -20, 6.2, 14));
  return g;
}

function addGround(scene) {
  const ground = new THREE.Mesh(
    new THREE.CircleGeometry(80, 64),
    new THREE.MeshStandardMaterial({ color: 0x14171f, roughness: 1 }),
  );
  ground.rotation.x = -Math.PI / 2;
  ground.receiveShadow = true;
  scene.add(ground);
  const grid = new THREE.GridHelper(160, 80, 0x232a38, 0x1a1f2a);
  grid.position.y = 0.01;
  scene.add(grid);
}
