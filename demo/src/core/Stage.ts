import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { Product } from "./Product";

export interface SelectionListener {
  (part: { label: string; meta: Record<string, unknown> } | null): void;
}

/** Three.js renderer + scene + lights + camera + orbit controls.  */
export class Stage {
  readonly scene = new THREE.Scene();
  readonly renderer: THREE.WebGLRenderer;
  readonly camera: THREE.PerspectiveCamera;
  readonly controls: OrbitControls;
  readonly root = new THREE.Group();
  private raf = 0;

  onSelection: SelectionListener | null = null;
  private raycaster = new THREE.Raycaster();
  private pointer = new THREE.Vector2();
  private current: Product | null = null;
  private clock = new THREE.Clock();
  private readonly panelWidth: number;

  constructor(canvas: HTMLCanvasElement, panelWidth = 280) {
    this.panelWidth = panelWidth;
    this.scene.background = new THREE.Color(0x0b0d12);
    this.scene.fog = new THREE.Fog(0x0b0d12, 20, 70);

    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;

    this.camera = new THREE.PerspectiveCamera(45, 1, 0.05, 200);
    this.camera.position.set(14, 9, 14);

    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.08;
    this.controls.minDistance = 4;
    this.controls.maxDistance = 40;
    this.controls.maxPolarAngle = Math.PI * 0.49;

    this.scene.add(this.root);

    addLights(this.scene);
    addGround(this.scene);

    window.addEventListener("resize", () => this.resize());
    canvas.addEventListener("pointerdown", () => {
      canvas.addEventListener("pointerup", this.onPointerUpOnce, { once: true });
    });
    this.resize();
  }

  setProduct(product: Product): void {
    if (this.current) this.root.remove(this.current.group);
    this.current = product;
    this.root.add(product.group);
    this.controls.target.copy(product.focus);
    this.camera.position.copy(product.cam).add(product.focus);
    this.controls.update();
  }

  start(): void {
    const tick = () => {
      const t = this.clock.getElapsedTime();
      if (this.current) this.current.frame(t);
      this.controls.update();
      this.renderer.render(this.scene, this.camera);
      this.raf = requestAnimationFrame(tick);
    };
    tick();
  }

  stop(): void {
    cancelAnimationFrame(this.raf);
  }

  private resize(): void {
    const w = window.innerWidth - this.panelWidth;
    const h = window.innerHeight;
    this.renderer.setSize(w, h, false);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }

  private readonly onPointerUpOnce = (ev: PointerEvent) => {
    const rect = this.renderer.domElement.getBoundingClientRect();
    this.pointer.x = ((ev.clientX - rect.left) / rect.width) * 2 - 1;
    this.pointer.y = -((ev.clientY - rect.top) / rect.height) * 2 + 1;
    this.raycaster.setFromCamera(this.pointer, this.camera);
    if (!this.current) return;
    const meshes: THREE.Object3D[] = [];
    for (const p of this.current.parts)
      if (p.object.visible) collect(p.object, meshes);
    const hits = this.raycaster.intersectObjects(meshes, true);
    if (hits.length === 0) {
      this.onSelection?.(null);
      return;
    }
    const owner = resolvePart(hits[0].object, this.current.parts);
    if (owner) {
      this.onSelection?.({ label: owner.label, meta: owner.meta });
    } else {
      this.onSelection?.(null);
    }
  };
}

function addLights(scene: THREE.Scene): void {
  scene.add(new THREE.HemisphereLight(0x9bb8ff, 0x202028, 0.55));
  const key = new THREE.DirectionalLight(0xffffff, 1.4);
  key.position.set(8, 12, 6);
  key.castShadow = true;
  key.shadow.mapSize.set(2048, 2048);
  key.shadow.camera.near = 1;
  key.shadow.camera.far = 40;
  key.shadow.camera.left = -12;
  key.shadow.camera.right = 12;
  key.shadow.camera.top = 12;
  key.shadow.camera.bottom = -12;
  scene.add(key);
  const fill = new THREE.DirectionalLight(0x88aaff, 0.45);
  fill.position.set(-6, 4, -6);
  scene.add(fill);
  const rim = new THREE.DirectionalLight(0xff9ad0, 0.25);
  rim.position.set(0, 6, -10);
  scene.add(rim);
}

function addGround(scene: THREE.Scene): void {
  const grid = new THREE.GridHelper(40, 40, 0x2a3242, 0x1a1f2a);
  grid.position.y = -0.01;
  scene.add(grid);
  const plane = new THREE.Mesh(
    new THREE.PlaneGeometry(40, 40),
    new THREE.MeshStandardMaterial({ color: 0x10141c, roughness: 1, metalness: 0 }),
  );
  plane.rotation.x = -Math.PI / 2;
  plane.receiveShadow = true;
  scene.add(plane);
}

function collect(o: THREE.Object3D, out: THREE.Object3D[]): void {
  o.traverse((c) => {
    if ((c as THREE.Mesh).isMesh) {
      c.castShadow = true;
      c.receiveShadow = true;
      out.push(c);
    }
  });
}

function resolvePart(
  hit: THREE.Object3D,
  parts: { object: THREE.Object3D; label: string; meta: Record<string, unknown> }[],
) {
  let o: THREE.Object3D | null = hit;
  while (o) {
    const found = parts.find((p) => p.object === o);
    if (found) return found;
    o = o.parent;
  }
  return null;
}
