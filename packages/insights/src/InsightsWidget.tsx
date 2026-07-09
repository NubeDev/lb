// The insights dashboard WIDGET — a compact list of insights, self-contained: it drives `useInsights`
// over the injected `InsightsClient` and renders the package's own look. Two behaviours through ONE
// component, chosen by the `interactive` prop:
//   - read-only (`interactive={false}`, the default): a glanceable list, no action buttons.
//   - acknowledge (`interactive`): each open/acked row gets ack / resolve / dismiss inline.
// The convenience wrappers `InsightsReadWidget` / `InsightsAckWidget` pin the prop.
//
// The look is OPTIONAL: a host that wants its own skin ignores this component and drives `useInsights`
// + the model helpers into its own markup. This widget is the batteries-included default.

import type { JSX } from "react";
import { useState } from "react";
import { Lightbulb, RefreshCw } from "lucide-react";

import type { InsightsClient, ListQuery } from "./types";
import { useInsights } from "./useInsights";
import { InsightRow } from "./InsightRow";
import { InsightActions } from "./InsightActions";

export interface InsightsWidgetProps {
  /** The injected transport seam (how to reach the node's `insight.*` verbs). */
  client: InsightsClient;
  /** The starting filter (status / severity / tags / range / limit). Defaults to `{ limit: 20 }`. */
  filter?: ListQuery;
  /** Panel title. Defaults to "Insights". */
  title?: string;
  /** When true, each row carries ack / resolve / dismiss actions; when false (default), read-only. */
  interactive?: boolean;
  /** Show the header refresh button. Default: true. */
  showRefresh?: boolean;
  /** Show the "Load more" footer when a next page exists. Default: true. */
  paged?: boolean;
  /** Click a row (e.g. to open a host detail surface). Rows are static when omitted. */
  onSelect?: (id: string) => void;
  /** `now` for the time-ago (test determinism). */
  now?: number;
}

const DEFAULT_FILTER: ListQuery = { limit: 20 };

/** The insights widget. Read-only or acknowledge, one component. */
export function InsightsWidget({
  client,
  filter = DEFAULT_FILTER,
  title = "Insights",
  interactive = false,
  showRefresh = true,
  paged = true,
  onSelect,
  now,
}: InsightsWidgetProps): JSX.Element {
  const state = useInsights(client, filter);
  // Locally-dismissed rows — a dismiss hides the row in this widget without a durable status change.
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());
  // The verb currently in flight (the hook tracks only the row id) — drives the right button spinner.
  const [pending, setPending] = useState<"ack" | "resolve" | null>(null);

  function run(id: string, action: "ack" | "resolve") {
    setPending(action);
    void state.act(id, action).finally(() => setPending(null));
  }

  const rows = state.items.filter((it) => !dismissed.has(it.id));

  return (
    <div className="ins-root">
      <div className="ins-header">
        <h3 className="ins-header-title">
          <Lightbulb size={15} />
          {title}
          {rows.length > 0 && <span className="ins-header-count">({rows.length})</span>}
        </h3>
        {showRefresh && (
          <div className="ins-header-actions">
            <button
              type="button"
              className="ins-btn"
              onClick={() => void state.refresh()}
              disabled={state.loading}
              aria-label="Refresh insights"
            >
              <RefreshCw size={13} className={state.loading ? "ins-spin" : undefined} />
            </button>
          </div>
        )}
      </div>

      {state.error && rows.length === 0 ? (
        <div className="ins-error" role="alert">
          {state.error}
        </div>
      ) : rows.length === 0 ? (
        <div className="ins-empty">
          <Lightbulb size={16} className={state.loading ? "ins-spin" : undefined} />
          {state.loading ? "Loading insights…" : "No insights match this filter."}
        </div>
      ) : (
        <ul className="ins-list">
          {rows.map((it) => (
            <InsightRow
              key={it.id}
              insight={it}
              onSelect={onSelect}
              now={now}
              actions={
                interactive ? (
                  <InsightActions
                    insight={it}
                    actingOn={state.actingOn === it.id ? pending : null}
                    onAck={it.status === "open" ? () => run(it.id, "ack") : undefined}
                    onResolve={() => run(it.id, "resolve")}
                    onDismiss={() =>
                      setDismissed((prev) => new Set(prev).add(it.id))
                    }
                  />
                ) : undefined
              }
            />
          ))}
        </ul>
      )}

      {paged && state.nextCursor !== null && rows.length > 0 && (
        <div className="ins-more">
          <button
            type="button"
            className="ins-btn"
            onClick={() => void state.loadMore()}
            disabled={state.loading}
            aria-label="Load more insights"
          >
            <RefreshCw size={13} className={state.loading ? "ins-spin" : undefined} />
            Load more
          </button>
        </div>
      )}
    </div>
  );
}

/** Read-only preset — a glanceable list, no actions. */
export function InsightsReadWidget(props: Omit<InsightsWidgetProps, "interactive">): JSX.Element {
  return <InsightsWidget {...props} interactive={false} />;
}

/** Acknowledge preset — each row carries ack / resolve / dismiss. */
export function InsightsAckWidget(props: Omit<InsightsWidgetProps, "interactive">): JSX.Element {
  return <InsightsWidget {...props} interactive />;
}
