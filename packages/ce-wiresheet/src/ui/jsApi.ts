// CodeMirror autocompletion fed by the jsScriptStore `getApi` action.
//
// getApi() → { dts, symbols } where `symbols` is a JSON array of
//   { label, kind:"prop"|"method"|…, signature, detail, doc, scope }
// `scope` groups completions: "ctx" → members offered after `ctx.`, "global"
// (or absent) → bare-identifier completions. Fetched once per service and
// cached, then read from a ref so the editor's extensions stay stable.

import type { Completion, CompletionContext, CompletionResult } from "@codemirror/autocomplete";
import type { FlexValue } from "../lib/engine-types";

export interface JsSymbol {
  label: string;
  kind?: string;
  signature?: string;
  detail?: string;
  doc?: string;
  scope?: string;
}

type CallAction = (uid: number, name: string, params?: Record<string, FlexValue>) => Promise<Record<string, FlexValue>>;

/** A labeled example script from the service's getExamples library. */
export interface JsExample {
  label: string;
  source: string;
  desc?: string;
}

const examplesCache = new Map<number, Promise<JsExample[]>>();

/** Fetch + cache the example-script library. Defensive about the item shape
 *  (label/name/title, source/code) since getExamples is still in flux. */
export function loadExamples(call: CallAction, serviceUid: number, action = "getExamples"): Promise<JsExample[]> {
  let pr = examplesCache.get(serviceUid);
  if (!pr) {
    pr = call(serviceUid, action, {}).then((ret) => {
      const raw = typeof ret?.examples === "string" ? ret.examples : "";
      try {
        const arr = JSON.parse(raw);
        if (!Array.isArray(arr)) return [];
        return arr.map((e: Record<string, unknown>, i: number): JsExample => ({
          // Prefer a human title for display; fall back to the machine name/label.
          label: String(e.title ?? e.label ?? e.name ?? `example ${i + 1}`),
          source: String(e.source ?? e.code ?? ""),
          desc: typeof (e.desc ?? e.description) === "string" ? (e.desc ?? e.description) as string : undefined,
        }));
      } catch {
        return [];
      }
    });
    examplesCache.set(serviceUid, pr);
    pr.catch(() => examplesCache.delete(serviceUid));
  }
  return pr;
}

/** Parsed getApi payload: completion `symbols` + the `dts` declaration text. */
export interface JsApi { symbols: JsSymbol[]; dts: string }

const cache = new Map<number, Promise<JsApi>>();

/** Fetch + cache the raw getApi payload for a jsScriptStore (keyed by its uid).
 *  One network call backs both the symbols and the dts. */
export function loadJsApiRaw(call: CallAction, serviceUid: number, action = "getApi"): Promise<JsApi> {
  let pr = cache.get(serviceUid);
  if (!pr) {
    pr = call(serviceUid, action, {}).then((ret) => {
      const rawSyms = typeof ret?.symbols === "string" ? ret.symbols : "";
      const dts = typeof ret?.dts === "string" ? ret.dts : "";
      let symbols: JsSymbol[] = [];
      try { const arr = JSON.parse(rawSyms); if (Array.isArray(arr)) symbols = arr as JsSymbol[]; } catch { /* ignore */ }
      return { symbols, dts };
    });
    cache.set(serviceUid, pr);
    pr.catch(() => cache.delete(serviceUid)); // don't cache a failed fetch
  }
  return pr;
}

/** Fetch + cache the script API completion symbols (legacy/fallback source). */
export function loadJsApi(call: CallAction, serviceUid: number, action = "getApi"): Promise<JsSymbol[]> {
  return loadJsApiRaw(call, serviceUid, action).then((r) => r.symbols);
}

const typeOf = (k?: string): Completion["type"] =>
  k === "method" ? "method" : k === "prop" ? "property" : k === "function" ? "function" : "variable";

const toCompletion = (s: JsSymbol): Completion => ({
  label: s.label,
  type: typeOf(s.kind),
  detail: s.detail || s.signature,
  info: s.doc || s.signature,
});

// The return/value type from a symbol's signature: methods "f(...): T" → T,
// props "x: T" → T. (Falls back to "" for void/unknown shapes.)
function typeFromSignature(sig?: string): string {
  if (!sig) return "";
  const method = sig.match(/\)\s*:\s*(.+)$/);
  if (method) return method[1].trim();
  const prop = sig.match(/:\s*(.+)$/);
  return prop ? prop[1].trim() : "";
}

