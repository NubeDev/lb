// `@nube/genui/authoring` — the AUTHORING stratum entry (loaded by the builder ONLY; a viewer imports
// `@nube/genui` and never bundles this or the parser). It exposes the OpenUI-Lang adapter, the normalize
// pass, and the ONE loud accept step that the "AI widget" builder runs when the author accepts a preview.
//
// `acceptSpec` is the genui-scope "Parse once, persist the IR" boundary made concrete: parse →
// normalize → validate → size-check, ONCE, loudly. It returns the typed IR to persist (never raw Lang),
// the warnings to have shown in the preview, and — on failure — the stated rejection message. The SAME
// checks the host re-runs on `dashboard.save` (Decision 6), so accept and save reject identically.

import type { Catalog } from "./catalog/defineCatalog";
import { migrate } from "./ir/migrate";
import { errors, validate, warnings } from "./ir/validate";
import type { Finding, IrSpec } from "./ir/types";
import { normalize } from "./normalize/normalize";
import { parseLang } from "./adapters/openui/parse";

export { parseLang } from "./adapters/openui/parse";
export type { ParseResultIr } from "./adapters/openui/parse";
export { createLangStream } from "./adapters/openui/stream";
export type { LangStream } from "./adapters/openui/stream";
export { elementToIr } from "./adapters/openui/toIr";
export { normalize, PLACEHOLDER } from "./normalize/normalize";
export type { NormalizeResult } from "./normalize/normalize";
export { buildLangLibrary, catalogToLangName, langNameToCatalog, langRootName } from "./catalog/library";

/** The persisted-block size bound (genui-scope: "~8 KB"). The whole `options.genui` block (IR + meta) is
 *  bounded; an over-budget spec is almost certainly a bad generation and is rejected at accept AND at
 *  save. Kept here as the single source of truth the builder and tests share; the host mirrors it. */
export const GENUI_MAX_BYTES = 8 * 1024;

/** The byte size of a spec as it will be persisted (JSON). */
export function specByteSize(ir: IrSpec): number {
  return new TextEncoder().encode(JSON.stringify(ir)).length;
}

export interface AcceptOptions {
  catalog: Catalog;
  surfaceId?: string;
  /** Enforce the size bound (default true). The builder passes the whole `options.genui` size; when only
   *  the IR is measured here, the caller may add the meta overhead itself. */
  maxBytes?: number;
}

export interface AcceptResult {
  ok: boolean;
  ir?: IrSpec;
  /** All findings (warnings shown in preview + the errors that blocked, if any). */
  findings: Finding[];
  /** The single stated rejection message when `ok` is false. */
  error?: string;
}

/** Run the loud accept step on RAW OpenUI-Lang emission text. Parse → normalize → validate → size-check.
 *  Returns the typed IR to persist, or a loud rejection with the stated message. */
export function acceptLang(text: string, opts: AcceptOptions): AcceptResult {
  const surfaceId = opts.surfaceId ?? "cell";
  const parsed = parseLang(text, opts.catalog, surfaceId);
  return finishAccept(parsed.ir, parsed.findings, opts);
}

/** Run the loud accept step on an ALREADY-TYPED IR (the headless direct-IR choreography — no Lang round
 *  trip). Migrates, normalizes, validates, size-checks. */
export function acceptIr(ir: IrSpec, opts: AcceptOptions): AcceptResult {
  return finishAccept(migrate(ir), [], opts);
}

function finishAccept(ir: IrSpec, priorFindings: Finding[], opts: AcceptOptions): AcceptResult {
  const max = opts.maxBytes ?? GENUI_MAX_BYTES;
  const { spec, findings: normFindings } = normalize(ir, opts.catalog);
  const validateFindings = validate(spec, { catalog: opts.catalog });
  const findings = [...priorFindings, ...normFindings, ...validateFindings];

  const blocking = errors(validateFindings);
  if (blocking.length) {
    return {
      ok: false,
      findings,
      error: `widget spec is invalid: ${blocking.map((f) => f.message).join("; ")}`,
    };
  }
  const size = specByteSize(spec);
  if (size > max) {
    return {
      ok: false,
      findings,
      error: `widget spec is too large (${size} bytes > ${max}). Simplify the widget — one widget, one job.`,
    };
  }
  return { ok: true, ir: spec, findings: [...priorFindings, ...normFindings, ...warnings(validateFindings)] };
}
