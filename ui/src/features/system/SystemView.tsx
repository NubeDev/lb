// The System page — the admin, READ-ONLY workspace topology + status console (system-map scope). Two
// views over one snapshot: a status grid (a `Card` per subsystem with its live numbers + health) for
// "is it healthy", and a react-flow topology for "what is connected". Poll-on-open with a manual
// Refresh — honest for a debugging console you open deliberately (no live feed in v1). Obeys the UI
// standard (shadcn-first, `AppPageHeader`, responsive: the grid reflows 1→3 columns and is the primary
// phone surface; the graph degrades to pan/zoom). Layout + wiring only; data lives in `useSystem`.

import { Suspense, lazy, useState, type KeyboardEvent } from "react";
import { Activity, ArrowUpRight, LayoutGrid, Network, Plus, RefreshCw } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { cn } from "@/lib/utils";
import type { CoreSurface } from "@/features/shell";
import { useSystem } from "./useSystem";
import { HEALTH_STYLES, healthRank } from "./health";
import { surfaceForSubsystem } from "./navigate";
import { SubsystemDetailSheet } from "./SubsystemDetailSheet";
import type { ServiceStatus } from "@/lib/system/system.types";

// Code-split the graph (and `@xyflow/react`) so it only loads when the user flips to the topology.
const SystemTopologyGraph = lazy(() => import("./SystemTopologyGraph"));

interface Props {
  ws: string;
  /** Switch the shell to another surface — lets a card drill into the page that owns it (outbox,
   *  extensions, data, …). Omitted in tests that render the page in isolation. */
  onNavigate?: (surface: CoreSurface) => void;
  /** The surfaces the session is allowed to see — a card only links to a page that is actually
   *  reachable (the gateway re-checks regardless). */
  allowedSurfaces?: CoreSurface[];
}

type Mode = "grid" | "graph";

