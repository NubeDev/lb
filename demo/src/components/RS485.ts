import * as THREE from "three";
import { box } from "./box";
import { materials } from "../core/Product";

export interface Rs485Options {
  label?: string;
}

/** Pluggable green terminal block, 4 pins, RS-485 A/B + aux. */
export function RS485(opts: Rs485Options = {}): THREE.Group {
  const label = opts.label ?? "RS485";
  const g = new THREE.Group();
  g.name = label;

  const housing = box(1.8, 0.95, 1.0, materials.plastic(0x2e7d4f));
  housing.position.y = 0.47;
  g.add(housing);

  const divMat = new THREE.MeshStandardMaterial({ color: 0x1a3a25, roughness: 0.5 });
  for (let i = -1; i <= 1; i++) {
    const div = box(0.06, 0.6, 1.0, divMat);
    div.position.set(i * 0.55, 0.95, 0);
    g.add(div);
  }

  for (let i = 0; i < 4; i++) {
    const pin = box(0.2, 0.3, 0.3, materials.pad());
    pin.position.set(-0.65 + i * 0.42, 0.45, 0.35);
    g.add(pin);
  }

  g.userData.label = label;
  g.userData.meta = { connector: "pluggable terminal", channels: 1 };
  return g;
}
