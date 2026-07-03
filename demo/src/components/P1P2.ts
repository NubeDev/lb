import * as THREE from "three";
import { box } from "./box";
import { materials } from "../core/Product";

export interface P1P2Options {
  label?: string;
}

/** BACnet MS/TP P1P2 connector — 3-pin pluggable terminal, orange housing. */
export function P1P2(opts: P1P2Options = {}): THREE.Group {
  const label = opts.label ?? "P1P2";
  const g = new THREE.Group();
  g.name = label;

  const housing = box(1.6, 0.95, 1.0, materials.plastic(0xd4721a));
  housing.position.y = 0.47;
  g.add(housing);

  const divMat = new THREE.MeshStandardMaterial({ color: 0x6e380c, roughness: 0.5 });
  for (let i = -1; i <= 1; i += 2) {
    const div = box(0.06, 0.6, 1.0, divMat);
    div.position.set(i * 0.45, 0.95, 0);
    g.add(div);
  }

  for (let i = 0; i < 3; i++) {
    const pin = box(0.22, 0.3, 0.3, materials.pad());
    pin.position.set(-0.48 + i * 0.48, 0.45, 0.35);
    g.add(pin);
  }

  g.userData.label = label;
  g.userData.meta = { connector: "pluggable terminal 3-pin", protocol: "BACnet MS/TP" };
  return g;
}