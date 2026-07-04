// The subsystem detail surface (system-map scope) — a shadcn `Sheet` (side drawer) that opens when a
// status card with no owning page (gateway/bus/mcp) is clicked, so the map is never a dead end. Shows
// the subsystem's health, one-line detail, every metric, and — for the Zenoh `bus` — the live peer +
// router zid lists (the detail behind the counts). Loads `system.subsystem` on open (read-only,
// admin-gated server-side). Layout + wiring only; the fetch lives in `useSubsystemDetail`.

import { useSubsystemDetail } from "./useSubsystemDetail";
import { HEALTH_STYLES } from "./health";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";

interface Props {
  /** The subsystem id to detail, or `null` when the sheet is closed. */
  subsystemId: string | null;
  /** Close the sheet (clear the selection). */
  onClose: () => void;
}

/** The slide-over detail for one subsystem. Open iff `subsystemId` is set. */
export function SubsystemDetailSheet({ subsystemId, onClose }: Props) {
  const { detail, error, loading } = useSubsystemDetail(subsystemId);

  return (
    <Sheet open={subsystemId !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent
        className="w-full max-w-md overflow-y-auto sm:max-w-md"
        aria-label={subsystemId ? `${subsystemId} detail` : "subsystem detail"}
      >
        {subsystemId && (
          <>
            <SheetHeader>
              <SheetTitle className="flex items-center gap-2">
                <span className="truncate">{detail?.service.label ?? subsystemId}</span>
                {detail && <HealthDot health={detail.service.health} />}
              </SheetTitle>
              <SheetDescription>
                {detail?.service.detail ?? (loading ? "Reading subsystem…" : "")}
              </SheetDescription>
            </SheetHeader>

            <div className="space-y-5 px-4 pb-6">
              {error && (
                <p role="alert" className="text-sm text-destructive">
                  {error}
                </p>
              )}

              {detail && (
                <>
                  <DetailRow label="subsystem">
                    <span className="font-mono text-xs text-fg">{detail.service.id}</span>
                  </DetailRow>
                  <DetailRow label="group">
                    <span className="text-xs text-fg">{detail.service.group}</span>
                  </DetailRow>
                  <DetailRow label="node role">
                    <span className="text-xs text-fg">{detail.role}</span>
                  </DetailRow>

                  {detail.service.metrics.length > 0 && (
                    <section aria-label="metrics" className="space-y-2">
                      <h3 className="text-xs font-medium uppercase tracking-wide text-muted">
                        Metrics
                      </h3>
                      <div className="flex flex-wrap gap-2">
                        {detail.service.metrics.map((m) => (
                          <span
                            key={m.label}
                            className="inline-flex items-baseline gap-1 rounded-md border border-border bg-bg px-2 py-1 text-xs"
                            aria-label={`${detail.service.id} ${m.label}`}
                          >
                            <span className="text-muted">{m.label}</span>
                            <span className="font-medium tabular-nums text-fg">{m.value}</span>
                          </span>
                        ))}
                      </div>
                    </section>
                  )}

                  {detail.service.id === "bus" && <BusPeers extra={detail.extra} />}
                </>
              )}
            </div>
          </>
        )}
      </SheetContent>
    </Sheet>
  );
}

function HealthDot({ health }: { health: keyof typeof HEALTH_STYLES }) {
  const style = HEALTH_STYLES[health];
  return (
    <span className="inline-flex shrink-0 items-center gap-1.5">
      <span className={`h-2 w-2 rounded-full ${style.dot}`} aria-hidden />
      <span className={`text-xs font-medium ${style.text}`} aria-label={`health ${style.label}`}>
        {style.label}
      </span>
    </span>
  );
}

function DetailRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-2">
      <span className="text-xs text-muted">{label}</span>
      {children}
    </div>
  );
}

/** The bus's live peer + router zid lists (the detail behind the `peers`/`routers` counts). A solo
 *  node on an empty mesh has no peers — say so honestly rather than show an empty box as a fault. */
function BusPeers({ extra }: { extra: Record<string, unknown> }) {
  const peers = asZids(extra.peer_zids);
  const routers = asZids(extra.router_zids);
  return (
    <section aria-label="mesh peers" className="space-y-3">
      <h3 className="text-xs font-medium uppercase tracking-wide text-muted">Zenoh mesh</h3>
      <ZidList label="peers" zids={peers} />
      <ZidList label="routers" zids={routers} />
    </section>
  );
}

function ZidList({ label, zids }: { label: string; zids: string[] }) {
  return (
    <div aria-label={`bus ${label} list`}>
      <p className="mb-1 text-xs text-muted">
        {label} ({zids.length})
      </p>
      {zids.length === 0 ? (
        <p className="text-xs text-muted">none connected (solo on the mesh)</p>
      ) : (
        <ul className="space-y-1">
          {zids.map((z) => (
            <li
              key={z}
              className="truncate rounded-md border border-border bg-bg px-2 py-1 font-mono text-xs text-fg"
            >
              {z}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** Narrow an opaque `extra` field to a string list (the wire shape is `string[]`, but `extra` is
 *  typed loose so the detail type stays subsystem-agnostic). */
function asZids(v: unknown): string[] {
  return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [];
}
