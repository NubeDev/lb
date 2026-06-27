// The render-template API client — the `template.*` durable scripted-view CRUD (widget-builder scope).
// These ride the host-mediated `POST /mcp/call` bridge (the `mcp_call` invoke verb), exactly like any
// other MCP tool — the builder consumes tools, it doesn't get a bespoke REST surface. Each is
// capability-gated server-side (`mcp:template.<verb>:call`); the workspace + author come from the
// session token (the hard wall, §7), never an argument.

import { invoke } from "@/lib/ipc/invoke";

/** The render engine a template targets (mirrors `lb_host::Engine`). */
export type TemplateEngine = "template" | "plot" | "d3";

/** A durable scripted-view template (mirrors `lb_host::RenderTemplate`). */
export interface RenderTemplate {
  id: string;
  title: string;
  engine: TemplateEngine;
  code: string;
  author: string;
  updated_ts: number;
  deleted?: boolean;
}

/** A roster summary (no code body; mirrors `lb_host::RenderTemplateSummary`). */
export interface RenderTemplateSummary {
  id: string;
  title: string;
  engine: TemplateEngine;
  author: string;
  updated_ts: number;
}

/** Logical now for the upsert ts (no wall-clock in the verb; the caller supplies it). */
function now(): number {
  return Date.now();
}

/** Save (create/update) a durable template. Author-only update. Mirrors `template.save`. */
export function saveTemplate(
  id: string,
  title: string,
  engine: TemplateEngine,
  code: string,
): Promise<RenderTemplate> {
  return invoke<RenderTemplate>("mcp_call", {
    tool: "template.save",
    args: { id, title, engine, code, now: now() },
  });
}

/** Read one durable template (its code). Mirrors `template.get`. */
export function getTemplate(id: string): Promise<RenderTemplate> {
  return invoke<RenderTemplate>("mcp_call", { tool: "template.get", args: { id } });
}

/** List the workspace's durable templates (summaries). Mirrors `template.list`. */
export function listTemplates(): Promise<RenderTemplateSummary[]> {
  return invoke<{ templates: RenderTemplateSummary[] }>("mcp_call", {
    tool: "template.list",
    args: {},
  }).then((r) => r.templates);
}

/** Soft-delete a durable template (author-only). Mirrors `template.delete`. */
export function deleteTemplate(id: string): Promise<void> {
  return invoke<{ ok: boolean }>("mcp_call", {
    tool: "template.delete",
    args: { id, now: now() },
  }).then(() => undefined);
}
