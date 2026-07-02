// Scene persistence over the host-mediated bridge — the ONLY place thecrew reads/writes the
// document store. Scenes are workspace docs (content_type JSON), reached through the shipped
// `assets.get_doc` / `assets.put_doc` / `assets.list_docs` verbs (zero core additions —
// thecrew-extension-scope.md §MCP surface). Every call crosses the bridge; the host resolves
// the workspace from the signed token and re-checks the cap, workspace-first.
//
// Discovery convention (parent scope Open question 3, resolved): scene docs use the id PREFIX
// `scene:` AND the tag `scene`. The prefix is what the PICKER filters on, because the shipped
// `assets.list_docs` returns only `{id, title}` per doc — it does NOT return tags (verified in
// crates/host/src/assets/tool.rs `list_docs`), so a tag-side filter is impossible without a core
// change. We still tag `scene` so a future tag-returning list can filter server-side. See the
// session doc's findings.
//
// Save concurrency is LAST-WRITER-WINS today: `assets.put_doc` has no revision check (verified in
// crates/host/src/assets/put_doc.rs) — the real fix is a generic document-store revision ask
// (parent scope Open question 1's finding). The interim here (thecrew-extension-scope.md §Risks):
// a whole-doc read-before-write content compare against the snapshot the editor loaded, surfacing
// an honest "scene changed underneath you" conflict instead of silently clobbering. We do NOT fork
// or extend put_doc.

import type { Bridge, WidgetBridge } from "./contract";
import type { SceneDoc } from "../scene/scene.types";
import { validateScene } from "../scene/validate";

/** The doc-id prefix + tag every scene carries. `list_docs` filters on the prefix (no tag in the
 *  list output); the tag rides along for a future tag-returning list. */
export const SCENE_PREFIX = "scene:";
export const SCENE_TAG = "scene";

/** A scene as the picker sees it (from `list_docs` — id + title only). */
export interface SceneSummary {
  id: string;
  title: string;
}

/** A loaded scene: the normalized doc plus the exact `content` string it was stored as, kept so a
 *  later save can detect a change underneath us (the last-writer-wins interim). */
export interface LoadedScene {
  id: string;
  title: string;
  doc: SceneDoc;
  /** the raw stored JSON string — the baseline for the read-before-write conflict check */
  baseline: string;
}

/** Thrown by `saveScene` when the stored doc changed since `loaded.baseline` — an honest conflict
 *  the toolbar surfaces as "scene changed underneath you — reload?", never a silent clobber. */
export class SceneConflictError extends Error {
  constructor(public readonly id: string) {
    super(`scene "${id}" changed underneath you`);
    this.name = "SceneConflictError";
  }
}

/** Ensure a scene id carries the discovery prefix (idempotent). */
export function sceneId(id: string): string {
  return id.startsWith(SCENE_PREFIX) ? id : SCENE_PREFIX + id;
}

/** Stable serialization: sorted keys so a byte-compare of two logically-equal docs matches. This is
 *  the baseline both the round-trip test and the conflict check rely on. */
export function serializeScene(doc: SceneDoc): string {
  return JSON.stringify(doc, Object.keys(doc).length ? sortedReplacer() : undefined);
}

/** A replacer that emits object keys in sorted order (deep) — deterministic bytes for compare. */
function sortedReplacer() {
  return function (this: unknown, _key: string, value: unknown): unknown {
    if (value && typeof value === "object" && !Array.isArray(value)) {
      const rec = value as Record<string, unknown>;
      return Object.keys(rec)
        .sort()
        .reduce<Record<string, unknown>>((acc, k) => {
          acc[k] = rec[k];
          return acc;
        }, {});
    }
    return value;
  };
}

/** List the workspace's scene docs (the picker). Filters the shipped `list_docs` on the id prefix
 *  because the list output carries no tags. */
export async function listScenes(bridge: Bridge | WidgetBridge): Promise<SceneSummary[]> {
  const res = await bridge.call<{ docs?: SceneSummary[] }>("assets.list_docs");
  const docs = res?.docs ?? [];
  return docs.filter((d) => d.id.startsWith(SCENE_PREFIX));
}

/** Load a scene doc by id. Validates/normalizes the stored JSON (unknown type → placeholder, the
 *  same total path the playground proved) and keeps the raw baseline for the conflict check. */
export async function loadScene(
  bridge: Bridge | WidgetBridge,
  id: string,
): Promise<LoadedScene> {
  const key = sceneId(id);
  const res = await bridge.call<{ id: string; title?: string; content?: string }>(
    "assets.get_doc",
    { id: key },
  );
  const content = res?.content ?? "";
  let parsed: unknown = {};
  try {
    parsed = content ? JSON.parse(content) : {};
  } catch {
    // A corrupt body must not crash the load — validate() turns any input into an empty scene.
    parsed = {};
  }
  const { doc } = validateScene(parsed);
  return { id: key, title: res?.title ?? key, doc, baseline: serializeScene(doc) };
}

/** Save a scene. The last-writer-wins interim: re-read the stored doc first and compare it to the
 *  baseline the editor loaded; if it changed, THROW `SceneConflictError` instead of clobbering.
 *  When `loaded` is omitted (a brand-new scene) there is no baseline to defend. Returns the new
 *  baseline so the caller can keep editing without an extra read. */
export async function saveScene(
  bridge: Bridge,
  args: { id: string; title: string; doc: SceneDoc; loaded?: LoadedScene; ts?: number },
): Promise<LoadedScene> {
  const key = sceneId(args.id);

  // Read-before-write: only a scene we previously loaded has a baseline to protect. A conflict is
  // "the stored bytes differ from what we loaded" — someone else (or a future agent) wrote it.
  if (args.loaded) {
    const current = await loadScene(bridge, key);
    if (current.baseline !== args.loaded.baseline) {
      throw new SceneConflictError(key);
    }
  }

  const content = serializeScene(args.doc);
  await bridge.call("assets.put_doc", {
    id: key,
    title: args.title,
    content,
    content_type: "json",
    tags: [SCENE_TAG],
    ts: args.ts ?? 0,
  });
  return { id: key, title: args.title, doc: args.doc, baseline: content };
}
