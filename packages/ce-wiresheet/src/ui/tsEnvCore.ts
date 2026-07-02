// In-browser TypeScript language service for the JS script editor — the HEAVY
// core (statically imports `typescript` + the standard-library .d.ts strings).
// Loaded lazily via ./tsEnv; don't import this module statically from UI code.
//
// Scripts are ES modules exporting lifecycle functions — `export function
// evaluate(ctx) {…}` — where `ctx` is typed `Ctx` by the engine but written in
// plain JS (untyped). To get real inference (locals, for-of element types,
// member access, diagnostics) we:
//   1. turn the engine's getApi `dts` into AMBIENT GLOBAL declarations (strip its
//      `export function` lifecycle block so the interfaces are global), and
//   2. INJECT parameter type annotations into the user's lifecycle functions
//      (`evaluate(ctx)` → `evaluate(ctx: Ctx)`), tracking the inserted spans so
//      positions/diagnostics map back to the editor's text exactly.
//
// One module-level environment is reused across editors; each query re-syncs the
// active editor's doc first, so switching tabs can't cross-contaminate.

import ts from "typescript";
import { createSystem, createVirtualTypeScriptEnvironment, type VirtualTypeScriptEnvironment } from "@typescript/vfs";
import { TS_LIBS } from "./tsLibs";

const SCRIPT = "/script.ts";
const CTX_DTS = "/ctx.d.ts";

const COMPILER_OPTIONS: ts.CompilerOptions = {
  target: ts.ScriptTarget.ES2020,
  lib: ["lib.es2020.d.ts"],
  module: ts.ModuleKind.ESNext,
  moduleResolution: ts.ModuleResolutionKind.Bundler,
  allowJs: true,
  checkJs: false,
  noEmit: true,
  noImplicitAny: false, // user helpers with untyped params shouldn't error
  strict: false,
  skipLibCheck: true,
  allowNonTsExtensions: true,
};

// Lifecycle exports whose parameters we annotate (positional types, from the dts).
const LIFECYCLE: Record<string, string[]> = {
  evaluate: ["Ctx"],
  onStart: ["Ctx"],
  onStop: ["Ctx"],
  onParentChanged: ["Ctx", "CtxComponent | null", "CtxComponent | null"],
  onChildAdded: ["Ctx", "CtxComponent"],
  onChildRemoved: ["Ctx", "CtxComponent"],
  onDeleted: ["Ctx"],
};

interface Insert { at: number; text: string }

// Strip the dts's trailing `export function …` lifecycle block so what remains
// (the interface declarations) is an ambient GLOBAL script visible to the module.
function toGlobalDts(dts: string): string {
  const idx = dts.indexOf("\nexport ");
  return idx >= 0 ? dts.slice(0, idx) : dts;
}

// Annotate lifecycle-function parameters; return the rewritten text plus the
// inserted spans (sorted by original offset) for position mapping.
function transformSource(src: string): { text: string; inserts: Insert[] } {
  const inserts: Insert[] = [];
  const re = /\bfunction\s+(evaluate|onStart|onStop|onParentChanged|onChildAdded|onChildRemoved|onDeleted)\s*\(([^)]*)\)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(src))) {
    const types = LIFECYCLE[m[1]];
    const parenOpen = m.index + m[0].lastIndexOf("(");
    let cursor = parenOpen + 1;
    m[2].split(",").forEach((part, i) => {
      const partStart = cursor;
      cursor += part.length + 1; // include the comma separator
      const type = types[i];
      if (!type || part.includes(":")) return; // no type for this slot, or already annotated
      const id = part.match(/[A-Za-z_$][\w$]*/);
      if (!id) return;
      inserts.push({ at: partStart + (id.index ?? 0) + id[0].length, text: ": " + type });
    });
  }
  inserts.sort((a, b) => a.at - b.at);
  let text = "", prev = 0;
  for (const ins of inserts) { text += src.slice(prev, ins.at) + ins.text; prev = ins.at; }
  return { text: text + src.slice(prev), inserts };
}

// Editor offset → TS-file offset: shift right by inserts at/before it.
function o2t(inserts: Insert[], o: number): number {
  let shift = 0;
  for (const ins of inserts) { if (ins.at <= o) shift += ins.text.length; else break; }
  return o + shift;
}

// TS-file offset → editor offset: undo inserts before it; clamp if inside one.
function t2o(inserts: Insert[], t: number): number {
  let shift = 0;
  for (const ins of inserts) {
    const tsAt = ins.at + shift;
    if (t <= tsAt) break;
    if (t < tsAt + ins.text.length) return ins.at;
    shift += ins.text.length;
  }
  return t - shift;
}

let env: VirtualTypeScriptEnvironment | null = null;
let envDts = "\0"; // sentinel so first sync always builds
let lastCode: string | null = null;
let lastInserts: Insert[] = [];

