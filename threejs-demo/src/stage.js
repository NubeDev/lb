// Renderer / scene / lights / camera / orbit controls. Nothing station-specific.

import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";

export function createStage(canvas) {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFSoftShadowMap;
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.35;

  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0x0b0d12);
  scene.fog = new THREE.Fog(0x0b0d12, 60, 140);

  const camera = new THREE.PerspectiveCamera(50, 1, 0.1, 300);
  camera.position.set(22, 14, 26);

  const controls = new OrbitControls(camera, canvas);
  controls.target.set(0, 2, 0);
  controls.enableDamping = true;
  controls.dampingFactor = 0.06;
  controls.maxPolarAngle = Math.PI * 0.49; // don't go under the ground
  controls.minDistance = 8;
  controls.maxDistance = 90;

  // night-time servo look: dim ambient, one warm key, cool fill
  scene.add(new THREE.HemisphereLight(0x9db4d6, 0x232733, 0.85));

  const key = new THREE.DirectionalLight(0xfff2dd, 1.4);
  key.position.set(18, 24, 12);
  key.castShadow = true;
  key.shadow.mapSize.set(2048, 2048);
  key.shadow.camera.left = -35;
  key.shadow.camera.right = 35;
  key.shadow.camera.top = 35;
  key.shadow.camera.bottom = -35;
  scene.add(key);

  const fill = new THREE.DirectionalLight(0x6f8fc9, 0.35);
  fill.position.set(-20, 10, -16);
  scene.add(fill);

  function resize() {
    const w = canvas.clientWidth, h = canvas.clientHeight;
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  }
  addEventListener("resize", resize);
  resize();

  // render loop with a per-frame hook (hotspot reprojection lives there)
  const onFrame = new Set();
  renderer.setAnimationLoop(() => {
    controls.update();
    for (const cb of onFrame) cb();
    renderer.render(scene, camera);
  });

  return { renderer, scene, camera, controls, onFrame };
}
