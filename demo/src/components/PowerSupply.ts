import * as THREE from "three";
import { box } from "./box";
import { materials } from "../core/Product";

export interface PowerSupplyOptions {
  label?: string;
}

/** 24V AC/DC power input — 3-pin terminal block, grey industrial housing. */
export function PowerSupply(opts: PowerSupplyOptions = {}): THREE.Group {
  const label = opts.label ?? "24V";
  const g = new THREE.Group();
  g.name = label;

  const housing = box(1.8, 1.05, 1.0, materials.plastic(0x4a5058));
  housing.position.y = 0.52;
  g.add(housing);

  const divMat = new THREE.MeshStandardMaterial({ color: 0x2a3035, roughness: 0.5 });
  for (let i = -1; i <= 1; i += 2) {
    const div = box(0.06, 0.65, 1.0, divMat);
    div.position.set(i * 0.52, 1.05, 0);
    g.add(div);
  }

  const pinLabels = ["L/24V", "N/GND", "PE"];
  for (let i = 0; i < 3; i++) {
    const pin = box(0.25, 0.32, 0.3, materials.pad());
    pin.position.set(-0.55 + i * 0.55, 0.48, 0.35);
    g.add(pin);
  }

  const led = new THREE.Mesh(
    new THREE.CylinderGeometry(0.07, 0.07, 0.06, 8),
    materials.ledGreen(),
  );
  led.position.set(0.68, 1.08, 0.45);
  led.rotation.x = Math.PI / 2;
  g.add(led);

  g.userData.label = label;
  g.userData.meta = { connector: "terminal 3-pin", input: "24V AC/DC", pins: pinLabels.join("/") };
  return g;
}