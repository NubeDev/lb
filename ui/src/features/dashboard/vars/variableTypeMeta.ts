// The variable-type catalog (advanced-variables scope) — one entry per `VariableType`: an icon, a title,
// and a one-line description shown in the type picker (the friendly "choose variable type" step). Pure
// data, no React, so the picker and any future help surface read the SAME source. FILE-LAYOUT: the
// catalog is a fact, the picker is a view.

import type { LucideIcon } from "lucide-react";
import { Database, List, TextCursorInput, Lock, Clock, Search, Server } from "lucide-react";

import type { VariableType } from "@/lib/vars";

export interface VariableTypeMeta {
  type: VariableType;
  /** The picker title (Grafana-parity display name — friendlier than the raw kind). */
  title: string;
  /** One line: what the variable's values ARE and where they come from. */
  description: string;
  icon: LucideIcon;
}

/** The catalog, in the order the picker lists them (most-reached first). Every `VariableType` appears
 *  exactly once — a new type is added here, not branched on elsewhere. */
export const VARIABLE_TYPES: VariableTypeMeta[] = [
  {
    type: "query",
    title: "Query",
    description: "Values come from a read source's rows — a live, workspace-scoped option list.",
    icon: Database,
  },
  {
    type: "custom",
    title: "Custom",
    description: "A fixed list you define by hand. Use text : value to show a label but interpolate a value.",
    icon: List,
  },
  {
    type: "text",
    title: "Textbox",
    description: "A free-text box on the bar — the viewer types any value.",
    icon: TextCursorInput,
  },
  {
    type: "const",
    title: "Constant",
    description: "A hidden fixed value. Handy for a shared dashboard you don't want editable on the bar.",
    icon: Lock,
  },
  {
    type: "interval",
    title: "Interval",
    description: "A list of time spans (1m, 5m, 1h) that feeds $__interval.",
    icon: Clock,
  },
  {
    type: "source",
    title: "Source",
    description: "Resolve options from an extension or series source under the viewer's caps.",
    icon: Search,
  },
  {
    type: "datasource",
    title: "Data source",
    description: "Pick a registered datasource by variable to switch it across panels.",
    icon: Server,
  },
];

/** Look up one type's metadata (falls back to a minimal record for an unknown/legacy kind). */
export function variableTypeMeta(type: VariableType): VariableTypeMeta {
  return VARIABLE_TYPES.find((t) => t.type === type) ?? { type, title: type, description: "", icon: List };
}
