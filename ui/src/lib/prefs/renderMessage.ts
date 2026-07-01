// Client-side MF1 catalog rendering (i18n-catalogs scope, the client half). The client parses the
// SAME MF1 source strings the Rust host renders — from the GENERATED `catalog.generated.ts` (byte-
// identical to the host `.mf` assets, drift-tested) — using `intl-messageformat` (the de-facto TS MF1
// implementation the dialect is pinned to). Host and client therefore only ever see constructs both
// implement: the host==client guarantee the one-source design rests on.
//
// Typed placeholders that touch canonical data (`{ts, date}`, `{v, quantity, dim}`) are the HOST's job
// (the client calls `format.*` for those, so tz/unit/CLDR math has one implementation). This module
// covers the pure text/plural/select rendering — where the client formats locally for zero-latency
// first paint — and is the surface the intl-messageformat cross-check test exercises against the Rust
// parser.

import { IntlMessageFormat } from "intl-messageformat";
import { BUILTIN_CATALOGS, type BuiltinCatalog } from "./catalog.generated";

/** The merged catalog for `locale` — the workspace `override` (from `prefs.catalog`) shadowing the
 *  built-in, then the `en` built-in, per the fallback chain. An unknown locale merges over `en`. */
export function mergedCatalog(
  locale: string,
  override: Record<string, string> = {},
): BuiltinCatalog {
  const base = BUILTIN_CATALOGS[locale] ?? BUILTIN_CATALOGS.en;
  return { version: base.version, messages: { ...base.messages, ...override } };
}

/** Render catalog `key` for `locale` with `args`, consulting `override` first. Fallback chain:
 *  override → builtin[locale] → builtin.en → the key literal (never blank, never throws). Mirrors the
 *  host `catalog::render` selection order for the pure (non-typed-placeholder) messages. */
export function renderMessage(
  key: string,
  args: Record<string, unknown>,
  locale: string,
  override: Record<string, string> = {},
): string {
  const src = pickSource(key, locale, override);
  if (src === null) return key; // never blank — the key literal is the last resort.
  try {
    // `#` inside a plural arm and `one`/`other`/`=N` selection are intl-messageformat native — the
    // exact subset the host parser implements. The locale drives plural-category selection (en/es
    // share the two-category CLDR rule the host hand-encodes).
    const mf = new IntlMessageFormat(src, locale);
    return String(mf.format(args as Record<string, string | number>));
  } catch {
    return key; // a malformed message falls to the key, matching the host's never-panic contract.
  }
}

/** The message source for `key`: override, then builtin[locale], then builtin.en, else null. */
function pickSource(
  key: string,
  locale: string,
  override: Record<string, string>,
): string | null {
  if (override[key] !== undefined) return override[key];
  const local = BUILTIN_CATALOGS[locale];
  if (local?.messages[key] !== undefined) return local.messages[key];
  const en = BUILTIN_CATALOGS.en;
  if (en?.messages[key] !== undefined) return en.messages[key];
  return null;
}
