// One insight row — a severity dot + title + mono origin meta + a side column (status badge + time-
// ago). The whole row is a <button> when `onSelect` is given (a clickable list) and a plain <div>
// otherwise (a read-only widget). Optional inline `actions` render below the row (the acknowledge
// widget). The package's own look (scoped `ins-*`); a host with its own skin builds its own row from
// the hooks + model helpers.

import type { JSX, ReactNode } from "react";
import type { Insight } from "./types";
import { originLine, timeAgo } from "./model";
import { SeverityBadge, StatusBadge } from "./InsightBadge";

export interface InsightRowProps {
  insight: Insight;
  selected?: boolean;
  /** Click handler — present → the row is a button; absent → a static row (read-only). */
  onSelect?: (id: string) => void;
  /** Which badges to show on the side column. Status shows by default; severity is already carried by
   *  the leading dot, so its redundant chip is off by default (opt in for a legend-style row). */
  showStatus?: boolean;
  showSeverity?: boolean;
  /** Optional inline actions node (rendered below the row body) — the acknowledge widget's buttons. */
  actions?: ReactNode;
  /** `now` for the time-ago (test determinism). */
  now?: number;
}

/** Render one insight row. */
export function InsightRow({
  insight,
  selected,
  onSelect,
  showStatus = true,
  showSeverity = false,
  actions,
  now,
}: InsightRowProps): JSX.Element {
  const dotClass =
    insight.severity === "critical"
      ? "is-critical"
      : insight.severity === "warning"
        ? "is-warning"
        : "is-info";

  const body = (
    <>
      <span className={`ins-dot ${dotClass}`} role="img" aria-label={`severity: ${insight.severity}`} />
      <span className="ins-row-main">
        <span className="ins-row-title">{insight.title}</span>
        <span className="ins-row-meta">
          {originLine(insight.origin)} · ×{insight.count}
        </span>
      </span>
      <span className="ins-row-side">
        {showSeverity && <SeverityBadge severity={insight.severity} />}
        {showStatus && <StatusBadge status={insight.status} />}
        <span className="ins-time">{timeAgo(insight.last_ts, now)}</span>
      </span>
    </>
  );

  return (
    <li>
      {onSelect ? (
        <button
          type="button"
          className={`ins-row${selected ? " is-selected" : ""}`}
          aria-selected={selected}
          aria-label={`select insight ${insight.dedup_key}`}
          onClick={() => onSelect(insight.id)}
        >
          {body}
        </button>
      ) : (
        <div className={`ins-row${selected ? " is-selected" : ""}`}>{body}</div>
      )}
      {actions}
    </li>
  );
}
