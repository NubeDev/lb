// The extension-lifecycle api client — one call per export, mirroring the host `ext` surface verbs
// and the gateway `/extensions` routes 1:1 (lifecycle-management scope). One uniform `ExtRow` across
// both tiers (wasm/native), exactly like `lb_host::ExtRow`. The workspace is the session's. Start/stop
// map to enable/disable (the durable intent the reconciler honors); uninstall evicts the binary.

import { invoke } from "@/lib/ipc/invoke";

/** A page or widget an extension contributes to the shell (ui-federation / dashboard-widgets scopes).
 *  Mirrors `lb_assets::ExtUi`. `entry` is the ESM bundle file under the gateway's
 *  `/extensions/{ext}/ui/` dir; `scope` is the read-only MCP tools the page may call through the
 *  host bridge (already narrowed to the install grant server-side). */
export interface ExtUi {
  entry: string;
  label: string;
  icon: string;
  scope: string[];
}

export interface ExtRow {
  ext: string;
  version: string;
  tier: "wasm" | "native";
  enabled: boolean;
  running: boolean;
  health: string;
  restart_count: number;
  /** A full sidebar page this extension contributes, or null. */
  ui?: ExtUi | null;
  /** The dashboard widget tiles this extension contributes — one per `[[widget]]` table it
   *  declared (dashboard-widgets scope). Empty/absent if none. Mirrors `lb_host::ExtRow::widgets`. */
  widgets?: ExtUi[];
}

/** A signed extension artifact — the wire shape the host `ext_publish` / registry-host `POST
 *  /artifacts` accept (mirrors `lb_registry::Artifact` 1:1). `wasm`/`signature` are byte arrays
 *  (`Vec<u8>` → JSON number[]). The UI never mints these — a signed artifact is produced by the
 *  publisher tooling; the console just uploads it. The host verify-before-stores it. */
export interface Artifact {
  ext_id: string;
  version: string;
  manifest_toml: string;
  wasm: number[];
  digest_hex: string;
  publisher_key_id: string;
  signature: number[];
}

/** List installed extensions (both tiers, live state). Mirrors `ext.list`. */
export function listExtensions(): Promise<ExtRow[]> {
  return invoke<ExtRow[]>("ext_list", {});
}

/** Enable (start) `ext` — flips the durable intent; a native child is (re)started. Mirrors `ext.enable`. */
export function enableExtension(ext: string): Promise<void> {
  return invoke<void>("ext_enable", { ext });
}

/** Disable (stop) `ext` — flips intent off; a running native child is stopped (distinct from
 *  uninstall: the install stays). The reconciler won't auto-start it. Mirrors `ext.disable`. */
export function disableExtension(ext: string): Promise<void> {
  return invoke<void>("ext_disable", { ext });
}

/** Uninstall `ext` — stop any native child + tombstone the install + evict the cached binary
 *  (idempotent). Irreversible from the console. Mirrors `ext.uninstall`. */
export function uninstallExtension(ext: string): Promise<void> {
  return invoke<void>("ext_uninstall", { ext });
}

/** Upload (publish) a signed `artifact` into the current workspace. The host verifies the signature
 *  BEFORE storing — a tampered/unsigned/foreign-key upload is rejected and nothing is stored
 *  (verify-before-store). The workspace comes from the session token, never the artifact. Mirrors
 *  `ext.publish` / the gateway `POST /extensions`. */
export function publishArtifact(artifact: Artifact): Promise<void> {
  return invoke<void>("ext_publish", { artifact });
}