// Map an API type to the scope that holds its members. The engine names nested
// API types `Ctx<Thing>` and scopes their members `ctx.<thing>` (e.g.
// CtxComponent → "ctx.component"). The first Ctx* token anywhere in the type is
// used, so unions/arrays/nullables resolve too (`CtxComponent | null`,
// `CtxComponent[]`). Primitives (number/string/void/…) → no scope.
function scopeForType(type: string): string | null {
  const m = type.match(/\bCtx([A-Za-z0-9]+)\b/);
  return m ? "ctx." + m[1].toLowerCase() : null;
}

const escapeRe = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const stripCall = (seg: string) => seg.replace(/\([^()]*\)\s*$/, "").trim(); // "component(1)" → "component"

// Split a member expression on TOP-LEVEL dots (ignoring dots inside call args),
// e.g. "ctx.app.component(100006)" → ["ctx","app","component(100006)"].
function splitTopLevelDots(expr: string): string[] {
  const out: string[] = [];
  let depth = 0, buf = "";
  for (const ch of expr) {
    if (ch === "(") { depth++; buf += ch; }
    else if (ch === ")") { depth = Math.max(0, depth - 1); buf += ch; }
    else if (ch === "." && depth === 0) { out.push(buf); buf = ""; }
    else buf += ch;
  }
  out.push(buf);
  return out;
}

// Resolve a member expression to the scope holding its members. `ctx` is the
// root; calls (`parent()`) are followed by their return type; a non-ctx root is
// treated as a LOCAL variable and inferred from its declaration in `doc`.
function resolveChainScope(symbols: JsSymbol[], doc: string, objExpr: string, depth = 0): string | null {
  if (depth > 8) return null;
  const parts = splitTopLevelDots(objExpr.trim());
  let scope: string | null = stripCall(parts[0]) === "ctx" ? "ctx" : inferLocalScope(symbols, doc, stripCall(parts[0]), depth + 1);
  for (let i = 1; scope && i < parts.length; i++) {
    const sym = symbols.find((s) => s.scope === scope && s.label === stripCall(parts[i]));
    scope = sym ? scopeForType(typeFromSignature(sym.signature)) : null;
  }
  return scope;
}

// Infer a local variable's member-scope from `const|let|var name = <chain>`.
function inferLocalScope(symbols: JsSymbol[], doc: string, name: string, depth: number): string | null {
  if (!/^[A-Za-z_$][\w$]*$/.test(name)) return null;
  const m = doc.match(new RegExp("\\b(?:const|let|var)\\s+" + escapeRe(name) + "\\s*=\\s*([^;\\n]+)"));
  return m ? resolveChainScope(symbols, doc, m[1].trim(), depth) : null;
}

// Member chain before the cursor: identifiers + optional single-level calls.
const CHAIN_RE = /([\w$]+(?:\([^()]*\))?(?:\.[\w$]+(?:\([^()]*\))?)*)\.([\w$]*)$/;

/** Completion source reading the latest symbols from `ref`. Member access
 *  (`ctx.self.x`, `ctx.app.component(0).`, or a local var) resolves the chain by
 *  each member's TYPE to its member-scope; bare identifiers offer globals + ctx. */
export function jsCompletionSource(ref: { current: JsSymbol[] }) {
  return (context: CompletionContext): CompletionResult | null => {
    const symbols = ref.current;
    const member = context.matchBefore(CHAIN_RE);
    if (member) {
      const m = member.text.match(CHAIN_RE);
      if (m) {
        const doc = context.state.doc.toString();
        const scope = resolveChainScope(symbols, doc, m[1]);
        const options = scope ? symbols.filter((s) => s.scope === scope).map(toCompletion) : [];
        if (options.length === 0) return null;
        return { from: member.to - m[2].length, options, validFor: /^[\w$]*$/ };
      }
    }
    const word = context.matchBefore(/[\w$]*/);
    if (!word || (word.from === word.to && !context.explicit)) return null;
    const globals = symbols.filter((s) => !s.scope || s.scope === "global").map(toCompletion);
    const options: Completion[] = [{ label: "ctx", type: "variable", detail: "script context" }, ...globals];
    return { from: word.from, options, validFor: /^[\w$]*$/ };
  };
}
