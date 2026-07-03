import * as THREE from "three";
import { box } from "./box";
import { materials } from "../core/Product";

export interface CM4Options {
  label?: string;
}

/** Raspberry Pi Compute Module 4 on a DIP mezzanine board with WiFi antenna. */
export function CM4(opts: CM4Options = {}): THREE.Group {
  const label = opts.label ?? "CM4";
  const g = new THREE.Group();
  g.name = label;

  const carrier = box(4.5, 0.12, 3.2, materials.pcb());
  carrier.position.y = 0.06;
  g.add(carrier);

  const module = box(3.2, 0.1, 2.8, new THREE.MeshStandardMaterial({ color: 0x2c4e6e, roughness: 0.55, metalness: 0.4 }));
  module.position.set(0, 0.17, 0);
  g.add(module);

  const soc = box(1.4, 0.08, 1.4, new THREE.MeshStandardMaterial({ color: 0x1a1a1a, roughness: 0.3, metalness: 0.7 }));
  soc.position.set(-0.6, 0.26, 0.3);
  g.add(soc);

  const ram = box(0.9, 0.05, 0.7, new THREE.MeshStandardMaterial({ color: 0x222222, roughness: 0.3, metalness: 0.65 }));
  ram.position.set(0.8, 0.24, -0.4);
  g.add(ram);

  const cpuLabel = box(0.8, 0.02, 0.8, new THREE.MeshStandardMaterial({ color: 0xb8c0c8, roughness: 0.5, metalness: 0.2 }));
  cpuLabel.position.set(-0.6, 0.31, 0.3);
  g.add(cpuLabel);

  const antennaTrace = box(2.8, 0.01, 0.08, materials.pad());
  antennaTrace.position.set(0, 0.18, 1.35);
  g.add(antennaTrace);

  const antennaChip = box(0.3, 0.06, 0.2, new THREE.MeshStandardMaterial({ color: 0x3a4048, roughness: 0.4, metalness: 0.5 }));
  antennaChip.position.set(0, 0.21, 1.45);
  g.add(antennaChip);

  g.userData.label = label;
  g.userData.meta = {
    module: "Raspberry Pi CM4",
    soc: "BCM2711",
    wireless: "WiFi 5 + BLE 5.0",
    form: "DIP mezzanine",
  };
  return g;
}