// The assets API client — one call per export, mirroring the Rust asset verbs
// (`host::put_doc`, `host::get_doc`, `host::share_doc`, …) and the node command names
// one-to-one. The UI never calls `invoke` directly; it goes through these named verbs
// (FILE-LAYOUT frontend rules). These map to the `assets.*` MCP tool surface on the node.
//
// `author` is the caller's principal. The REAL node derives the principal from the session
// token and ignores this arg (the gateway demo-principal is a STATUS.md open question); the
// in-memory fake uses it to resolve the membership gate so the UI's allow/deny paths are
// exercised in tests exactly as the node would.

import type { Doc, Skill } from "./assets.types";
import { invoke } from "@/lib/ipc/invoke";

/** Create/update a doc owned by the caller in workspace `ws`. Mirrors `lb_host::put_doc`. */
export function putDoc(
  ws: string,
  id: string,
  title: string,
  content: string,
  ts: number,
  author?: string,
): Promise<Doc> {
  return invoke<Doc>("assets_put_doc", { ws, id, title, content, ts, author });
}

/** Read a doc by id (capability + membership checked on the node). Mirrors `lb_host::get_doc`. */
export function getDoc(ws: string, id: string, author?: string): Promise<Doc> {
  return invoke<Doc>("assets_get_doc", { ws, id, author });
}

/** List the caller's own docs in `ws`. Mirrors `lb_host::list_docs`. */
export function listDocs(ws: string, author?: string): Promise<Doc[]> {
  return invoke<Doc[]>("assets_list_docs", { ws, author });
}

/** Share a doc to a team (owner only). Mirrors `lb_host::share_doc`. */
export function shareDoc(ws: string, id: string, team: string): Promise<void> {
  return invoke<void>("assets_share_doc", { ws, id, team });
}

/** Link a doc into a channel (owner only). Mirrors `lb_host::link_doc`. */
export function linkDoc(ws: string, id: string, channel: string): Promise<void> {
  return invoke<void>("assets_link_doc", { ws, id, channel });
}

/** Load a granted skill (latest, or a pinned version). Mirrors `lb_host::load_skill`. */
export function loadSkill(ws: string, id: string, version?: string): Promise<Skill> {
  return invoke<Skill>("assets_load_skill", { ws, id, version });
}