function ensureEnv(dts: string): VirtualTypeScriptEnvironment {
  if (env && envDts === dts) return env;
  const fsMap = new Map<string, string>(TS_LIBS);
  // Safety net: some TS versions resolve the default lib by target name.
  fsMap.set("/lib.es2020.full.d.ts", '/// <reference lib="es2020" />\n');
  fsMap.set(CTX_DTS, toGlobalDts(dts));
  fsMap.set(SCRIPT, " ");
  const system = createSystem(fsMap);
  env = createVirtualTypeScriptEnvironment(system, [SCRIPT, CTX_DTS], ts, COMPILER_OPTIONS);
  envDts = dts;
  lastCode = null;
  return env;
}

// Build/reuse the env for `dts` and sync `code` into the script file.
function sync(dts: string, code: string): VirtualTypeScriptEnvironment {
  const e = ensureEnv(dts);
  if (code !== lastCode) {
    const tr = transformSource(code);
    e.updateFile(SCRIPT, tr.text || " ");
    lastCode = code;
    lastInserts = tr.inserts;
  }
  return e;
}

/** Signature of {@link getCompletions} (lets ./tsEnv reference the return type
 *  without importing `typescript` itself). */
export type GetCompletions = typeof getCompletions;

/** Raw TS completion entries at an editor offset (empty if dts not ready). */
export function getCompletions(dts: string, code: string, pos: number): ts.CompletionEntry[] {
  if (!dts) return [];
  try {
    const e = sync(dts, code);
    const info = e.languageService.getCompletionsAtPosition(SCRIPT, o2t(lastInserts, pos), {
      includeCompletionsForModuleExports: false,
    });
    return info?.entries ?? [];
  } catch { return []; }
}

/** Detail + documentation for one completion entry (for the info popup). */
export function getCompletionDetail(dts: string, code: string, pos: number, name: string): { detail: string; doc: string } | null {
  if (!dts) return null;
  try {
    const e = sync(dts, code);
    const d = e.languageService.getCompletionEntryDetails(SCRIPT, o2t(lastInserts, pos), name, undefined, undefined, undefined, undefined);
    if (!d) return null;
    return { detail: ts.displayPartsToString(d.displayParts), doc: ts.displayPartsToString(d.documentation) };
  } catch { return null; }
}

/** Hover info at an editor offset, with span mapped back to editor coordinates. */
export function getQuickInfo(dts: string, code: string, pos: number): { from: number; to: number; detail: string; doc: string } | null {
  if (!dts) return null;
  try {
    const e = sync(dts, code);
    const qi = e.languageService.getQuickInfoAtPosition(SCRIPT, o2t(lastInserts, pos));
    if (!qi) return null;
    return {
      from: t2o(lastInserts, qi.textSpan.start),
      to: t2o(lastInserts, qi.textSpan.start + qi.textSpan.length),
      detail: ts.displayPartsToString(qi.displayParts),
      doc: ts.displayPartsToString(qi.documentation),
    };
  } catch { return null; }
}

export interface TsDiagnostic { from: number; to: number; message: string; severity: "error" | "warning" | "info" }

/** Syntactic + semantic diagnostics, spans mapped back to editor coordinates. */
export function getDiagnostics(dts: string, code: string): TsDiagnostic[] {
  if (!dts) return [];
  try {
    const e = sync(dts, code);
    const raw = [
      ...e.languageService.getSyntacticDiagnostics(SCRIPT),
      ...e.languageService.getSemanticDiagnostics(SCRIPT),
    ];
    return raw.map((d): TsDiagnostic => {
      const start = d.start ?? 0;
      return {
        from: t2o(lastInserts, start),
        to: t2o(lastInserts, start + (d.length ?? 0)),
        message: ts.flattenDiagnosticMessageText(d.messageText, "\n"),
        severity: d.category === ts.DiagnosticCategory.Error ? "error" : d.category === ts.DiagnosticCategory.Warning ? "warning" : "info",
      };
    });
  } catch { return []; }
}

/** Map a TS completion kind to a CodeMirror completion type. */
export function cmCompletionType(kind: string): string {
  switch (kind) {
    case ts.ScriptElementKind.memberFunctionElement:
    case ts.ScriptElementKind.functionElement:
    case ts.ScriptElementKind.localFunctionElement:
    case ts.ScriptElementKind.constructorImplementationElement:
      return "method";
    case ts.ScriptElementKind.memberVariableElement:
    case ts.ScriptElementKind.memberGetAccessorElement:
    case ts.ScriptElementKind.memberSetAccessorElement:
      return "property";
    case ts.ScriptElementKind.variableElement:
    case ts.ScriptElementKind.letElement:
    case ts.ScriptElementKind.constElement:
    case ts.ScriptElementKind.parameterElement:
      return "variable";
    case ts.ScriptElementKind.classElement:
      return "class";
    case ts.ScriptElementKind.interfaceElement:
      return "interface";
    case ts.ScriptElementKind.enumElement:
      return "enum";
    case ts.ScriptElementKind.keyword:
      return "keyword";
    default:
      return "text";
  }
}
