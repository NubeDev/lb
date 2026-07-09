// A one-line summary of a variable for the collapsed row (advanced-variables scope). Pure — the row view
// reads it to show "$site · Query · multi" without expanding. FILE-LAYOUT: the summary is a fact.

import type { Variable } from "@/lib/vars";
import { variableTypeMeta } from "./variableTypeMeta";

/** Short, human tags describing a variable's selection affordances (multi / all / required / hidden). */
export function variableTags(v: Variable): string[] {
  const tags: string[] = [];
  if (v.multi) tags.push("multi");
  if (v.includeAll) tags.push("all");
  if (v.required) tags.push("required");
  if (v.hide === "hideVariable") tags.push("hidden");
  return tags;
}

/** The collapsed-row summary: `<Type> · multi · all`. */
export function variableSummary(v: Variable): string {
  return [variableTypeMeta(v.type).title, ...variableTags(v)].join(" · ");
}
