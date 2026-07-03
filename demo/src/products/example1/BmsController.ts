import * as THREE from "three";
import { Product, materials } from "../../core/Product";
import { box } from "../../components/box";
import { PCB } from "../../components/PCB";
import { Ethernet } from "../../components/Ethernet";
import { RS485 } from "../../components/RS485";
import { P1P2 } from "../../components/P1P2";
import { PowerSupply } from "../../components/PowerSupply";
import { USB } from "../../components/USBC";
import { CM4 } from "../../components/CM4";

/** A port sitting on one long face of the clamshell. */
interface FacePort {
  group: THREE.Group;
  x: number;
  width: number;
  label: string;
}

/**
 * LB-MINI-BMS — a DIN-rail clamshell building controller.
 *
 * Two PCBs are stacked inside a slim black enclosure. The bottom board's field
 * wiring (2× ETH, 2× RS485, P1P2, 24V, USB-C) exits the FRONT long face; the top
 * board's network + compute (ETH-1, CM4) exits the BACK long face. Each long face
 * is a bezel with real cutouts, so the ports poke through gaps rather than sitting
 * in front of a blank wall.
 */
export class BmsController extends Product {
  // Enclosure — slim DIN unit. Width is driven by the 7-port front row.
  readonly W = 14.0; // width  (X, the long axis / DIN length)
  readonly D = 6.0; //  depth  (Z, front-to-back)
  readonly H = 3.4; //  height (Y)
  readonly wall = 0.12; // panel thickness

  // The two board decks and where their ports exit the face vertically.
  readonly bottomPcbY = 0.85;
  readonly topPcbY = 2.35;
  readonly pcbThickness = 0.12;

  build(): void {
    this.buildBottomDeck();
    this.buildTopDeck();
    this.buildEnclosure();
  }

  /** Bottom PCB + field-wiring terminals exiting the FRONT (−Z) face. */
  private buildBottomDeck(): void {
    const pcbW = this.W - 1.0;
    const pcbD = this.D - 1.0;

    const pcb = PCB({ width: pcbW, depth: pcbD, thickness: this.pcbThickness });
    pcb.position.y = this.bottomPcbY;
    this.register(this.add(pcb), "PCB Bottom", { layers: 4, thickness: "1.2mm", finish: "ENIG" });

    // Ports laid left→right along the front face, facing −Z (out the front).
    const specs: { make: () => THREE.Group; label: string; width: number }[] = [
      { make: () => Ethernet({ label: "ETH-2" }), label: "ETH-2", width: 1.55 },
      { make: () => Ethernet({ label: "ETH-3" }), label: "ETH-3", width: 1.55 },
      { make: () => RS485({ label: "RS485-A" }), label: "RS485-A", width: 1.8 },
      { make: () => RS485({ label: "RS485-B" }), label: "RS485-B", width: 1.8 },
      { make: () => P1P2({ label: "P1P2" }), label: "P1P2", width: 1.6 },
      { make: () => PowerSupply({ label: "24V AC/DC" }), label: "24V AC/DC", width: 1.8 },
      { make: () => USB({ label: "USB-C" }), label: "USB-C", width: 0.9 },
    ];

    const ports = this.layoutRow(specs);
    for (const p of ports) {
      // Sit the connector on the bottom deck and rotate it to face out the front (−Z).
      // Push it forward so its opening sits just proud of the bezel, not recessed.
      p.group.position.set(p.x, this.bottomPcbY + this.pcbThickness / 2, -this.D / 2 + this.wall + 0.25);
      p.group.rotation.y = Math.PI; // components model their opening toward +Z; flip to −Z.
      const obj = this.add(p.group);
      this.register(obj, p.label, obj.userData.meta ?? {});
    }

    this.frontPorts = ports;
  }

  /** Top PCB + network/compute exiting the BACK (+Z) face. */
  private buildTopDeck(): void {
    const pcbW = this.W - 1.0;
    const pcbD = this.D - 1.0;

    const pcb = PCB({ width: pcbW, depth: pcbD, thickness: this.pcbThickness, color: 0x2a6e4a });
    pcb.position.y = this.topPcbY;
    this.register(this.add(pcb), "PCB Top", { layers: 4, thickness: "1.2mm", finish: "ENIG" });

    const deckY = this.topPcbY + this.pcbThickness / 2;

    // CM4 mezzanine sits flat on the top deck, roughly centered.
    const cm4 = CM4({ label: "CM4" });
    cm4.position.set(1.6, deckY, 0);
    const cm4Obj = this.add(cm4);
    this.register(cm4Obj, "CM4", cm4Obj.userData.meta ?? {});

    // ETH-1 exits the back face (+Z), facing +Z (its native orientation).
    const eth = Ethernet({ label: "ETH-1" });
    const ethX = -3.2;
    eth.position.set(ethX, deckY, this.D / 2 - this.wall - 0.25);
    const ethObj = this.add(eth);
    this.register(ethObj, "ETH-1", ethObj.userData.meta ?? {});

    this.backPorts = [{ group: eth, x: ethX, width: 1.55, label: "ETH-1" }];
  }

  /**
   * Slim black clamshell: base, vented lid, two solid end caps, and two long
   * faces built as bezels with cutouts where the ports pass through.
   */
  private buildEnclosure(): void {
    const black = materials.black();
    const W = this.W;
    const D = this.D;
    const H = this.H;
    const t = this.wall;

    const base = box(W, t, D, black);
    base.position.y = t / 2;
    this.register(this.add(base), "Enclosure Base", {
      material: "anodized aluminum",
      width: `${W}cm`,
      depth: `${D}cm`,
      height: `${H}cm`,
      mount: "DIN rail",
    });

    this.buildLid(black);

    // Solid short end caps (left/right).
    for (const sx of [-1, 1]) {
      const cap = box(t, H, D, black);
      cap.position.set(sx * (W / 2 - t / 2), H / 2, 0);
      this.register(this.add(cap), sx < 0 ? "End Cap L" : "End Cap R", { material: "aluminum" });
    }

    // Long faces with port cutouts.
    this.buildBezelFace(-1, this.frontPorts, this.bottomPcbY, "Front Bezel", black);
    this.buildBezelFace(1, this.backPorts, this.topPcbY, "Back Bezel", black);

    // DIN clip on the back, below the bezel.
    this.buildDinClip(black);
  }

