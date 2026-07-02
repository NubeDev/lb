// The unified extensions console (admin-console + lifecycle-management scopes) — the real console
// that SUPERSEDES the demo RegistryView + NativeView. Lists installed extensions across BOTH tiers
// (wasm/native) with tier · version · enabled · running · health · restart count; start/stop via
// enable/disable; uninstall (binary eviction). Install-from-catalog / upload is optional this pass
// (deferred — the scope allows it). Every disable/uninstall routes through the shared
// ConfirmDestructive. Markup + wiring only; data lives in useExtensions. The gateway re-checks every
// verb; the UI gate is convenience, not the boundary.

import { useState } from "react";
import {
  Boxes,
  MonitorCog,
  Package,
  Puzzle,
  ShieldAlert,
  ShieldCheck,
  type LucideIcon,
} from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { ConfirmDestructive } from "@/features/confirm";
import type { ExtRow } from "@/lib/ext/ext.api";
import { useExtensions } from "./useExtensions";
import { UploadArtifact } from "./UploadArtifact";

type Pending = { kind: "disable" | "uninstall"; ext: string } | null;

interface Props {
  ws: string;
}

const SURFACE_PREVIEW_COUNT = 4;
const SCOPE_RESULT_LIMIT = 100;

const ICONS: Record<string, LucideIcon> = {
  "shield-check": ShieldCheck,
};

type Surface = {
  key: string;
  kind: "Page" | "Widget";
  label: string;
  icon: string;
  scope: string[];
};

function stateTone(row: ExtRow) {
  if (!row.enabled || row.health === "disabled") {
    return "border-border bg-bg text-muted";
  }
  if (row.running && row.health === "ok") {
    return "border-accent/25 bg-accent/10 text-accent";
  }
  return "border-destructive/25 bg-destructive/10 text-destructive";
}

function ExtensionIcon({ icon, label }: { icon: string; label: string }) {
  const Icon = ICONS[icon] ?? Puzzle;

  return (
    <div
      aria-label={`${label} icon`}
      className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-border bg-bg text-accent"
      title={icon}
    >
      <Icon size={16} />
    </div>
  );
}

function collectSurfaces(row: ExtRow): Surface[] {
  return [
    ...(row.ui
      ? [
          {
            key: `page:${row.ui.label}`,
            kind: "Page" as const,
            label: row.ui.label,
            icon: row.ui.icon,
            scope: row.ui.scope,
          },
        ]
      : []),
    ...(row.widgets ?? []).map((widget, index) => ({
      key: `widget:${widget.label}:${index}`,
      kind: "Widget" as const,
      label: widget.label,
      icon: widget.icon,
      scope: widget.scope,
    })),
  ];
}

function collectScope(surfaces: Surface[]) {
  const seen = new Set<string>();
  const scope: string[] = [];

  for (const surface of surfaces) {
    for (const item of surface.scope) {
      if (!seen.has(item)) {
        seen.add(item);
        scope.push(item);
      }
    }
  }

  return scope;
}

function SurfacesSummary({ surfaces }: { surfaces: Surface[] }) {
  const [expanded, setExpanded] = useState(false);
  const visible = expanded ? surfaces : surfaces.slice(0, SURFACE_PREVIEW_COUNT);
  const hidden = Math.max(0, surfaces.length - visible.length);

  if (surfaces.length === 0) {
    return (
      <div className="border-t border-border pt-4 text-xs text-muted">
        No UI surfaces declared.
      </div>
    );
  }

  return (
    <div className="border-t border-border pt-4">
      <div className="flex items-center gap-2 text-xs font-medium text-fg">
        <MonitorCog size={14} className="text-accent" />
        <span>Surfaces</span>
        <Badge variant="outline" className="ml-auto shrink-0">
          {surfaces.length}
        </Badge>
      </div>

      <div className={`mt-3 grid gap-2 ${expanded ? "max-h-52 overflow-y-auto pr-1" : ""}`}>
        {visible.map((surface) => (
          <div key={surface.key} className="flex min-w-0 items-center gap-2 text-xs">
            <ExtensionIcon icon={surface.icon} label={surface.label} />
            <span className="min-w-0 flex-1 truncate font-medium text-fg">{surface.label}</span>
            <Badge variant="secondary">{surface.kind}</Badge>
            <span className="shrink-0 text-muted">{surface.scope.length} verbs</span>
          </div>
        ))}
      </div>

      {(hidden > 0 || expanded) && (
        <div className="mt-2 flex items-center gap-2">
          {!expanded && hidden > 0 && (
            <span className="text-xs text-muted">{hidden} more surfaces hidden</span>
          )}
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="ml-auto h-7 px-2 text-xs text-muted hover:text-fg"
            onClick={() => setExpanded((value) => !value)}
          >
            {expanded ? "Show fewer" : "Show all"}
          </Button>
        </div>
      )}
    </div>
  );
}

