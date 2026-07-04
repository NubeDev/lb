// The authoring panel — the guided third region of the Playground (rules-editor-ux scope). A tabbed
// surface (Functions | Examples | Data) that turns a blank editor into a discoverable one: browse the
// engine's registered verbs, load a working example, or explore the data a rule can query — each click
// drops a snippet at the editor cursor (Functions/Data) or loads a body (Examples). The data sections
// load lazily on first reveal of the Data tab (honest loading/deny/empty states). One component per
// file (FILE-LAYOUT); the tab bodies live in their own components.

import { useState } from "react";

import { PanelTabs, type PanelTab } from "./PanelTabs";
import { FunctionPalette } from "./FunctionPalette";
import { ExampleList } from "./ExampleList";
import { DataExplorer } from "./DataExplorer";
import { ParamDeclEditor } from "./ParamDeclEditor";
import { useDataExplorer } from "./useDataExplorer";
import type { RuleParam } from "@/lib/rules";

type TabId = "functions" | "examples" | "data" | "params";

const TABS: PanelTab<TabId>[] = [
  { id: "functions", label: "Functions" },
  { id: "examples", label: "Examples" },
  { id: "data", label: "Data" },
  { id: "params", label: "Params" },
];

interface AuthoringPanelProps {
  ws: string;
  /** Insert a snippet at the editor cursor (Functions + Data). */
  onInsert: (snippet: string) => void;
  /** Load an example body into the buffer (Examples) — the parent guards the dirty indicator. */
  onLoadExample: (body: string) => void;
  /** The rule's declared params + their setter (Params) — co-owned with the body by `useRules`. */
  params: RuleParam[];
  onParamsChange: (params: RuleParam[]) => void;
}

/** The Functions | Examples | Data | Params authoring panel. */
export function AuthoringPanel({ ws, onInsert, onLoadExample, params, onParamsChange }: AuthoringPanelProps) {
  const [tab, setTab] = useState<TabId>("functions");

  return (
    <aside
      aria-label="authoring panel"
      className="flex w-72 shrink-0 flex-col border-l border-border bg-card"
    >
      <PanelTabs tabs={TABS} active={tab} onChange={setTab} />
      <div className="min-h-0 flex-1">
        {tab === "functions" ? <FunctionPalette onInsert={onInsert} /> : null}
        {tab === "examples" ? <ExampleList onLoad={onLoadExample} /> : null}
        {/* The Data tab body only mounts on reveal, so the explorer verbs fire then — not on page load. */}
        {tab === "data" ? <DataExplorerTab ws={ws} onInsert={onInsert} /> : null}
        {tab === "params" ? <ParamDeclEditor params={params} onChange={onParamsChange} /> : null}
      </div>
    </aside>
  );
}

/** The Data tab body — its hook fires the explorer verbs on mount (i.e. when the tab is opened). */
function DataExplorerTab({ ws, onInsert }: { ws: string; onInsert: (snippet: string) => void }) {
  const state = useDataExplorer(ws);
  return <DataExplorer state={state} onInsert={onInsert} />;
}
