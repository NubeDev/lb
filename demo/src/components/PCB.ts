import * as THREE from "three";
import { box } from "./box";
import { materials } from "../core/Product";

export interface PcbOptions {
  width?: number;
  depth?: number;
  thickness?: number;
  color?: number;
}

/** A printed-circuit board slab with mounting holes and a decorative trace grid. */
export function PCB(opts: PcbOptions = {}): THREE.Group {
  const width = opts.width ?? 10;
  const depth = opts.depth ?? 8;
  const thickness = opts.thickness ?? 0.12;
  const color = opts.color ?? 0x1b6e3a;

  const g = new THREE.Group();
  g.name = "PCB";

  const board = box(width, thickness, depth, materials.pcb().clone());
  (board.material as THREE.MeshStandardMaterial).color.setHex(color);
  g.add(board);

  const holeMat = new THREE.MeshStandardMaterial({ color: 0x0a0d12, roughness: 0.9 });
  const r = 0.12;
  for (const sx of [-1, 1]) {
    for (const sz of [-1, 1]) {
      const h = new THREE.Mesh(new THREE.CylinderGeometry(r, r, thickness * 1.5, 16), holeMat);
      h.position.set(sx * (width / 2 - 0.5), 0, sz * (depth / 2 - 0.5));
      g.add(h);
    }
  }

  const traceMat = new THREE.MeshStandardMaterial({ color: 0xc8a25b, roughness: 0.3, metalness: 0.8 });
  const rows = Math.max(3, Math.round(depth / 1.2));
  for (let i = -rows; i <= rows; i++) {
    const t = box(width * 0.85, 0.005, 0.018, traceMat);
    t.position.set(0, thickness / 2 + 0.003, (i * depth) / (rows * 2 + 1));
    g.add(t);
  }

  g.userData.thickness = thickness;
  return g;
}
