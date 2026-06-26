// The in-memory registry stand-in used when NOT in the Tauri shell (plain browser, tests). It mirrors
// the node's `registry.*` contract faithfully enough that the UI behaves identically here and against
// the real node (the verb names + shapes match the Rust commands one-to-one).
//
// Faithful to the gates the user actually hits:
//   - the CAPABILITY gate: each verb needs its `mcp:registry.<verb>:call` grant, else "denied"
//     (the same opaque deny the Rust `registry_test` proves);
//   - the SIGNATURE gate (the S7 headline): installing an artifact whose catalog entry is flagged
//     untrusted (tampered/unsigned/foreign-key) returns `verified: false` and installs NOTHING — the
//     verify-before-cache gate, independent of the capability gate (the Rust `install_rejects_
//     tampered_artifact_even_with_grant` mirror);
//   - OFFLINE + ROLLBACK: once a version is installed (cached), installing it again — or a PRIOR
//     version — is the same verb; the "installed version" flips, mirroring `install_from_registry`.
//
// One file per concern (FILE-LAYOUT): the registry fake lives beside the channel/agent/workflow fakes.

import type { CatalogEntry } from "@/lib/registry/registry.types";

const LIST_CAP = "mcp:registry.list:call";
const INSTALL_CAP = "mcp:registry.install:call";

/** A seeded catalog entry plus whether its artifact is trusted (a faithful stand-in for "the
 *  signature verifies against an allow-listed publisher key"). */
interface SeedEntry extends CatalogEntry {
  trusted: boolean;
}

// Workspace-scoped state (key prefix = ws) — the hard wall, mirrored.
const catalog = new Map<string, SeedEntry[]>(); // `${ws}` -> entries
const installed = new Map<string, string>(); // `${ws}/${extId}` -> installed version

const k = (ws: string, x: string) => `${ws}/${x}`;

function capMatches(held: string[], cap: string): boolean {
  return held.some((h) => h === cap || h === "mcp:registry.*:call" || h === "mcp:*:call");
}

export function registryFakeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> | null {
  switch (cmd) {
    case "registry_list": {
      const { ws, extId, caps } = args as { ws: string; extId: string; caps?: string[] };
      if (!capMatches(caps ?? [], LIST_CAP)) return Promise.reject(new Error("denied"));
      const entries = (catalog.get(ws) ?? [])
        .filter((e) => e.extId === extId)
        .map(({ trusted: _trusted, ...e }) => e); // strip the test-only flag from the wire shape
      return Promise.resolve(entries as T);
    }
    case "registry_install": {
      const { ws, extId, version, caps } = args as {
        ws: string;
        extId: string;
        version: string;
        caps?: string[];
      };
      if (!capMatches(caps ?? [], INSTALL_CAP)) return Promise.reject(new Error("denied"));
      const entry = (catalog.get(ws) ?? []).find(
        (e) => e.extId === extId && e.version === version,
      );
      // Unknown version → nothing to install (mirrors NotAvailable; the UI surfaces it as not verified).
      if (!entry) return Promise.resolve({ extId, version, verified: false } as T);
      // THE SIGNATURE GATE: an untrusted artifact is refused — nothing installed, even with the grant.
      if (!entry.trusted) return Promise.resolve({ extId, version, verified: false } as T);
      // Verified → install (or roll back to) this version; the installed version flips.
      installed.set(k(ws, extId), version);
      return Promise.resolve({ extId, version, verified: true } as T);
    }
    default:
      return null; // not a registry command — let the caller fall through
  }
}

/** Test/seed helper: add a catalog entry to a workspace, marked trusted or not. */
export function __seedCatalog(ws: string, entry: SeedEntry): void {
  const list = catalog.get(ws) ?? [];
  list.push(entry);
  catalog.set(ws, list);
}

/** Test helper: the currently-installed version of `extId` in `ws` (or undefined). */
export function __installedVersion(ws: string, extId: string): string | undefined {
  return installed.get(k(ws, extId));
}

/** Test helper: clear all registry fake state. */
export function __resetRegistryFake(): void {
  catalog.clear();
  installed.clear();
}
