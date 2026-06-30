// The node-icon resolver — maps a descriptor's `icon` (a lucide icon name) to a component, with a
// fallback by `NodeKind` so every node shows a glyph even when a descriptor carries no icon. Pure
// presentation; one place owns the icon set so the palette, the rail, and the canvas node agree.

import {
  ArrowDownToLine,
  Boxes,
  Code,
  GitBranch,
  Hash,
  Radio,
  Wrench,
  Zap,
  type LucideIcon,
} from "lucide-react";

import type { NodeDescriptor, NodeKind } from "@/lib/flows";

/** Named icons the palette/node understand. A descriptor's `icon` must be one of these keys. */
const ICONS: Record<string, LucideIcon> = {
  zap: Zap,
  wrench: Wrench,
  code: Code,
  hash: Hash,
  "git-branch": GitBranch,
  "arrow-down-to-line": ArrowDownToLine,
};

/** The kind fallback when a descriptor declares no icon. */
const KIND_FALLBACK: Record<NodeKind, LucideIcon> = {
  trigger: Radio,
  transform: Boxes,
  sink: ArrowDownToLine,
  source: Radio,
};

/** Resolve the icon component for a descriptor: its named icon, else its kind fallback. */
export function nodeIcon(desc: Pick<NodeDescriptor, "icon" | "kind">): LucideIcon {
  if (desc.icon && ICONS[desc.icon]) return ICONS[desc.icon];
  return KIND_FALLBACK[desc.kind] ?? Boxes;
}
