import type { Product } from "../core/Product";
import { BmsController } from "../products/example1/BmsController";

export type ProductFactory = () => Product;

export interface ProductEntry {
  name: string;
  description: string;
  factory: ProductFactory;
}

export const productIndex: ProductEntry[] = [
  {
    name: "LB-MINI-BMS",
    description: "Dual-PCB BMS controller in black enclosure: 2× ETH, 2× RS485, P1P2, 24V AC/DC, USB-C, CM4 with WiFi",
    factory: () => new BmsController({ name: "LB-MINI-BMS" }),
  },
];

export function createProduct(name: string): Product | null {
  const entry = productIndex.find((e) => e.name === name);
  return entry ? entry.factory() : null;
}