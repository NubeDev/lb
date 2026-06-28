// Built-in Extension Studio. This is a shell-local view, not an extension, so the chicken-and-egg
// SDK path can scaffold/build/publish the first extension through the real gateway.

import { useEffect, useMemo, useState } from "react";
import { Boxes, FolderOpen, Hammer, PackageCheck, Plus, RefreshCw, Wrench } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  type DevkitFeature,
  type DevkitTier,
  type InspectReport,
  type TemplateInfo,
  buildDevkitExtension,
  inspectDevkitExtension,
  listDevkitTemplates,
  publishDevkitExtension,
  scaffoldDevkitExtension,
} from "@/lib/devkit/devkit.api";
import { openDevkitBuildLog } from "@/lib/devkit/devkit.stream";

interface Props {
  ws: string;
}

type Status = "idle" | "busy" | "ok" | "error";

const FEATURE_LABELS: Record<DevkitFeature, string> = {
  ui: "UI",
  "series-read": "Series read",
  ingest: "Ingest",
  kv: "KV",
};

function defaultId() {
  return `studio-${Math.floor(Date.now() / 1000)}`;
}

export function StudioView({ ws }: Props) {
  const [templates, setTemplates] = useState<TemplateInfo[]>([]);
  const [tier, setTier] = useState<DevkitTier>("wasm");
  const [features, setFeatures] = useState<DevkitFeature[]>(["ui", "series-read"]);
  const [id, setId] = useState(defaultId);
  const [path, setPath] = useState("");
  const [inspect, setInspect] = useState<InspectReport | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [status, setStatus] = useState<Status>("idle");
  const [message, setMessage] = useState("");

  useEffect(() => {
    void listDevkitTemplates()
      .then((rows) => {
        setTemplates(rows);
        const first = rows[0];
        if (first) {
          setTier(first.tier);
          setFeatures(first.features.filter((f) => f === "ui" || f === "series-read"));
        }
      })
      .catch((e) => fail(e));
  }, []);

  const activeTemplate = useMemo(
    () => templates.find((t) => t.tier === tier) ?? templates[0],
    [templates, tier],
  );
  const availableFeatures = activeTemplate?.features ?? [];

  function fail(error: unknown) {
    setStatus("error");
    setMessage(error instanceof Error ? error.message : String(error));
  }

  function toggleFeature(feature: DevkitFeature) {
    setFeatures((current) =>
      current.includes(feature)
        ? current.filter((item) => item !== feature)
        : [...current, feature],
    );
  }

  async function generate() {
    setStatus("busy");
    setMessage("");
    try {
      const report = await scaffoldDevkitExtension({ id, tier, features });
      setPath(report.path);
      const next = await inspectDevkitExtension(report.path);
      setInspect(next);
      setStatus("ok");
      setMessage(`Generated ${report.id}`);
    } catch (e) {
      fail(e);
    }
  }

  async function inspectFolder() {
    if (!path.trim()) return;
    setStatus("busy");
    setMessage("");
    try {
      const next = await inspectDevkitExtension(path.trim());
      setInspect(next);
      setStatus("ok");
      setMessage(`Loaded ${next.id}`);
    } catch (e) {
      fail(e);
    }
  }

  async function build() {
    if (!path.trim()) return;
    setStatus("busy");
    setMessage("");
    setLogs([]);
    try {
      const started = await buildDevkitExtension(path.trim());
      setMessage(started.job_id);
      await waitForBuildLog(started.log_subject, (line) =>
        setLogs((current) => [...current.slice(-199), line]),
      );
      const next = await inspectDevkitExtension(path.trim());
      setInspect(next);
      setStatus(next.built ? "ok" : "error");
      setMessage(next.built ? "Build finished" : "Build did not produce an artifact");
    } catch (e) {
      fail(e);
    }
  }

  async function publish() {
    if (!path.trim()) return;
    setStatus("busy");
    setMessage("");
    try {
      await publishDevkitExtension(path.trim());
      setStatus("ok");
      setMessage("Published");
    } catch (e) {
      fail(e);
    }
  }

  return (
    <div className="flex h-full min-h-0 flex-col bg-bg">
      <AppPageHeader
        icon={Wrench}
        title="Extension Studio"
        description="Scaffold, build, and publish local extensions"
        workspace={ws}
      />
      <main className="grid min-h-0 flex-1 grid-cols-1 gap-4 overflow-auto p-4 lg:grid-cols-[22rem_1fr]">
        <section className="space-y-4">
          <div className="rounded-md border border-border bg-card p-3">
            <div className="mb-3 flex items-center gap-2 text-sm font-medium">
              <Plus size={16} />
              Generate
            </div>
            <label className="space-y-1 text-xs text-muted">
              <span>ID</span>
              <Input value={id} onChange={(e) => setId(e.target.value)} />
            </label>
            <div className="mt-3 grid grid-cols-2 gap-2">
              {(["wasm", "native"] as DevkitTier[]).map((item) => (
                <Button
                  key={item}
                  type="button"
                  variant={tier === item ? "solid" : "outline"}
                  onClick={() => setTier(item)}
                >
                  {item}
                </Button>
              ))}
            </div>
            <div className="mt-3 grid grid-cols-2 gap-2">
              {availableFeatures.map((feature) => (
                <label
                  key={feature}
                  className="flex items-center gap-2 rounded-md border border-border bg-bg px-2 py-1.5 text-xs text-fg"
                >
                  <input
                    type="checkbox"
                    checked={features.includes(feature)}
                    onChange={() => toggleFeature(feature)}
                  />
                  {FEATURE_LABELS[feature]}
                </label>
              ))}
            </div>
            <Button className="mt-3 w-full" onClick={generate} disabled={status === "busy"}>
              <Plus size={16} />
              Generate
            </Button>
          </div>

          <div className="rounded-md border border-border bg-card p-3">
            <div className="mb-3 flex items-center gap-2 text-sm font-medium">
              <FolderOpen size={16} />
              Existing Folder
            </div>
            <Input value={path} onChange={(e) => setPath(e.target.value)} placeholder="rust/extensions/..." />
            <Button className="mt-3 w-full" variant="outline" onClick={inspectFolder}>
              <RefreshCw size={16} />
              Inspect
            </Button>
          </div>

          <div className="grid grid-cols-2 gap-2">
            <Button onClick={build} disabled={!path || status === "busy"}>
              <Hammer size={16} />
              Build
            </Button>
            <Button variant="solid" onClick={publish} disabled={!path || status === "busy"}>
              <PackageCheck size={16} />
              Publish
            </Button>
          </div>
          {message && <StatusMessage status={status} message={message} />}
        </section>

        <section className="grid min-h-0 grid-rows-[auto_1fr] gap-4">
          <InspectPanel inspect={inspect} />
          <div className="min-h-0 rounded-md border border-border bg-card p-3">
            <div className="mb-3 flex items-center gap-2 text-sm font-medium">
              <Boxes size={16} />
              Build Log
            </div>
            <pre className="h-full min-h-[18rem] overflow-auto rounded-md bg-bg p-3 text-xs leading-5 text-fg">
              {logs.length ? logs.join("\n") : "Waiting for build output."}
            </pre>
          </div>
        </section>
      </main>
    </div>
  );
}

