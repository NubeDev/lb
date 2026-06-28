// Browser client for the local extension SDK surface. Every devkit verb rides the existing
// host-mediated `POST /mcp/call` bridge; publish deliberately reuses `POST /extensions`.

import { invoke } from "@/lib/ipc/invoke";

export type DevkitTier = "wasm" | "native";
export type DevkitFeature = "ui" | "series-read" | "ingest" | "kv";

export interface TemplateInfo {
  tier: DevkitTier;
  features: DevkitFeature[];
  world: string;
}

export interface ScaffoldReport {
  id: string;
  tier: DevkitTier;
  path: string;
  files: string[];
}

export interface BuildStarted {
  job_id: string;
  log_subject: string;
}

export interface InspectReport {
  id: string;
  tier: DevkitTier;
  tools: string[];
  caps: string[];
  built: boolean;
  toolchain: {
    cargo: boolean;
    pnpm: boolean;
    wasm32_wasip2: boolean;
  };
}

export function listDevkitTemplates(): Promise<TemplateInfo[]> {
  return invoke<TemplateInfo[]>("mcp_call", { tool: "devkit.templates", args: {} });
}

export function scaffoldDevkitExtension(input: {
  id: string;
  tier: DevkitTier;
  features: DevkitFeature[];
}): Promise<ScaffoldReport> {
  return invoke<ScaffoldReport>("mcp_call", {
    tool: "devkit.scaffold",
    args: input,
  });
}

export function buildDevkitExtension(path: string): Promise<BuildStarted> {
  return invoke<BuildStarted>("mcp_call", {
    tool: "devkit.build",
    args: { path, ts: Date.now() },
  });
}

export function inspectDevkitExtension(path: string): Promise<InspectReport> {
  return invoke<InspectReport>("mcp_call", { tool: "devkit.inspect", args: { path } });
}

export function publishDevkitExtension(path: string): Promise<void> {
  return invoke<void>("ext_publish", { artifact: { path } });
}
