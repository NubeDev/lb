// A read of what's in the selected folder: identity, capabilities, and — surfaced up front — whether
// the local toolchain can actually build it. Missing cargo / pnpm / the wasm target is the single most
// common reason a build fails, so we warn here (before the user spends minutes on it), not after.

import { Boxes, CircleCheck, TriangleAlert, Wrench } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { InspectReport } from "@/lib/devkit/devkit.api";

export function InspectionSummary({ inspect }: { inspect: InspectReport }) {
  const chain = inspect.toolchain;
  const missing = [
    !chain.cargo && "cargo",
    !chain.pnpm && "pnpm",
    inspect.tier === "wasm" && !chain.wasm32_wasip2 && "wasm32-wasip2 target",
  ].filter(Boolean) as string[];

  return (
    <div className="rounded-lg border border-border bg-bg/60 p-4">
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant="secondary" className="gap-1.5 font-mono">
          <Boxes size={12} className="text-accent" />
          {inspect.id}
        </Badge>
        <Badge variant="outline">{inspect.tier}</Badge>
        <Badge variant={inspect.built ? "default" : "outline"}>
          {inspect.built ? "built" : "not built yet"}
        </Badge>
      </div>

      <dl className="mt-3 grid gap-2 text-xs">
        <Row
          label="Tools"
          value={inspect.tools.join(", ") || "none declared"}
        />
        <Row label="Caps" value={inspect.caps.join(", ") || "none declared"} />
      </dl>

      <div
        className={cn(
          "mt-3 flex items-start gap-2 rounded-md border px-3 py-2 text-xs",
          missing.length
            ? "border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300"
            : "border-border bg-panel/60 text-muted",
        )}
      >
        {missing.length ? (
          <TriangleAlert size={14} className="mt-px shrink-0" />
        ) : (
          <CircleCheck size={14} className="mt-px shrink-0 text-accent" />
        )}
        <span className="min-w-0">
          {missing.length ? (
            <>
              Toolchain incomplete —{" "}
              <span className="font-medium">{missing.join(", ")}</span> not
              found. The build will fail until it's installed.
            </>
          ) : (
            <>
              <Wrench size={11} className="mr-1 inline align-[-1px]" />
              Toolchain ready — cargo, pnpm
              {inspect.tier === "wasm" && ", wasm32-wasip2"} present.
            </>
          )}
        </span>
      </div>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[3.5rem_1fr] items-baseline gap-2">
      <dt className="text-muted">{label}</dt>
      <dd className="min-w-0 break-words font-mono text-[11px] text-fg">
        {value}
      </dd>
    </div>
  );
}
