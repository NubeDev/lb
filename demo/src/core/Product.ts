import * as THREE from "three";

export interface ProductPart {
  object: THREE.Object3D;
  label: string;
  meta: Record<string, unknown>;
  rest: THREE.Vector3;
  explode: THREE.Vector3;
}

export interface ProductOptions {
  name: string;
  focus?: THREE.Vector3;
  cam?: THREE.Vector3;
}

export abstract class Product {
  readonly group = new THREE.Group();
  readonly parts: ProductPart[] = [];
  readonly focus: THREE.Vector3;
  readonly cam: THREE.Vector3;

  constructor(opts: ProductOptions) {
    this.focus = opts.focus ?? new THREE.Vector3(0, 1, 0);
    this.cam = opts.cam ?? new THREE.Vector3(14, 9, 14);
    this.group.name = String(opts.name);
  }

  abstract build(): void;

  /** Add a sub-object to the visible group and store its "rest" transform. */
  protected add(object: THREE.Object3D, rest = object.position.clone()): THREE.Object3D {
    object.position.copy(rest);
    object.userData.rest = rest.clone();
    this.group.add(object);
    return object;
  }

  /** Register a part as selectable + explodable. Metadata shown in the panel. */
  protected register(
    object: THREE.Object3D,
    label: string,
    meta: Record<string, unknown> = {},
    explode = new THREE.Vector3(0, restY(object) * 2.5 + 1, 0),
  ): THREE.Object3D {
    object.name = label;
    object.userData.label = label;
    object.userData.meta = meta;
    void this.parts.push({ object, label, meta, rest: restFrom(object), explode });
    void explode; // eslintSuppress unused
    return object;
  }

  applyExplode(amount: number): void {
    for (const p of this.parts) {
      p.object.position.copy(p.rest).lerp(p.explode, amount);
    }
  }

  setVisible(predicate: (p: ProductPart) => boolean): void {
    for (const p of this.parts) p.object.visible = predicate(p);
  }

  frame(_t: number): void {}
}

function restFrom(o: THREE.Object3D): THREE.Vector3 {
  return (o.userData.rest as THREE.Vector3) ?? new THREE.Vector3();
}
function restY(o: THREE.Object3D): number {
  return restFrom(o).y;
}

/** Material library reused across every product / component. */
export const materials = {
  pcb: () => new THREE.MeshStandardMaterial({ color: 0x1b6e3a, roughness: 0.7, metalness: 0.1 }),
  pad: () => new THREE.MeshStandardMaterial({ color: 0xc8a25b, roughness: 0.4, metalness: 0.6 }),
  steel: () => new THREE.MeshStandardMaterial({ color: 0x80848c, roughness: 0.35, metalness: 0.9 }),
  darkSteel: () => new THREE.MeshStandardMaterial({ color: 0x4a4e57, roughness: 0.5, metalness: 0.85 }),
  plastic: (c = 0x20242c) => new THREE.MeshStandardMaterial({ color: c, roughness: 0.6, metalness: 0.05 }),
  ledGreen: () => new THREE.MeshStandardMaterial({ color: 0x00ff66, emissive: 0x00a040, emissiveIntensity: 1.4 }),
  ledRed: () => new THREE.MeshStandardMaterial({ color: 0xff3a3a, emissive: 0xa02020, emissiveIntensity: 1.2 }),
  black: () => new THREE.MeshStandardMaterial({ color: 0x0e1116, roughness: 0.45, metalness: 0.7 }),
};
