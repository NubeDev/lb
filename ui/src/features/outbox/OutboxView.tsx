// The outbox status view — read-only delivery status (collaboration scope, slice 4). Shows the
// workspace's effects grouped pending → delivered → dead-lettered (→ held). No editing: the outbox
// is must-deliver infrastructure, users see effects + status, never an editable queue. Data in
// useOutbox.
//
// Styled to read as a sibling of the inbox: the same `AppPageHeader` + Refresh affordance, the same
// status-dot/`Badge` colour vocabulary the System page's `HEALTH_STYLES` speaks (emerald / amber /
// destructive / muted), and shadcn primitives in place of the old raw `<header>`/`<ul>` markup. The
// group headers keep their `Title · N` text — the gateway test asserts that shape, including `· 0`
// for an empty group, so every group always renders.

import { RefreshCw, Send } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useOutbox } from "./useOutbox";
import type { Effect } from "@/lib/outbox/outbox.types";

interface Props {
  ws: string;
}

/** Lifecycle groups in display order. `held` is optional on the wire (older nodes), so it's added
 *  conditionally at render time rather than here. */
const GROUPS = [
  { key: "pending", title: "Pending" },
  { key: "delivered", title: "Delivered" },
  { key: "dead_lettered", title: "Dead-lettered" },
] as const;

export function OutboxView({ ws }: Props) {
  const { status, error, loading, refresh } = useOutbox();

  const held = status.held ?? [];
  const total = status.pending.length + status.delivered.length + status.dead_lettered.length + held.length;

  return (
    <section className="flex h-full flex-col bg-bg">
      <AppPageHeader
        icon={Send}
        title="Outbox"
        description="Read-only delivery status for queued effects."
        workspace={ws}
        actions={
          <Button
            variant="outline"
            size="sm"
            onClick={() => void refresh()}
            disabled={loading}
            aria-label="Refresh outbox"
          >
            <RefreshCw size={14} className={cn(loading && "animate-spin")} />
            Refresh
          </Button>
        }
      />

      {error && (
        <div className="px-4 pt-3">
          <Alert variant="destructive">
            <AlertTitle>Couldn’t load the outbox</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        </div>
      )}

      {total === 0 && !loading ? (
        <div className="flex flex-1 items-center justify-center p-6">
          <EmptyPane />
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto">
          {GROUPS.map((g) => (
            <Group
              key={g.key}
              title={g.title}
              effects={status[g.key]}
            />
          ))}
          {held.length > 0 && <Group title="Held" effects={held} />}
        </div>
      )}
    </section>
  );
}

/** One lifecycle group — a header (kept as `Title · N` so the gateway test's counts hold) and a
 *  borderless list of effect rows. Renders even when empty (the test asserts `· 0`). */
function Group({ title, effects }: { title: string; effects: Effect[] }) {
  return (
    <section className="border-b border-border px-4 py-3 last:border-b-0">
      <h2 className="mb-2 text-xs font-medium text-muted">
        {title} · {effects.length}
      </h2>
      {effects.length === 0 ? (
        <p className="text-xs text-muted/70">No effects.</p>
      ) : (
        <ul role="list" className="flex flex-col gap-1.5">
          {effects.map((e) => (
            <EffectRow key={e.id} effect={e} />
          ))}
        </ul>
      )}
    </section>
  );
}

function EffectRow({ effect: e }: { effect: Effect }) {
  const style = STATUS_STYLE[e.status] ?? STATUS_STYLE.pending;
  return (
    <li
      role="listitem"
      className="flex items-center gap-3 rounded-md border border-border bg-card/60 px-3 py-2"
    >
      <span className={cn("h-2 w-2 shrink-0 rounded-full", style.dot)} aria-hidden />
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm text-fg">
          <span className="text-muted">{e.target}</span> {e.action}
        </div>
        <div className="text-xs text-muted">
          {e.attempts} {e.attempts === 1 ? "attempt" : "attempts"}
          {e.ts > 0 ? ` · ${new Date(e.ts).toLocaleString()}` : ""}
        </div>
      </div>
      <Badge variant="outline" className={cn("gap-1.5 border-0 px-1.5", style.text)}>
        {style.label}
      </Badge>
    </li>
  );
}

/** The status → token mapping, mirroring `HEALTH_STYLES` so an effect's lifecycle reads the same as
 *  the System page's colour vocabulary (emerald ok / amber held / destructive dead-lettered). */
const STATUS_STYLE: Record<Effect["status"], { label: string; dot: string; text: string }> = {
  pending: { label: "Pending", dot: "bg-muted/60", text: "text-muted" },
  held: { label: "Held", dot: "bg-amber-500", text: "text-amber-600 dark:text-amber-400" },
  delivered: { label: "Delivered", dot: "bg-emerald-500", text: "text-emerald-600 dark:text-emerald-400" },
  failed: { label: "Failed", dot: "bg-amber-500", text: "text-amber-600 dark:text-amber-400" },
  "dead-lettered": { label: "Dead-lettered", dot: "bg-destructive", text: "text-destructive" },
  discarded: { label: "Discarded", dot: "bg-muted/60", text: "text-muted" },
};

function EmptyPane() {
  return (
    <div className="flex w-full max-w-sm items-center gap-3 rounded-lg border border-border bg-card/60 p-4 text-sm text-muted">
      <Send size={18} />
      No effects queued.
    </div>
  );
}
