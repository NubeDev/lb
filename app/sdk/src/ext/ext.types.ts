// Extension-list types — mirror `ui/src/lib/ext/ext.api.ts` / `lb_host::ExtRow` one-to-one. The app
// consumes `ext.list` for nav discovery only in this slice; the `[app]` block lands with the
// app-extensions slice as an additive field.

/** A page or widget an extension contributes — mirrors `lb_assets::ExtUi`. */
export interface ExtUi {
  entry: string;
  label: string;
  icon: string;
  /** The MCP tools the page may call through the host bridge (narrowed to the install grant). */
  scope: string[];
  data?: boolean;
}

/** One installed extension with live state — mirrors `lb_host::ExtRow`. */
export interface ExtRow {
  ext: string;
  version: string;
  tier: "wasm" | "native";
  enabled: boolean;
  running: boolean;
  health: string;
  restart_count: number;
  ui?: ExtUi | null;
  widgets?: ExtUi[];
}