  /** Vented top lid: a plate with a row of slot cutouts (built as slats). */
  private buildLid(mat: THREE.Material): void {
    const W = this.W;
    const D = this.D;
    const t = this.wall;
    const y = this.H - t / 2;

    // Front and back rails of the lid (solid margins).
    const margin = 1.1;
    for (const sz of [-1, 1]) {
      const rail = box(W, t, margin, mat);
      rail.position.set(0, y, sz * (D / 2 - margin / 2));
      this.add(rail);
    }
    // Center strip carrying vent slats between the rails.
    const slatZ = D - margin * 2;
    const slatCount = 9;
    const gap = 0.18;
    const slatW = (W - gap * (slatCount + 1)) / slatCount;
    for (let i = 0; i < slatCount; i++) {
      const slat = box(slatW, t, slatZ, mat);
      const x = -W / 2 + gap + slatW / 2 + i * (slatW + gap);
      slat.position.set(x, y, 0);
      this.add(slat);
    }
    // Register one lid slat as the toggle handle for the whole lid group is overkill;
    // instead register a thin full-footprint marker so "Lid" appears selectable.
    const marker = box(W, t * 0.4, D, mat);
    marker.position.set(0, y + t * 0.4, 0);
    marker.visible = false; // invisible pick proxy kept out of the way
    this.register(this.add(marker), "Lid (vented)", { openings: `${slatCount} vent slots` });
  }

  /**
   * One long face as a bezel: a bottom rail below the port row, a top rail above
   * it, and short pillars between adjacent ports. Ports show through the gaps.
   *
   * @param sz    -1 = front (−Z), +1 = back (+Z)
   * @param ports the ports that pierce this face (already positioned)
   * @param deckY the PCB deck height whose ports exit here
   */
  private buildBezelFace(sz: number, ports: FacePort[], deckY: number, label: string, mat: THREE.Material): void {
    const W = this.W;
    const D = this.D;
    const H = this.H;
    const t = this.wall;
    const z = sz * (D / 2 - t / 2);

    // The port "window" spans this vertical band; rails fill above and below it.
    const portBottom = deckY - 0.1;
    const portTop = deckY + 1.3;

    // Bottom rail (floor of enclosure → bottom of port window).
    const lowerH = portBottom;
    if (lowerH > 0.02) {
      const lower = box(W, lowerH, t, mat);
      lower.position.set(0, lowerH / 2, z);
      this.add(lower);
    }
    // Top rail (top of port window → lid).
    const upperH = H - portTop;
    if (upperH > 0.02) {
      const upper = box(W, upperH, t, mat);
      upper.position.set(0, portTop + upperH / 2, z);
      this.add(upper);
    }

    // Pillars between ports (and at the two ends) fill the window's leftover width.
    const windowH = portTop - portBottom;
    const edges: number[] = [-W / 2 + t]; // start just inside the left cap
    const sorted = [...ports].sort((a, b) => a.x - b.x);
    for (const p of sorted) {
      edges.push(p.x - p.width / 2 - 0.06);
      edges.push(p.x + p.width / 2 + 0.06);
    }
    edges.push(W / 2 - t); // end just inside the right cap

    // edges come in pairs [gapStart, portStart, portEnd, gapStart, ...]; fill the
    // solid spans (even→odd index pairs) with pillars.
    for (let i = 0; i + 1 < edges.length; i += 2) {
      const a = edges[i];
      const b = edges[i + 1];
      const w = b - a;
      if (w > 0.04) {
        const pillar = box(w, windowH, t, mat);
        pillar.position.set((a + b) / 2, portBottom + windowH / 2, z);
        this.add(pillar);
      }
    }

    // A selectable marker for the whole face.
    const marker = box(W - t * 2, 0.02, t, mat);
    marker.position.set(0, H - 0.4, z);
    marker.visible = false;
    this.register(this.add(marker), label, { openings: `${ports.length} port cutouts` });
  }

  /** Simple DIN-rail clip on the back underside. */
  private buildDinClip(mat: THREE.Material): void {
    const clip = box(this.W * 0.5, 0.5, 0.3, mat);
    clip.position.set(0, 0.25, this.D / 2 + 0.15);
    this.register(this.add(clip), "DIN Clip", { standard: "DIN 35mm rail" });
  }

  /**
   * Distribute a list of port specs evenly across the usable front-face width,
   * instantiating each and returning its placement. Centers the whole row.
   */
  private layoutRow(specs: { make: () => THREE.Group; label: string; width: number }[]): FacePort[] {
    const usable = this.W - this.wall * 2 - 0.6; // inset from both end caps
    const totalW = specs.reduce((s, p) => s + p.width, 0);
    const gaps = specs.length + 1;
    const gap = Math.max(0.15, (usable - totalW) / gaps);

    const ports: FacePort[] = [];
    let cursor = -usable / 2 + gap;
    for (const spec of specs) {
      const x = cursor + spec.width / 2;
      ports.push({ group: spec.make(), x, width: spec.width, label: spec.label });
      cursor += spec.width + gap;
    }
    return ports;
  }

  private frontPorts: FacePort[] = [];
  private backPorts: FacePort[] = [];
}