function StatusMessage({ status, message }: { status: Status; message: string }) {
  const tone = status === "error" ? "destructive" : status === "ok" ? "default" : "secondary";
  return (
    <Badge variant={tone} className="max-w-full">
      <span className="truncate">{message}</span>
    </Badge>
  );
}

function InspectPanel({ inspect }: { inspect: InspectReport | null }) {
  return (
    <div className="rounded-md border border-border bg-card p-3">
      <div className="mb-3 text-sm font-medium">Inspection</div>
      {!inspect ? (
        <div className="text-xs text-muted">No folder selected.</div>
      ) : (
        <div className="grid gap-3 text-xs">
          <div className="flex flex-wrap gap-2">
            <Badge>{inspect.id}</Badge>
            <Badge variant="secondary">{inspect.tier}</Badge>
            <Badge variant={inspect.built ? "default" : "outline"}>
              {inspect.built ? "built" : "not built"}
            </Badge>
          </div>
          <KeyValues
            rows={[
              ["tools", inspect.tools.join(", ") || "none"],
              ["caps", inspect.caps.join(", ") || "none"],
              [
                "toolchain",
                [
                  inspect.toolchain.cargo ? "cargo" : "cargo missing",
                  inspect.toolchain.pnpm ? "pnpm" : "pnpm missing",
                  inspect.toolchain.wasm32_wasip2 ? "wasm32-wasip2" : "wasm target missing",
                ].join(" · "),
              ],
            ]}
          />
        </div>
      )}
    </div>
  );
}

function KeyValues({ rows }: { rows: [string, string][] }) {
  return (
    <dl className="grid gap-2">
      {rows.map(([key, value]) => (
        <div key={key} className="grid grid-cols-[5rem_1fr] gap-2">
          <dt className="text-muted">{key}</dt>
          <dd className="min-w-0 break-words font-mono text-[11px] text-fg">{value}</dd>
        </div>
      ))}
    </dl>
  );
}

function waitForBuildLog(subject: string, onLine: (line: string) => void) {
  return new Promise<void>((resolve, reject) => {
    let settled = false;
    let stream: { close: () => void } | null = null;
    let timer = 0;
    const finish = (fn: () => void) => {
      if (settled) return;
      settled = true;
      window.clearTimeout(timer);
      stream?.close();
      fn();
    };
    stream = openDevkitBuildLog(subject, (line) => {
      onLine(line);
      if (line === "devkit build: done") {
        finish(resolve);
      }
      if (line === "devkit build: failed") {
        finish(() => reject(new Error("build failed")));
      }
    });
    timer = window.setTimeout(
      () => finish(() => reject(new Error("build timed out"))),
      10 * 60 * 1000,
    );
  });
}
