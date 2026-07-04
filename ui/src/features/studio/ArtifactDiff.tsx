// The verdict panel — the loud, unmissable answer to "did the build actually work?". A full-width
// PASS (green) / FAIL (red) banner, then per-artifact old -> new size + delta and whether this run
// rewrote it (mtime advanced). Always shown after a build, pass OR fail, so the user never has to
// read the log to know the outcome. PASS = every binary/component was freshly written.

import {
  ArrowRight,
  CheckCircle2,
  FileCode2,
  Sparkles,
  XCircle,
} from "lucide-react";

import { cn } from "@/lib/utils";
import {
  type ArtifactDelta,
  buildConfirmed,
  formatBytes,
  formatDelta,
} from "@/lib/devkit/artifact-diff";

const KIND_LABEL: Record<string, string> = {
  "native-bin": "native binary",
  wasm: "wasm component",
  "remote-entry": "UI remote",
};

export function ArtifactDiff({ deltas }: { deltas: ArtifactDelta[] }) {
  const pass = deltas.length > 0 && buildConfirmed(deltas);

  return (
    <div
      className={cn(
        "shrink-0 overflow-hidden rounded-xl border-2",
        pass
          ? "border-emerald-500/60 bg-emerald-500/10"
          : "border-rose-500/60 bg-rose-500/10",
      )}
    >
      <Banner pass={pass} deltas={deltas} />

      {deltas.length > 0 && (
        <div className="grid gap-1.5 border-t border-border/40 bg-bg/50 p-3">
          {deltas.map((d) => (
            <ArtifactRow key={d.path} delta={d} />
          ))}
        </div>
      )}
    </div>
  );
}

function Banner({ pass, deltas }: { pass: boolean; deltas: ArtifactDelta[] }) {
  const reason = pass ? confirmedReason(deltas) : failReason(deltas);

  return (
    <div className="flex items-center gap-3 px-4 py-3">
      {pass ? (
        <CheckCircle2 size={32} className="shrink-0 text-emerald-500" />
      ) : (
        <XCircle size={32} className="shrink-0 text-rose-500" />
      )}
      <div className="min-w-0">
        <div
          className={cn(
            "text-lg font-bold leading-tight",
            pass
              ? "text-emerald-700 dark:text-emerald-300"
              : "text-rose-700 dark:text-rose-300",
          )}
        >
          {pass ? "BUILD VERIFIED" : "BUILD NOT VERIFIED"}
        </div>
        <div
          className={cn(
            "text-xs",
            pass
              ? "text-emerald-700/80 dark:text-emerald-300/80"
              : "text-rose-700/80 dark:text-rose-300/80",
          )}
        >
          {reason}
        </div>
      </div>
    </div>
  );
}

function confirmedReason(deltas: ArtifactDelta[]): string {
  const bin = deltas.find((d) => d.kind === "wasm" || d.kind === "native-bin");
  if (bin?.bytesDelta != null && bin.bytesDelta !== 0) {
    return `The ${KIND_LABEL[bin.kind]} was freshly written (${formatDelta(bin.bytesDelta)}). Every binary is up to date.`;
  }
  return "Every binary was freshly written this build.";
}

function failReason(deltas: ArtifactDelta[]): string {
  if (deltas.length === 0) {
    return "No build artifacts were found on disk — the build produced no output.";
  }
  if (deltas.some((d) => d.change === "missing")) {
    return "An artifact went missing — the build failed partway through.";
  }
  const binaries = deltas.filter(
    (d) => d.kind === "wasm" || d.kind === "native-bin",
  );
  if (binaries.length === 0) {
    return "Only a UI remote was produced — no binary/component was built.";
  }
  if (binaries.some((d) => d.change === "unchanged")) {
    return "A binary was NOT rewritten (same timestamp) — this build likely did nothing.";
  }
  return "The build did not produce a verified artifact.";
}

function ArtifactRow({ delta }: { delta: ArtifactDelta }) {
  const name = delta.path.split("/").pop() ?? delta.path;
  const fresh = delta.change === "new" || delta.change === "rebuilt";

  return (
    <div className="flex flex-wrap items-center gap-x-2 gap-y-1 rounded-md border border-border/60 bg-panel/50 px-2.5 py-1.5 text-[11px]">
      <FileCode2 size={12} className="shrink-0 text-muted" />
      <span className="font-mono text-fg">{name}</span>
      <span className="rounded-md bg-border/50 px-1.5 py-0 text-[10px] text-muted">
        {KIND_LABEL[delta.kind] ?? delta.kind}
      </span>

      <span className="ml-auto flex items-center gap-1.5 font-mono text-muted">
        {delta.before !== null && (
          <>
            <span>{formatBytes(delta.before)}</span>
            <ArrowRight size={10} className="shrink-0" />
          </>
        )}
        <span className="font-semibold text-fg">
          {delta.after !== null ? formatBytes(delta.after) : "gone"}
        </span>
        {delta.bytesDelta !== null && delta.bytesDelta !== 0 && (
          <span
            className={cn(
              delta.bytesDelta > 0
                ? "text-emerald-600 dark:text-emerald-400"
                : "text-rose-600 dark:text-rose-400",
            )}
          >
            {formatDelta(delta.bytesDelta)}
          </span>
        )}
      </span>

      <ChangeBadge change={delta.change} fresh={fresh} />
    </div>
  );
}

function ChangeBadge({
  change,
  fresh,
}: {
  change: ArtifactDelta["change"];
  fresh: boolean;
}) {
  return (
    <span
      className={cn(
        "flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
        fresh
          ? "bg-emerald-500/20 text-emerald-700 dark:text-emerald-300"
          : "bg-rose-500/20 text-rose-700 dark:text-rose-300",
      )}
    >
      {change === "new" && <Sparkles size={9} />}
      {change}
    </span>
  );
}