function ScopeInspector({ scope }: { scope: string[] }) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const normalizedQuery = query.trim().toLowerCase();
  const filtered = normalizedQuery
    ? scope.filter((item) => item.toLowerCase().includes(normalizedQuery))
    : scope;
  const visible = filtered.slice(0, SCOPE_RESULT_LIMIT);
  const clipped = Math.max(0, filtered.length - visible.length);

  return (
    <div className="min-w-0">
      <div className="flex items-center gap-2 text-xs font-medium text-fg">
        <ShieldCheck size={14} className="text-accent" />
        <span>Bridge scope</span>
        <Badge variant="outline" className="ml-auto shrink-0">
          {scope.length} verbs
        </Badge>
        {scope.length > 0 && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs text-muted hover:text-fg"
            onClick={() => setOpen((value) => !value)}
          >
            {open ? "Close" : "Inspect"}
          </Button>
        )}
      </div>

      {scope.length === 0 ? (
        <p className="mt-3 text-xs text-muted">No bridge verbs declared.</p>
      ) : (
        open && (
          <div className="mt-3 space-y-2">
            <Input
              aria-label="filter bridge scope"
              className="h-8 text-xs"
              placeholder="Filter verbs"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
            />
            <div className="max-h-56 overflow-y-auto pr-1">
              {visible.length === 0 ? (
                <p className="text-xs text-muted">No verbs match this filter.</p>
              ) : (
                <ul className="grid gap-1 sm:grid-cols-2 xl:grid-cols-3">
                  {visible.map((item) => (
                    <li
                      key={item}
                      className="min-w-0 truncate rounded-md border border-border bg-bg px-2 py-1 font-mono text-xs text-muted"
                      title={item}
                    >
                      {item}
                    </li>
                  ))}
                </ul>
              )}
            </div>
            <p className="text-xs text-muted">
              Showing {visible.length} of {filtered.length}
              {clipped > 0 ? `; narrow the filter to see ${clipped} more` : ""}.
            </p>
          </div>
        )
      )}
    </div>
  );
}

function ExtensionSupport({ row }: { row: ExtRow }) {
  const surfaces = collectSurfaces(row);
  const scope = collectScope(surfaces);
  const scoped = scope.length > 0;
  const SecurityIcon = scoped ? ShieldCheck : ShieldAlert;
  const bridgeLabel = surfaces.length === 0 ? "No bridge" : scoped ? "Scoped bridge" : "Unscoped bridge";
  const bridgeTone =
    surfaces.length === 0
      ? "gap-1 border-border bg-bg text-muted"
      : scoped
        ? "gap-1 border-accent/25 bg-accent/10 text-accent"
        : "gap-1 border-destructive/25 bg-destructive/10 text-destructive";

  return (
    <div className="grid gap-4 border-t border-border pt-4 md:grid-cols-[minmax(13rem,0.7fr)_minmax(0,1.3fr)]">
      <div className="min-w-0">
        <div className="flex items-start gap-3">
          <ExtensionIcon icon={row.ui?.icon ?? row.widgets?.[0]?.icon ?? ""} label={row.ext} />
          <div className="min-w-0">
            <div className="flex min-w-0 flex-wrap items-center gap-2 text-xs font-medium text-fg">
              <span className="truncate">{surfaces.length > 0 ? "UI supported" : "No UI surfaces"}</span>
              <Badge variant="outline" className="gap-1">
                <MonitorCog size={12} />
                {surfaces.length} surfaces
              </Badge>
            </div>
            <div className="mt-2 flex flex-wrap items-center gap-2 text-xs text-muted">
              <Badge
                variant="outline"
                className={bridgeTone}
              >
                <SecurityIcon size={12} />
                {bridgeLabel}
              </Badge>
              <span>{scope.length} unique verbs</span>
            </div>
          </div>
        </div>
      </div>

      <ScopeInspector scope={scope} />
    </div>
  );
}

