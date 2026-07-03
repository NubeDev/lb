import * as THREE from "three";

/** Box mesh with shadows enabled, the only geometry primitive the demo needs. */
export function box(
  w: number,
  h: number,
  d: number,
  material: THREE.Material,
): THREE.Mesh {
  const m = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), material);
  m.castShadow = true;
  m.receiveShadow = true;
  return m;
}

/** Mount a component onto a PCB-top height. */
export function onPcbTop(group: THREE.Group, pcbThickness: number, height: number): void {
  group.position.y = pcbThickness / 2 + height / 2;
}
