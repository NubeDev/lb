// "Pin to dashboard" affordance (widget-platform scope, Slice B) — the user-facing half of the pin
// mechanism. Mounted by `ResponseView` beside a rendered `rich_result`: a small control that picks a
// target dashboard (from `dashboard.list` + a "New dashboard" option) and calls `dashboard.pin` over the
// real gateway. The HOST mints the cell from the envelope (generic over the tool id, rule 10); the
// client passes the envelope through — it does NOT construct a cell. This keeps the envelope↔cell mapping
// in one place (the host), so a headless `POST /mcp/call` agent and the web UI produce the SAME pinned
// cell.
//
// One responsibility: turn a rendered `rich_result` into a persisted dashboard cell. The envelope is the
// `RichResultPayload` MINUS its wire tags (`kind`/`v`) — that's the `x-lb-render` shape the host mints from.

import { useEffect, useMemo, useRef, useState } from "react";
import { Pin, Plus, X } from "lucide-react";

import type { RichResultPayload } from "@/lib/channel/payload.types";
import { listDashboards, pinDashboard } from "@/lib/dashboard";
import type { DashboardSummary } from "@/lib/dashboard";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface Props {
  payload: RichResultPayload;
}

/** The `x-lb-render` envelope to pin — the payload minus its wire tags (`kind`/`v`). The host's
 *  `mint_cell_from_envelope` reads `view`/`source`/`action`/`options`/`tools`/`fieldConfig`. */
function envelopeOf(payload: RichResultPayload): Record<string, unknown> {
  const { kind: _kind, v: _v, ...envelope } = payload;
  return envelope;
}

const NEW_ID = "__new__";

/** A small popover-style affordance: a "Pin" button that reveals a target-dashboard picker + a
 *  pin/confirm. On success it shows a one-line confirmation naming the dashboard. Errors render inline
 *  (a non-owner pin → a 403 from the gateway, shown as a short message). Nothing here is a cell — the
 *  host owns the cell construction. */
export function PinToDashboard({ payload }: Props) {
  const [open, setOpen] = useState(false);
  const [dashboards, setDashboards] = useState<DashboardSummary[]>([]);
  const [target, setTarget] = useState<string>("");
  const [newTitle, setNewTitle] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pinnedName, setPinnedName] = useState<string | null>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  // Load the caller's dashboard roster when the affordance opens. `listDashboards` returns the
  // reachable summaries (own + team-shared + workspace); the caller may only pin into their OWN
  // dashboards (owner-only-update), but we list all reachable so the picker is honest about what's there.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    listDashboards()
      .then((rows) => {
        if (cancelled) return;
        setDashboards(rows);
        setTarget(rows.length > 0 ? rows[0].id : NEW_ID);
      })
      .catch((e) => !cancelled && setError(String(e?.message ?? e)));
    return () => {
      cancelled = true;
    };
  }, [open]);

  // Close on outside click (a lightweight dismiss — no portal; the dropdown is small and fits the row).
  useEffect(() => {
    if (!open) return;
    function onDown(e: MouseEvent) {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  const envelope = useMemo(() => envelopeOf(payload), [payload]);
  const isNew = target === NEW_ID;

  async function pin() {
    const id = isNew ? slugForNew(newTitle) : target;
    if (!id) {
      setError("pick a dashboard or enter a title for the new one");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const d = await pinDashboard(id, envelope, isNew ? newTitle || "Pinned" : "");
      setPinnedName(d.title || id);
      setOpen(false);
    } catch (e: unknown) {
      setError(humanizeError(e));
    } finally {
      setBusy(false);
    }
  }

  if (pinnedName && !open) {
    return (
      <div className="mt-1 text-xs text-muted" role="status">
        pinned to <span className="text-accent">{pinnedName}</span>{" "}
        <Button variant="ghost" size="sm" className="h-auto px-1 py-0 text-xs" onClick={() => setPinnedName(null)}>
          (pin again)
        </Button>
      </div>
    );
  }

  return (
    <div className="relative mt-1" ref={popoverRef}>
      <Button
        variant="ghost"
        size="sm"
        className="h-7 gap-1 px-2 text-xs text-muted"
        aria-label="Pin to dashboard"
        title="Pin this response to a dashboard"
        onClick={() => setOpen((o) => !o)}
      >
        <Pin size={14} /> Pin to dashboard
      </Button>
      {open && (
        <div
          className="absolute z-20 mt-1 w-64 rounded-md border border-border bg-panel p-2 shadow-md"
          role="dialog"
          aria-label="Pin to dashboard"
        >
          <div className="mb-1 flex items-center justify-between">
            <span className="text-xs font-medium">Pin to a dashboard</span>
            <Button
              variant="ghost"
              size="icon"
              className="h-6 w-6"
              aria-label="Close pin picker"
              onClick={() => setOpen(false)}
            >
              <X size={14} />
            </Button>
          </div>
          <select
            className="mb-2 w-full rounded-md border border-border bg-bg px-2 py-1 text-xs"
            value={target}
            onChange={(e) => setTarget(e.target.value)}
            aria-label="Target dashboard"
          >
            {dashboards.map((d) => (
              <option key={d.id} value={d.id}>
                {d.title || d.id}
              </option>
            ))}
            <option value={NEW_ID}>+ New dashboard…</option>
          </select>
          {isNew && (
            <Input
              className="mb-2 h-8 text-xs"
              placeholder="New dashboard title"
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              aria-label="New dashboard title"
            />
          )}
          <Button
            variant="default"
            size="sm"
            className="h-7 w-full gap-1 text-xs"
            disabled={busy || (isNew && !newTitle.trim())}
            onClick={() => void pin()}
          >
            <Plus size={14} /> {isNew ? "Create + pin" : "Pin"}
          </Button>
          {error && (
            <div className="mt-1 text-xs text-destructive" role="alert">
              {error}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

/** Mint a short slug for a new dashboard id from a title — lowercase, non-alphanumeric → `-`. The host
 *  treats the dashboard id opaquely (idempotent UPSERT on the slug), so a stable-ish slug is fine. */
function slugForNew(title: string): string {
  const slug = title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
 .replace(/^-+|-+$/g, "");
  return slug || `pin-${Date.now().toString(36)}`;
}

/** A gateway error (`InvokeError` 403/400/etc.) → a short human message. A 403 is the non-owner-deny
 *  (the pin cap is missing, or the dashboard is someone else's); a 400 is a host `BadInput` (a malformed
 *  envelope, an unknown view). Everything else surfaces the raw message. */
function humanizeError(e: unknown): string {
  const msg = e instanceof Error ? e.message : String(e);
  if (/403|forbidden|denied/i.test(msg)) return "not allowed — pick a dashboard you own, or a new one";
  if (/400|bad.?input|unknown view/i.test(msg)) return msg.replace(/^.*?:\s*/, "");
  return msg;
}