export function SystemView({ ws, onNavigate, allowedSurfaces = [] }: Props) {
  const { overview, topology, error, loading, refresh, loadTopology } = useSystem();
  const [mode, setMode] = useState<Mode>("grid");
  // The subsystem whose detail sheet is open (a no-page card was clicked), or null when closed.
  const [detailId, setDetailId] = useState<string | null>(null);

  // A card drills into the page that owns its subsystem, when that page exists and is allowed.
  const linkFor = (id: string): CoreSurface | null => {
    const surface = surfaceForSubsystem(id);
    if (surface && onNavigate && allowedSurfaces.includes(surface)) return surface;
    return null;
  };

  const showGraph = () => {
    setMode("graph");
    if (!topology) void loadTopology();
  };

  const services = overview
    ? [...overview.services].sort((a, b) => healthRank(a.health) - healthRank(b.health))
    : [];
  const degraded = services.filter((s) => s.health === "degraded" || s.health === "down").length;

  return (
    <section className="flex h-full min-w-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={Activity}
        title="System"
        description="Live workspace topology + subsystem health — the map you open first."
        workspace={ws}
        actions={
          <div className="flex items-center gap-2">
            {overview && (
              <Badge variant="outline" className="hidden gap-1.5 sm:inline-flex">
                <span className="text-muted">role</span>
                <span className="font-medium text-fg">{overview.role}</span>
              </Badge>
            )}
            <div
              className="flex rounded-md border border-border bg-bg p-0.5"
              role="tablist"
              aria-label="view mode"
            >
              <ModeTab mode="grid" active={mode === "grid"} onClick={() => setMode("grid")} />
              <ModeTab mode="graph" active={mode === "graph"} onClick={showGraph} />
            </div>
            <Button
              variant="outline"
              size="sm"
              aria-label="refresh"
              disabled={loading}
              onClick={() => void refresh()}
            >
              <RefreshCw size={14} className={loading ? "animate-spin" : undefined} />
              Refresh
            </Button>
          </div>
        }
      />

      {error && (
        <div
          role="alert"
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-sm text-destructive"
        >
          {error}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-hidden">
        {mode === "graph" ? (
          <Suspense
            fallback={
              <div className="flex h-full items-center justify-center text-sm text-muted">
                Loading topology…
              </div>
            }
          >
            {topology ? (
              <SystemTopologyGraph topology={topology} />
            ) : (
              <div className="flex h-full items-center justify-center text-sm text-muted">
                Reading topology…
              </div>
            )}
          </Suspense>
        ) : (
          <div className="h-full overflow-y-auto p-4">
            {degraded > 0 && (
              <p className="mb-3 text-xs text-muted" aria-label="degraded summary">
                {degraded} subsystem{degraded === 1 ? "" : "s"} want attention.
              </p>
            )}
            {services.length === 0 ? (
              <p className="text-sm text-muted">{loading ? "Reading snapshot…" : "No subsystems."}</p>
            ) : (
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
                {services.map((s) => {
                  // Every card is a control: one with an owning page navigates there (existing
                  // behavior); one without (gateway/bus/mcp) opens the in-page detail sheet instead
                  // of being a dead end.
                  const target = linkFor(s.id);
                  return (
                    <StatusCard
                      key={s.id}
                      service={s}
                      hasPage={target !== null}
                      onOpen={target ? () => onNavigate?.(target) : () => setDetailId(s.id)}
                    />
                  );
                })}
              </div>
            )}
          </div>
        )}
      </div>

      <SubsystemDetailSheet subsystemId={detailId} onClose={() => setDetailId(null)} />
    </section>
  );
}

function ModeTab({ mode, active, onClick }: { mode: Mode; active: boolean; onClick: () => void }) {
  const Icon = mode === "grid" ? LayoutGrid : Network;
  return (
    <Button
      type="button"
      role="tab"
      aria-selected={active}
      variant="ghost"
      size="sm"
      className={cn(
        "h-7 rounded-md px-2.5 text-xs",
        active ? "bg-accent/15 text-accent hover:bg-accent/20" : "text-muted",
      )}
      onClick={onClick}
    >
      <Icon size={14} />
      {mode === "grid" ? "Grid" : "Graph"}
    </Button>
  );
}

/** One subsystem card: label + health dot, the one-line detail, and the live metrics as a flat row.
 *  Every card is a control. `hasPage` cards drill into the page owning the subsystem (the `open <id>`
 *  affordance, an ↗ glyph); the rest open the in-page detail sheet (the `subsystem <id>` affordance, a
 *  + glyph) instead of dead-ending. Both are keyboard-operable with a hover ring. */
function StatusCard({
  service,
  hasPage,
  onOpen,
}: {
  service: ServiceStatus;
  hasPage: boolean;
  onOpen: () => void;
}) {
  const style = HEALTH_STYLES[service.health];
  // Page cards announce as "open <id>" (they navigate away); detail-sheet cards keep "subsystem <id>"
  // (they open a panel in place) — both still clickable.
  const Affordance = hasPage ? ArrowUpRight : Plus;
  return (
    <Card
      className={cn(
        style.border,
        "cursor-pointer transition-colors hover:border-accent/40 hover:bg-accent/5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/25",
      )}
      aria-label={hasPage ? `open ${service.id}` : `subsystem ${service.id}`}
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(e: KeyboardEvent) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onOpen();
        }
      }}
    >
      <CardHeader>
        <div className="flex items-center justify-between gap-2">
          <CardTitle className="flex min-w-0 items-center gap-1.5">
            <span className="truncate">{service.label}</span>
            <Affordance size={13} className="shrink-0 text-muted" aria-hidden />
          </CardTitle>
          <span className="inline-flex shrink-0 items-center gap-1.5">
            <span className={`h-2 w-2 rounded-full ${style.dot}`} aria-hidden />
            <span className={`text-xs font-medium ${style.text}`} aria-label={`health ${style.label}`}>
              {style.label}
            </span>
          </span>
        </div>
        <CardDescription>{service.detail}</CardDescription>
      </CardHeader>
      {service.metrics.length > 0 && (
        <CardContent className="flex flex-wrap gap-2">
          {service.metrics.map((m) => (
            <span
              key={m.label}
              className="inline-flex items-baseline gap-1 rounded-md border border-border bg-bg px-2 py-1 text-xs"
              aria-label={`${service.id} ${m.label}`}
            >
              <span className="text-muted">{m.label}</span>
              <span className="font-medium tabular-nums text-fg">{m.value}</span>
            </span>
          ))}
        </CardContent>
      )}
    </Card>
  );
}
