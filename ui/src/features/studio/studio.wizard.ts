// The Extension Studio state machine — one linear pipeline (create -> build -> publish) modeled as a
// three-step wizard instead of the old all-at-once panel. Owns every devkit side effect (scaffold,
// inspect, build + log stream, publish) and the step gating, so the view components stay presentational.
// Impossible states are unreachable: you can only reach Build once a folder exists, Publish once it built.

import { useEffect, useMemo, useState } from "react";

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

export type StudioStep = 1 | 2 | 3;
export type StudioMode = "new" | "existing";
export type StudioStatus = "idle" | "busy" | "ok" | "error";

function defaultId() {
  return `studio-${Math.floor(Date.now() / 1000)}`;
}

export interface StudioWizard {
  step: StudioStep;
  mode: StudioMode;
  status: StudioStatus;
  message: string;
  busy: boolean;

  templates: TemplateInfo[];
  tier: DevkitTier;
  features: DevkitFeature[];
  availableFeatures: DevkitFeature[];
  id: string;
  path: string;

  inspect: InspectReport | null;
  built: boolean;
  published: boolean;
  logs: string[];

  setMode: (mode: StudioMode) => void;
  setTier: (tier: DevkitTier) => void;
  toggleFeature: (feature: DevkitFeature) => void;
  setId: (id: string) => void;
  setPath: (path: string) => void;

  generate: () => Promise<void>;
  openExisting: () => Promise<void>;
  build: () => Promise<void>;
  publish: () => Promise<void>;

  goTo: (step: StudioStep) => void;
  back: () => void;
  reset: () => void;
}

export function useStudioWizard(): StudioWizard {
  const [step, setStep] = useState<StudioStep>(1);
  const [mode, setMode] = useState<StudioMode>("new");
  const [templates, setTemplates] = useState<TemplateInfo[]>([]);
  const [tier, setTier] = useState<DevkitTier>("wasm");
  const [features, setFeatures] = useState<DevkitFeature[]>([
    "ui",
    "series-read",
  ]);
  const [id, setId] = useState(defaultId);
  const [path, setPath] = useState("");
  const [inspect, setInspect] = useState<InspectReport | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [published, setPublished] = useState(false);
  const [status, setStatus] = useState<StudioStatus>("idle");
  const [message, setMessage] = useState("");

  useEffect(() => {
    void listDevkitTemplates()
      .then((rows) => {
        setTemplates(rows);
        const first = rows[0];
        if (first) {
          setTier(first.tier);
          setFeatures(
            first.features.filter((f) => f === "ui" || f === "series-read"),
          );
        }
      })
      .catch(fail);
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
    setPublished(false);
    setLogs([]);
    try {
      const report = await scaffoldDevkitExtension({ id, tier, features });
      setPath(report.path);
      const next = await inspectDevkitExtension(report.path);
      setInspect(next);
      setStatus("ok");
      setMessage(`Generated ${report.id}`);
      setStep(2);
    } catch (e) {
      fail(e);
    }
  }

  async function openExisting() {
    const trimmed = path.trim();
    if (!trimmed) return;
    setStatus("busy");
    setMessage("");
    setPublished(false);
    setLogs([]);
    try {
      const next = await inspectDevkitExtension(trimmed);
      setInspect(next);
      setPath(trimmed);
      setStatus("ok");
      setMessage(`Loaded ${next.id}`);
      setStep(2);
    } catch (e) {
      fail(e);
    }
  }

  async function build() {
    const trimmed = path.trim();
    if (!trimmed) return;
    setStatus("busy");
    setMessage("");
    setLogs([]);
    try {
      const started = await buildDevkitExtension(trimmed);
      setMessage(started.job_id);
      await waitForBuildLog(started.log_subject, (line) =>
        setLogs((current) => [...current.slice(-199), line]),
      );
      const next = await inspectDevkitExtension(trimmed);
      setInspect(next);
      setStatus(next.built ? "ok" : "error");
      setMessage(
        next.built ? "Build finished" : "Build did not produce an artifact",
      );
    } catch (e) {
      fail(e);
    }
  }

  async function publish() {
    const trimmed = path.trim();
    if (!trimmed) return;
    setStatus("busy");
    setMessage("");
    try {
      await publishDevkitExtension(trimmed);
      setStatus("ok");
      setMessage("Published");
      setPublished(true);
    } catch (e) {
      fail(e);
    }
  }

  function goTo(next: StudioStep) {
    setStatus("idle");
    setMessage("");
    setStep(next);
  }

  function back() {
    setStatus("idle");
    setMessage("");
    setStep((current) =>
      current > 1 ? ((current - 1) as StudioStep) : current,
    );
  }

  function reset() {
    setStep(1);
    setId(defaultId());
    setPath("");
    setInspect(null);
    setLogs([]);
    setPublished(false);
    setStatus("idle");
    setMessage("");
  }

  return {
    step,
    mode,
    status,
    message,
    busy: status === "busy",
    templates,
    tier,
    features,
    availableFeatures,
    id,
    path,
    inspect,
    built: inspect?.built ?? false,
    published,
    logs,
    setMode,
    setTier,
    toggleFeature,
    setId,
    setPath,
    generate,
    openExisting,
    build,
    publish,
    goTo,
    back,
    reset,
  };
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
      if (line === "devkit build: done") finish(resolve);
      if (line === "devkit build: failed")
        finish(() => reject(new Error("build failed")));
    });
    timer = window.setTimeout(
      () => finish(() => reject(new Error("build timed out"))),
      10 * 60 * 1000,
    );
  });
}
