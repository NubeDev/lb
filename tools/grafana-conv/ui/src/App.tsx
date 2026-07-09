// The one-screen converter UI (grafana-conv scope Stage 0).
// Left: drop/paste a Grafana dashboard JSON. Right: converted output + the report.
// Calls the local Rust seam over `/convert` (proxy in dev; same-origin packaged).

import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/card";
import { Textarea } from "@/components/textarea";
import { Report } from "@/components/report";
import { convert } from "@/lib/convert";
import type { ConvertResponse } from "@/types";

const SAMPLE = `{
  "schemaVersion": 42,
  "title": "Sample",
  "panels": [
    { "type": "timeseries", "gridPos": { "x": 0, "y": 0, "w": 12, "h": 8 }, "targets": [ { "refId": "A" } ] }
  ]
}`;

type State =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string }
  | { kind: "done"; data: ConvertResponse };

export function App() {
  const [input, setInput] = useState<string>("");
  const [state, setState] = useState<State>({ kind: "idle" });

  const run = useCallback(async () => {
    let parsed: unknown;
    try {
      parsed = input.trim() === "" ? {} : JSON.parse(input);
    } catch (e) {
      setState({ kind: "error", message: `invalid JSON: ${fmtErr(e)}` });
      return;
    }
    setState({ kind: "loading" });
    const r = await convert(parsed);
    if (r.ok) {
      setState({ kind: "done", data: r.data });
    } else {
      setState({ kind: "error", message: r.message });
    }
  }, [input]);

  // Drag-drop a `.json` file onto the input pane.
  const onDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault();
    const file = e.dataTransfer.files?.[0];
    if (!file) return;
    const text = await file.text();
    setInput(text);
  }, []);

  useEffect(() => {
    // paste a sample on first mount so the user sees the shape immediately.
    if (input === "") setInput(SAMPLE);
  }, [input]);

  const outputJson =
    state.kind === "done"
      ? JSON.stringify(state.data.dashboard, null, 2)
      : "";

  return (
    <div className="min-h-screen flex flex-col">
      <header className="border-b border-neutral-800 px-6 py-3 flex items-center justify-between">
        <div>
          <h1 className="text-base font-semibold">Grafana → Dashboard converter</h1>
          <p className="text-xs text-neutral-500">
            Paste or drop a Grafana dashboard JSON. Output is our record shape + an honest report.
          </p>
        </div>
      </header>
      <main className="flex-1 grid grid-cols-1 lg:grid-cols-2 gap-4 p-4">
        <Card
          onDrop={onDrop}
          onDragOver={(e) => e.preventDefault()}
          className="flex flex-col min-h-[60vh]"
        >
          <CardHeader className="flex items-center justify-between">
            <CardTitle>Grafana input</CardTitle>
            <div className="flex gap-2">
              <Button variant="ghost" size="sm" onClick={() => setInput(SAMPLE)}>
                Sample
              </Button>
              <Button size="sm" onClick={run} disabled={state.kind === "loading"}>
                Convert
              </Button>
            </div>
          </CardHeader>
          <CardContent className="flex-1 flex">
            <Textarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              spellCheck={false}
              placeholder="Drop a Grafana .json here, or paste it…"
              className="flex-1 min-h-[50vh]"
            />
          </CardContent>
        </Card>

        <div className="flex flex-col gap-4">
          <Card className="flex-1 min-h-[30vh]">
            <CardHeader className="flex items-center justify-between">
              <CardTitle>Output (our Dashboard)</CardTitle>
              {outputJson && (
                <div className="flex gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => navigator.clipboard.writeText(outputJson)}
                  >
                    Copy
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => download("dashboard.json", outputJson)}
                  >
                    Download
                  </Button>
                </div>
              )}
            </CardHeader>
            <CardContent>
              {state.kind === "error" && (
                <p className="text-sm text-rose-400">{state.message}</p>
              )}
              {state.kind === "loading" && (
                <p className="text-sm text-neutral-500">converting…</p>
              )}
              {state.kind === "done" && (
                <pre className="text-xs font-mono whitespace-pre-wrap break-all text-neutral-200 max-h-[40vh] overflow-auto">
                  {outputJson}
                </pre>
              )}
              {state.kind === "idle" && (
                <p className="text-sm text-neutral-500">Output appears here.</p>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Conversion report</CardTitle>
            </CardHeader>
            <CardContent>
              {state.kind === "done" ? (
                <Report report={state.data.report} />
              ) : (
                <p className="text-sm text-neutral-500">
                  Every mapped / degraded / dropped feature is named here — nothing looks silently
                  lost.
                </p>
              )}
            </CardContent>
          </Card>
        </div>
      </main>
    </div>
  );
}

function fmtErr(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function download(name: string, content: string) {
  const blob = new Blob([content], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}
