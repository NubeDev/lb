// The in-memory extension-lifecycle fake (TEST-ONLY) — mirrors the gateway `ext_*` routes 1:1
// (lifecycle-management scope). Workspace-scoped via the session store (as the real gateway derives
// the ws from the token). One uniform row `{ext,version,tier,enabled,running,health,restart_count}`
// across both tiers, exactly like `lb_host::ExtRow`. Returns `null` for any unowned command.

import { getSession } from "@/lib/session/session.store";

export interface ExtRow {
  ext: string;
  version: string;
  tier: "wasm" | "native";
  enabled: boolean;
  running: boolean;
  health: string;
  restart_count: number;
}

const installs = new Map<string, Map<string, ExtRow>>(); // ws → (ext → row)

function ws(): string {
  return getSession()?.workspace ?? "";
}
function rows(): Map<string, ExtRow> {
  const inner = installs.get(ws()) ?? new Map<string, ExtRow>();
  installs.set(ws(), inner);
  return inner;
}
function health(r: ExtRow): string {
  if (!r.enabled) return "disabled";
  return r.running ? "ok" : "stopped";
}

export function extFakeInvoke<T>(cmd: string, args?: Record<string, unknown>): T | null {
  switch (cmd) {
    case "ext_list":
      return [...rows().values()].sort((a, b) => a.ext.localeCompare(b.ext)) as T;
    case "ext_enable": {
      const { ext } = args as { ext: string };
      const r = rows().get(ext);
      if (r) {
        r.enabled = true;
        r.running = r.tier === "wasm";
        r.health = health(r);
      }
      return undefined as T;
    }
    case "ext_disable": {
      const { ext } = args as { ext: string };
      const r = rows().get(ext);
      if (r) {
        r.enabled = false;
        r.running = false;
        r.health = health(r);
      }
      return undefined as T;
    }
    case "ext_uninstall": {
      const { ext } = args as { ext: string };
      rows().delete(ext); // idempotent: deleting an absent ext is a no-op success.
      return undefined as T;
    }
    case "ext_publish": {
      // Verify-before-store, mirrored: an artifact flagged untrusted (tampered/unsigned/foreign-key)
      // is rejected and NOTHING is installed — the same gate the host's `ext_publish` enforces. On a
      // verified upload the extension appears installed in THIS workspace (the wall holds via `ws()`).
      const { artifact } = args as { artifact: Artifactish };
      if (artifact.__trusted === false) throw new Error("unverified"); // 422 on the real path
      __seedExt({
        ext: artifact.ext_id,
        version: artifact.version,
        tier: artifact.manifest_toml.includes("native") ? "native" : "wasm",
        enabled: true,
      });
      return undefined as T;
    }
    default:
      return null;
  }
}

/** The artifact wire shape the fake inspects (a subset of `Artifact` + a TEST-ONLY trust flag standing
 *  in for "the signature verifies against an allow-listed publisher key", exactly like registry.fake). */
interface Artifactish {
  ext_id: string;
  version: string;
  manifest_toml: string;
  /** TEST-ONLY: `false` = the verify-before-store gate rejects it (nothing installed). Default trusted. */
  __trusted?: boolean;
}

/** Test helper: seed an installed extension into the current session's workspace. */
export function __seedExt(row: Partial<ExtRow> & { ext: string }): void {
  const full: ExtRow = {
    version: "v1",
    tier: "wasm",
    enabled: true,
    running: row.tier === "native" ? false : true,
    health: "ok",
    restart_count: 0,
    ...row,
  };
  full.health = health(full);
  rows().set(full.ext, full);
}

/** Test helper: clear the fake installs between tests. */
export function __resetExtFake(): void {
  installs.clear();
}

/** DEV seed (no-gateway browser build only) — give the current workspace the two reference
 *  extensions so the console isn't empty out of the box, mirroring a freshly-provisioned node. Called
 *  once from the app shell; idempotent (no-op if the workspace already has installs). The gateway path
 *  never touches the fake, so this only affects the local dev/demo build. NOT used by tests (they
 *  `__resetExtFake` + `__seedExt` explicitly). */
export function seedDevExtensions(): void {
  if (rows().size > 0) return;
  __seedExt({ ext: "hello", version: "v2", tier: "wasm", enabled: true });
  __seedExt({
    ext: "echo-sidecar",
    version: "v1",
    tier: "native",
    enabled: true,
    running: true,
    restart_count: 0,
  });
}
