// The telemetry console (telemetry-console scope): the in-browser viewer over the workspace's capped
// telemetry ring + the audit lane. The headline observability surface: filter by source/actor/level/
// outcome/trace_id/text, toggle a live tail, click a trace_id to pivot to its correlated timeline.
// Filters are URL-encoded (shareable). The workspace wall + cap gate are server-side; a deny surfaces
// here as a labelled error, never as fabricated rows (CLAUDE §9).
//
// Data/state lives in `useTelemetry`; this file is the markup + the lane/URL wiring.

import { useCallback, useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { CAP, hasCap } from "@/lib/session";
import { useSession } from "@/lib/session/useSession";
import type { TelemetryLane } from "@/lib/telemetry";

import { AuditLane } from "./AuditLane";
import { decodeFilterFromQuery, encodeFilterToQuery } from "./filterUrl";
import { TelemetryFilterBar } from "./TelemetryFilterBar";
import { TelemetryList } from "./TelemetryList";
import { useTelemetry } from "./useTelemetry";

/** Read the initial filter from the URL hash query (`#/t/ws/telemetry?source=mqtt&level=warn`), so a
 *  shared/deep link restores the view. The hash carries the route + the query after `?`. */
function filterFromUrl() {
  const hash = typeof window !== "undefined" ? window.location.hash : "";
  const qIndex = hash.indexOf("?");
  return qIndex >= 0 ? decodeFilterFromQuery(hash.slice(qIndex + 1)) : {};
}

export function TelemetryView() {
  const { session } = useSession();
  const caps = session?.caps;
  const canAudit = hasCap(caps, CAP.auditQuery);

  const [lane, setLane] = useState<TelemetryLane>("telemetry");
  const tel = useTelemetry(filterFromUrl());

  // Reflect the active filter into the URL query (shareable) without adding history entries.
  useEffect(() => {
    if (typeof window === "undefined") return;
    const query = encodeFilterToQuery(tel.filter);
    const base = window.location.hash.split("?")[0];
    const next = query ? `${base}?${query}` : base;
    if (next !== window.location.hash) {
      window.history.replaceState(null, "", next);
    }
  }, [tel.filter]);

  const onPivot = useCallback((traceId: string) => void tel.pivotToTrace(traceId), [tel]);

  return (
    <div className="flex h-full flex-col gap-3 p-4">
      <header className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold">Telemetry</h1>
          <p className="text-xs text-muted-foreground">
            Recent operational events for this workspace — bounded (the ring evicts), not the
            mutation record. For long retention, export to a collector.
          </p>
        </div>
        <LaneTabs lane={lane} onLane={setLane} canAudit={canAudit} />
      </header>

      {lane === "telemetry" ? (
        <>
          <TelemetryFilterBar
            filter={tel.filter}
            onChange={tel.setFilter}
            live={tel.live}
            onLive={tel.setLive}
          />

          {tel.pivotTrace && (
            <div className="flex items-center gap-2 text-sm">
              <span className="text-muted-foreground">
                Trace <code className="rounded-md bg-muted-bg px-1 text-fg">{tel.pivotTrace}</code>
              </span>
              <Button variant="outline" size="sm" onClick={tel.clearPivot}>
                ← back to all events
              </Button>
            </div>
          )}

          {tel.error ? (
            <div className="rounded-md border border-red-500/30 bg-red-500/5 p-4 text-sm text-red-500">
              {/* Opaque deny / read error — never fabricated rows. */}
              Could not read telemetry: {tel.error}
            </div>
          ) : (
            <div className="min-h-0 flex-1 overflow-auto">
              <TelemetryList rows={tel.rows} onPivotTrace={onPivot} />
            </div>
          )}
        </>
      ) : (
        <div className="min-h-0 flex-1 overflow-auto">
          <AuditLane hasAuditGrant={canAudit} />
        </div>
      )}
    </div>
  );
}

function LaneTabs({
  lane,
  onLane,
  canAudit,
}: {
  lane: TelemetryLane;
  onLane: (l: TelemetryLane) => void;
  canAudit: boolean;
}) {
  return (
    <div className="flex gap-1 rounded-md border border-border p-0.5">
      <LaneButton active={lane === "telemetry"} onClick={() => onLane("telemetry")}>
        Telemetry
      </LaneButton>
      <LaneButton active={lane === "audit"} onClick={() => onLane("audit")}>
        Audit{!canAudit && " 🔒"}
      </LaneButton>
    </div>
  );
}

function LaneButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Button
      variant="ghost"
      size="sm"
      onClick={onClick}
      className={`h-auto rounded-md px-3 py-1 text-sm ${
        active
          ? "bg-accent text-accent-foreground"
          : "text-muted-foreground hover:text-foreground"
      }`}
    >
      {children}
    </Button>
  );
}
