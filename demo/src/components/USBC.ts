import * as THREE from "three";
import { box } from "./box";
import { materials } from "../core/Product";

export interface USBOptions {
  label?: string;
}

/** USB-C receptacle — compact metal-shrouded port with oval cutout. */
export function USB(opts: USBOptions = {}): THREE.Group {
  const label = opts.label ?? "USB-C";
  const g = new THREE.Group();
  g.name = label;

  const shield = box(0.9, 0.45, 1.1, materials.darkSteel());
  shield.position.y = 0.22;
  g.add(shield);

  const dark = new THREE.MeshStandardMaterial({ color: 0x080a10, roughness: 0.9 });
  const opening = box(0.55, 0.18, 0.08, dark);
  opening.position.set(0, 0.28, 0.53);
  g.add(opening);

  const tongue = box(0.4, 0.06, 0.55, materials.plastic(0xaab0b8));
  tongue.position.set(0, 0.25, 0.22);
  g.add(tongue);

  g.userData.label = label;
  g.userData.meta = { connector: "USB-C receptacle", role: "device/OTG" };
  return g;
}