export function ExtensionsView({ ws }: Props) {
  const { rows, error, setEnabled, reset, uninstall, upload } = useExtensions();
  const [pending, setPending] = useState<Pending>(null);

  return (
    <section className="flex h-full min-w-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={Boxes}
        title="Extensions"
        description="Installed extension tiers, health, and lifecycle actions."
        workspace={ws}
        actions={<UploadArtifact onUpload={upload} />}
      />

      {error && (
        <div
          role="alert"
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-sm text-destructive"
        >
          {error}
        </div>
      )}

      <ul className="min-h-0 flex-1 space-y-3 overflow-y-auto p-4">
        {rows.length === 0 ? (
          <li>
            <Card>
              <CardContent className="p-4 text-sm text-muted">No extensions installed.</CardContent>
            </Card>
          </li>
        ) : (
          rows.map((r) => (
            <li key={r.ext} role="listitem">
              <Card>
                <CardContent className="space-y-4 p-4">
                  <div className="flex flex-col gap-3 md:flex-row md:items-start">
                    <div className="min-w-0 flex-1">
                      <div className="flex min-w-0 flex-wrap items-center gap-2">
                        <Package size={15} className="text-accent" />
                        <h2 className="truncate text-sm font-semibold text-fg">
                          {r.ext}@{r.version}
                        </h2>
                        <Badge variant="secondary">{r.tier}</Badge>
                        <Badge variant="outline" className={stateTone(r)}>
                          {r.health}
                        </Badge>
                      </div>

                      <dl className="mt-3 flex flex-wrap gap-x-5 gap-y-2 text-xs">
                        <div className="flex items-center gap-1.5">
                          <dt className="text-muted">Intent</dt>
                          <dd className="font-medium text-fg">{r.enabled ? "enabled" : "off"}</dd>
                        </div>
                        <div className="flex items-center gap-1.5">
                          <dt className="text-muted">Runtime</dt>
                          <dd className="font-medium text-fg">{r.running ? "running" : "stopped"}</dd>
                        </div>
                        <div className="flex items-center gap-1.5" data-testid={`restarts-${r.ext}`}>
                          <dt className="sr-only">Restarts</dt>
                          <dd className="font-medium text-fg">
                            <span className="text-muted">restarts</span> {r.restart_count}
                          </dd>
                        </div>
                        <div className="flex items-center gap-1.5">
                          <dt className="text-muted">Surfaces</dt>
                          <dd className="font-medium text-fg">{collectSurfaces(r).length}</dd>
                        </div>
                      </dl>
                    </div>

                    <div className="flex shrink-0 items-center gap-2 md:ml-auto">
                      <Button
                        aria-label={`${r.enabled ? "stop" : "start"} ${r.ext}`}
                        variant="outline"
                        size="sm"
                        onClick={() =>
                          r.enabled
                            ? setPending({ kind: "disable", ext: r.ext })
                            : void setEnabled(r.ext, true)
                        }
                      >
                        {r.enabled ? "Stop" : "Start"}
                      </Button>
                      {r.tier === "native" && r.restart_count > 0 && (
                        <Button
                          aria-label={`reset ${r.ext}`}
                          variant="outline"
                          size="sm"
                          title="Re-arm the restart budget and force a fresh child (recovers a sidecar that exhausted its restart budget)"
                          onClick={() => void reset(r.ext)}
                        >
                          Reset
                        </Button>
                      )}
                      <Button
                        aria-label={`uninstall ${r.ext}`}
                        variant="destructive"
                        size="sm"
                        onClick={() => setPending({ kind: "uninstall", ext: r.ext })}
                      >
                        Uninstall
                      </Button>
                    </div>
                  </div>

                  <ExtensionSupport row={r} />
                  <SurfacesSummary surfaces={collectSurfaces(r)} />
                </CardContent>
              </Card>
            </li>
          ))
        )}
      </ul>

      {pending?.kind === "disable" && (
        <ConfirmDestructive
          title={`Stop ${pending.ext}`}
          consequence={`Disables ${pending.ext}: a running native child is stopped and the boot reconciler will NOT auto-start it. The install stays — re-enable to start again. Reversible.`}
          reversible
          escalation="none"
          confirmLabel="Stop"
          onConfirm={() => {
            void setEnabled(pending.ext, false);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
      {pending?.kind === "uninstall" && (
        <ConfirmDestructive
          title={`Uninstall ${pending.ext}`}
          consequence={`Stops any running child, tombstones the install record, and evicts the cached binary. The extension disappears from this workspace; reinstall from the registry to restore it.`}
          reversible={false}
          escalation="second-gate"
          confirmLabel="Uninstall"
          onConfirm={() => {
            void uninstall(pending.ext);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
    </section>
  );
}
