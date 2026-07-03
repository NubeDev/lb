import * as THREE from "three";
import { box } from "./box";
import { materials } from "../core/Product";

export interface EthernetOptions {
  label?: string;
  /** true = 100 Mb/s (red link LED); false = 1 Gb/s (green). */
  speed100?: boolean;
}

/** Magjack-style RJ45 port with two activity LEDs. */
export function Ethernet(opts: EthernetOptions = {}): THREE.Group {
  const label = opts.label ?? "ETH";
  const speed100 = opts.speed100 ?? false;

  const g = new THREE.Group();
  g.name = label;

  const housing = box(1.55, 1.4, 1.35, materials.darkSteel());
  housing.position.y = 0.7;
  g.add(housing);

  const dark = new THREE.MeshStandardMaterial({ color: 0x0a0d12, roughness: 0.9 });
  const opening = box(1.2, 0.85, 0.12, dark);
  opening.position.set(0, 0.78, 0.62);
  g.add(opening);

  const notch = box(1.2, 0.14, 0.05, dark);
  notch.position.set(0, 0.42, 0.66);
  g.add(notch);

  const ledMat = speed100 ? materials.ledRed() : materials.ledGreen();
  const link = new THREE.Mesh(new THREE.BoxGeometry(0.08, 0.05, 0.04), ledMat);
  link.position.set(0.6, 1.42, 0.5);
  g.add(link);

  const actMat = materials.ledGreen();
  const act = new THREE.Mesh(new THREE.BoxGeometry(0.08, 0.05, 0.04), actMat);
  act.position.set(-0.6, 1.42, 0.5);
  g.add(act);

  g.userData.label = label;
  g.userData.meta = { connector: "RJ45 magjack", speed: speed100 ? "100 Mb/s" : "1 Gb/s" };
  return g;
}
