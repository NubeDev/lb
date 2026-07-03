// The ui-layout API client — one call per export, mirroring the gateway's `layout.*` routes and the
// host verbs 1:1 (data-studio scope v2). The record is MEMBER-OWNED: the workspace + user come from
// the session token (the hard wall, §7), never an argument — a caller can only ever touch their own
// layout for a surface. The UI never calls `invoke` directly; it goes through these named verbs.

import type { UiLayout } from "./layout.types";
import { invoke } from "@/lib/ipc/invoke";

/** Read the caller's OWN layout for `surface` (`model: null` when never saved). Mirrors `layout.get`. */
export function getLayout(surface: string): Promise<UiLayout> {
  return invoke<UiLayout>("layout_get", { surface });
}

/** Upsert the caller's OWN layout for `surface` (LWW; keyed to the token `sub`). Mirrors `layout.set`. */
export function setLayout(surface: string, model: unknown): Promise<UiLayout> {
  return invoke<UiLayout>("layout_set", { surface, model });
}
