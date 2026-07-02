// Lazy front for the in-browser TypeScript language service. The real
// implementation (tsEnvCore) statically imports `typescript` (~7MB) plus the
// standard-library .d.ts strings, so it's split into its own chunk and loaded on
// first use — opening the JS editor — rather than bloating the wiresheet bundle
// for everyone. Until the chunk resolves, queries return empty and callers fall
// back to the lightweight symbol-based completion source.
//
// `import type` is erased at build time, so it does NOT pull the core in eagerly.
import type * as Core from "./tsEnvCore";

type CoreModule = typeof import("./tsEnvCore");

let core: CoreModule | null = null;
let loading: Promise<unknown> | null = null;

// Kick off (once) the dynamic import; returns the module if already resolved.
function ensureCore(): CoreModule | null {
  if (core) return core;
  if (!loading) loading = import("./tsEnvCore").then((m) => { core = m; }).catch(() => { loading = null; });
  return null;
}

/** True once the TypeScript chunk has loaded and queries are meaningful. */
export function isReady(): boolean {
  return core !== null;
}

export function getCompletions(dts: string, code: string, pos: number): ReturnType<Core.GetCompletions> {
  return (ensureCore()?.getCompletions(dts, code, pos) ?? []) as ReturnType<Core.GetCompletions>;
}

export function getCompletionDetail(dts: string, code: string, pos: number, name: string): { detail: string; doc: string } | null {
  return ensureCore()?.getCompletionDetail(dts, code, pos, name) ?? null;
}

export function getQuickInfo(dts: string, code: string, pos: number): { from: number; to: number; detail: string; doc: string } | null {
  return ensureCore()?.getQuickInfo(dts, code, pos) ?? null;
}

export function getDiagnostics(dts: string, code: string): Core.TsDiagnostic[] {
  return ensureCore()?.getDiagnostics(dts, code) ?? [];
}

export function cmCompletionType(kind: string): string {
  return core?.cmCompletionType(kind) ?? "text";
}
