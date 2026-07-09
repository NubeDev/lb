// Browser client for the host filesystem-metadata tools (`host.fs.*`), reached through the same
// host-mediated `POST /mcp/call` bridge as every other verb. Read-only: one directory level of
// metadata, used by the Studio folder picker so a user browses to an extension instead of typing a
// path. The host gates each call on `mcp:host.fs.list:call` and enforces its own path rules.

import { invoke } from "@/lib/ipc/invoke";

export type HostFsKind = "dir" | "file" | "symlink" | "other";

export interface HostFsEntry {
  name: string;
  kind: HostFsKind;
  size: number | null;
}

export interface HostFsList {
  /** The listed path, normalized to forward slashes. */
  path: string;
  os: string;
  entries: HostFsEntry[];
  /** True when the directory held more than the host's per-list cap. */
  truncated: boolean;
}

/**
 * Optional server-side narrowing for a directory listing. All fields are additive and default to the
 * unfiltered behavior when omitted (`host.fs.list` applies them per-entry before its per-list cap):
 *   - `name`          — case-insensitive substring the entry name must contain.
 *   - `extensions`    — file extensions (with or without a leading dot, e.g. `"db"` or `".db"`);
 *                       only real files match, case-insensitively.
 *   - `includeHidden` — when false (default) dot-prefixed entries (files AND dirs) are hidden.
 */
export interface HostFsListFilter {
  name?: string;
  extensions?: string[];
  includeHidden?: boolean;
}

/** List one directory level of host filesystem metadata. Entries arrive name-sorted. */
export function listHostDir(path: string, filter?: HostFsListFilter): Promise<HostFsList> {
  const args: Record<string, unknown> = { path };
  if (filter?.name?.trim()) args.name = filter.name.trim();
  if (filter?.extensions?.length) args.extensions = filter.extensions;
  // Only send include_hidden when explicitly enabled — omission is the host's "hide hidden" default.
  if (filter?.includeHidden) args.include_hidden = true;
  return invoke<HostFsList>("mcp_call", { tool: "host.fs.list", args });
}

export interface HostFsHome {
  /** The node's home directory, normalized to forward slashes. */
  path: string;
  os: string;
}

/** The node's home directory — a stable absolute anchor a filesystem picker starts browsing from. */
export function hostHomeDir(): Promise<HostFsHome> {
  return invoke<HostFsHome>("mcp_call", { tool: "host.fs.home", args: {} });
}
