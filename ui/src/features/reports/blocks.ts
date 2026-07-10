// Block-list operations for the report editor (reports scope). Pure array transforms over the ordered
// `blocks[]` — add / remove / move / patch — kept out of the editor component so the component stays
// presentational and each transform is unit-testable. One responsibility: immutable block-array edits.

import type { Block } from "@/lib/report";
import type { Cell } from "@/lib/dashboard";

/** A fresh empty markdown block. */
export function emptyMarkdown(): Block {
  return { kind: "markdown", body: "", pageBreak: false };
}

/** A panel block wrapping a renderable cell (an inline spec or a hydrated `panel:{id}` ref cell). */
export function panelBlock(cell: Cell): Block {
  return { kind: "panel", cell };
}

/** An image block referencing an uploaded asset. */
export function imageBlock(assetId: string, caption = ""): Block {
  return { kind: "image", assetId, caption };
}

/** Append a block. */
export function addBlock(blocks: Block[], block: Block): Block[] {
  return [...blocks, block];
}

/** Remove the block at `index`. */
export function removeBlock(blocks: Block[], index: number): Block[] {
  return blocks.filter((_, i) => i !== index);
}

/** Move the block at `index` by `delta` (−1 up, +1 down), clamped — the move-up/down reorder. */
export function moveBlock(blocks: Block[], index: number, delta: number): Block[] {
  const to = index + delta;
  if (to < 0 || to >= blocks.length) return blocks;
  const next = [...blocks];
  const [b] = next.splice(index, 1);
  next.splice(to, 0, b);
  return next;
}

/** Patch the block at `index` with a partial (body edit, pageBreak toggle, caption/width). */
export function patchBlock(blocks: Block[], index: number, patch: Partial<Block>): Block[] {
  return blocks.map((b, i) => (i === index ? { ...b, ...patch } : b));
}
