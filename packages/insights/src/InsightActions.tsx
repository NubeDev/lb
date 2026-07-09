// The insight action row — ack / resolve / dismiss buttons (the package's own look). Status-driven
// visibility so a stale action can't be re-fired: Ack shows only while `open`, Resolve until
// `resolved`, and a resolved insight shows a quiet "Resolved" marker. `onDismiss` is optional — a
// widget that only wants to hide a row locally (not resolve it server-side) wires it.
//
// Every action is gated at the host server-side; this UI gate is convenience. A read-only widget
// simply doesn't render this component.

import type { JSX } from "react";
import { Check, CheckCheck, RefreshCw, X } from "lucide-react";

import type { Insight } from "./types";

export interface InsightActionsProps {
  insight: Insight;
  /** The in-flight action (drives the spinner + disable), or null. */
  actingOn?: "ack" | "resolve" | null;
  onAck?: () => void;
  onResolve?: () => void;
  /** Optional local dismiss (hide the row) — distinct from `resolve` (a durable status change). */
  onDismiss?: () => void;
}

/** The ack/resolve/dismiss button row. Renders only the actions the current status allows. */
export function InsightActions({
  insight,
  actingOn = null,
  onAck,
  onResolve,
  onDismiss,
}: InsightActionsProps): JSX.Element {
  const busy = actingOn !== null;
  return (
    <div className="ins-actions">
      {onDismiss && (
        <button type="button" className="ins-btn" onClick={onDismiss} disabled={busy}>
          <X size={13} />
          Dismiss
        </button>
      )}
      {insight.status === "open" && onAck && (
        <button type="button" className="ins-btn" onClick={onAck} disabled={busy}>
          {actingOn === "ack" ? <RefreshCw size={13} className="ins-spin" /> : <Check size={13} />}
          Ack
        </button>
      )}
      {insight.status !== "resolved" && onResolve && (
        <button
          type="button"
          className="ins-btn is-primary"
          onClick={onResolve}
          disabled={busy}
        >
          {actingOn === "resolve" ? (
            <RefreshCw size={13} className="ins-spin" />
          ) : (
            <CheckCheck size={13} />
          )}
          Resolve
        </button>
      )}
      {insight.status === "resolved" && (
        <span className="ins-badge tone-success">
          <CheckCheck size={12} /> Resolved
        </span>
      )}
    </div>
  );
}
