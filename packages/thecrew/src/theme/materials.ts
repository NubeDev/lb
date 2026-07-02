// tokens → three.js materials, in ONE place (look-scope.md §lighting-recipe).
// Shape components never `new Material()` — they ask here, so status emissives, hover
// edge highlight, and the selective-bloom story stay consistent scene-wide.
//
// Selective bloom (look-scope §glow): the EffectComposer's Bloom runs with
// luminanceThreshold ≥ 1, so ONLY materials with emissiveIntensity > 1 glow —
// status emissives and the selection halo. Bodies never cross the threshold.

import * as THREE from "three";
import { tokens } from "./tokens";

export type StatusKind = "running" | "stopped" | "fault" | "override";

// ---- shared (cached) materials ---------------------------------------------------

const cache = new Map<string, THREE.Material>();

function cached<T extends THREE.Material>(key: string, make: () => T): T {
  let m = cache.get(key);
  if (!m) {
    m = make();
    cache.set(key, m);
  }
  return m as T;
}

/** Matte PBR equipment body — desaturated steel, never saturated (look-scope). */
export function bodyMaterial(): THREE.MeshStandardMaterial {
  return cached(
    "body",
    () =>
      new THREE.MeshStandardMaterial({
        color: tokens.color.steel,
        roughness: 0.85,
        metalness: 0.15,
      }),
  );
}

/** Slightly-darker inset faces (fan hubs, damper frames, filter recesses). */
export function bodyDarkMaterial(): THREE.MeshStandardMaterial {
  return cached(
    "bodyDark",
    () =>
      new THREE.MeshStandardMaterial({
        color: new THREE.Color(tokens.color.steel).multiplyScalar(0.55),
        roughness: 0.9,
        metalness: 0.1,
      }),
  );
}

/** Duct body — slightly lighter than the canvas background (look-scope §recipe). */
export function ductBodyMaterial(): THREE.MeshStandardMaterial {
  return cached(
    "duct",
    () =>
      new THREE.MeshStandardMaterial({
        color: tokens.color.duct,
        roughness: 0.9,
        metalness: 0.05,
      }),
  );
}

/** Status emissive — the ONLY saturated color on equipment. `running`/`fault`/
 * `override` push past the bloom threshold; `stopped` stays dim (no glow). */
export function statusMaterial(status: StatusKind): THREE.MeshStandardMaterial {
  return cached(`status:${status}`, () => {
    const color = tokens.color.status[status];
    const glows = status !== "stopped";
    return new THREE.MeshStandardMaterial({
      color,
      emissive: color,
      emissiveIntensity: glows ? 2 : 0.15,
      roughness: 0.4,
      metalness: 0,
      toneMapped: false,
    });
  });
}

/** Selection halo — accent emissive above the bloom threshold, translucent. */
export function haloMaterial(): THREE.MeshStandardMaterial {
  return cached(
    "halo",
    () =>
      new THREE.MeshStandardMaterial({
        color: tokens.color.accent,
        emissive: tokens.color.accent,
        emissiveIntensity: 1.6,
        transparent: true,
        opacity: 0.85,
        toneMapped: false,
        depthWrite: false,
      }),
  );
}

/** Hover edge highlight — subtle, below the bloom threshold. */
export function hoverMaterial(): THREE.MeshStandardMaterial {
  return cached(
    "hover",
    () =>
      new THREE.MeshStandardMaterial({
        color: tokens.color.accent,
        emissive: tokens.color.accent,
        emissiveIntensity: 0.35,
        transparent: true,
        opacity: 0.5,
        depthWrite: false,
      }),
  );
}

/** Ghost preview during drag-place — translucent accent, never blooms. */
export function ghostMaterial(): THREE.MeshStandardMaterial {
  return cached(
    "ghost",
    () =>
      new THREE.MeshStandardMaterial({
        color: tokens.color.accent,
        transparent: true,
        opacity: 0.3,
        depthWrite: false,
      }),
  );
}

export type Medium = keyof typeof tokens.color.medium; // "air" | "chw" | "hw"

export function mediumColor(medium: string): string {
  return tokens.color.medium[medium as Medium] ?? tokens.color.medium.air;
}

// ---- per-instance materials (animated) --------------------------------------------

/** Scrolling chevron texture for duct flow — one canvas texture per duct instance;
 * the Duct component scrolls `texture.offset.x` at bound flow speed (useFrame).
 * Per-instance because each duct scrolls at its own bound speed. */
export function ductFlowMaterial(medium: string): {
  material: THREE.MeshBasicMaterial;
  texture: THREE.CanvasTexture;
} {
  const canvas = document.createElement("canvas");
  canvas.width = 64;
  canvas.height = 64;
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    // no 2D canvas (headless test env): untextured but valid material
    const texture = new THREE.CanvasTexture(canvas);
    return {
      material: new THREE.MeshBasicMaterial({ transparent: true, opacity: 0 }),
      texture,
    };
  }
  ctx.clearRect(0, 0, 64, 64);
  ctx.strokeStyle = mediumColor(medium);
  ctx.lineWidth = 7;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  // one chevron pointing +x, vertically centered
  ctx.beginPath();
  ctx.moveTo(18, 12);
  ctx.lineTo(42, 32);
  ctx.lineTo(18, 52);
  ctx.stroke();
  const texture = new THREE.CanvasTexture(canvas);
  texture.wrapS = THREE.RepeatWrapping;
  texture.wrapT = THREE.ClampToEdgeWrapping;
  const material = new THREE.MeshBasicMaterial({
    map: texture,
    transparent: true,
    opacity: 0.75,
    depthWrite: false,
    toneMapped: false,
  });
  return { material, texture };
}

// ---- scene-wide animation tick -----------------------------------------------------

/** Called once per frame from SceneCanvas: pulses the fault emissive at ~0.5 Hz
 * (calm, not alarm-strobe — look-scope §motion). All faults pulse in sync, which
 * reads as intentional. Reduced motion is respected by not calling this. */
export function updateMaterials(tSec: number): void {
  const fault = cache.get("status:fault") as THREE.MeshStandardMaterial | undefined;
  if (fault) {
    const phase = Math.sin(tSec * Math.PI * 2 * tokens.motion.faultPulseHz);
    fault.emissiveIntensity = 1.8 + phase * 0.7; // 1.1 – 2.5, always ≥ dim glow
  }
}
