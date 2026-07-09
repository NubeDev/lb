// Apply a variable's `regex` to its resolved options (advanced-variables scope). Rows that don't match
// are DROPPED; a match with `(?<text>)`/`(?<value>)` named capture groups splits display text from the
// interpolated value; a plain match with no named groups keeps the option (filter-only). Pure TS.
//
// Regex is user input (scope risk): a pathological pattern can ReDoS. We bound it — the pattern is
// length-capped and each input string is length-capped before matching. On an invalid pattern we return
// the options UNCHANGED (fail honestly, don't drop everything) — the bar surfaces the raw list.

import type { RegexApplyTo, VariableOption } from "./types";

/** Bounds (ReDoS mitigation): a too-long pattern or input is skipped rather than risked. */
const MAX_PATTERN = 1000;
const MAX_INPUT = 10_000;

/** Compile a `/pattern/flags` or bare `pattern` string; `null` if invalid/too long. */
export function compileRegex(raw: string): RegExp | null {
  if (!raw || raw.length > MAX_PATTERN) return null;
  // Accept Grafana's `/pattern/flags` literal form as well as a bare pattern.
  const lit = /^\/(.*)\/([a-z]*)$/s.exec(raw);
  const source = lit ? lit[1] : raw;
  const flags = lit ? lit[2] : "";
  try {
    return new RegExp(source, flags);
  } catch {
    return null;
  }
}

/** Apply `regex` (against value|text) to `options`. Filters non-matches; splits text/value on named
 *  capture groups; returns the input unchanged if the pattern is invalid. */
export function applyRegex(
  options: VariableOption[],
  regex: string | undefined,
  applyTo: RegexApplyTo = "value",
): VariableOption[] {
  if (!regex) return options;
  const re = compileRegex(regex);
  if (!re) return options; // fail honestly — don't silently drop every option
  const out: VariableOption[] = [];
  for (const opt of options) {
    const subject = applyTo === "text" ? opt.text : opt.value;
    if (subject.length > MAX_INPUT) continue;
    re.lastIndex = 0;
    const m = re.exec(subject);
    if (!m) continue; // no match → dropped
    const g = m.groups;
    if (g && (g.text !== undefined || g.value !== undefined)) {
      const value = g.value ?? g.text ?? m[0];
      const text = g.text ?? g.value ?? m[0];
      out.push({ ...opt, text, value });
    } else if (m[1] !== undefined) {
      // A single unnamed capture group is Grafana's value (and text).
      out.push({ ...opt, text: m[1], value: m[1] });
    } else {
      // Filter-only match: keep the option as-is.
      out.push(opt);
    }
  }
  return out;
}
