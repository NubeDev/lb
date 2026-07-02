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

export type ArtifactKind = "native-bin" | "wasm" | "remote-entry";

export interface Artifact {
  kind: ArtifactKind;
  path: string;
  size: number;
  /** RFC3339 UTC (seconds precision), or null if the mtime couldn't be read. */
  mtime: string | null;
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
  /** Concrete build outputs on disk with current size + mtime; empty before a first build. */
  artifacts: Artifact[];
}

export interface DevkitRoot {
  path: string;
  os: string;
}

export function listDevkitTemplates(): Promise<TemplateInfo[]> {
  return invoke<TemplateInfo[]>("mcp_call", { tool: "devkit.templates", args: {} });
}

/** The absolute devkit root directory — where scaffolds live and the only tree `inspect`/`build`/
 *  `publish` accept a path under. The "open existing" folder picker browses from here. */
export function devkitRoot(): Promise<DevkitRoot> {
  return invoke<DevkitRoot>("mcp_call", { tool: "devkit.root", args: {} });
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
