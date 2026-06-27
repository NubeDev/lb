// View types for the durable-workflow verbs the bridge exposes (`outbox.status`, `inbox.list`,
// `inbox.resolve`) plus the ingest write the demo round-trips on. The host owns the wire shape; the
// page reads only what it shows. These mirror the REAL verb results `lb_host::call_tool` returns:
//   ingest.write   → { accepted: number }
//   outbox.status  → { pending: Effect[], delivered: Effect[], dead_lettered: Effect[] }
//   inbox.list     → { items: Item[] }
//   inbox.resolve  → { ok: true }

/** One outbox effect, as `outbox.status` groups them by lifecycle. Permissive — the page renders only
 *  the id/target/action for a short row; the rest of the durable shape is carried but unused here. */
export interface Effect {
  id: string;
  target?: string;
  action?: string;
  status?: string;
  [k: string]: unknown;
}

/** The `outbox.status` result the bridge forwards: effects grouped by delivery lifecycle stage. */
export interface OutboxStatus {
  pending: Effect[];
  delivered: Effect[];
  dead_lettered: Effect[];
}

/** One durable inbox item, as `inbox.list` returns them inside `{ items }`. */
export interface InboxItem {
  id: string;
  channel: string;
  author: string;
  body: string;
  ts: number;
}

/** The `inbox.list` result the bridge forwards: the channel's durable items, oldest→newest. */
export interface InboxListResult {
  items: InboxItem[];
}

/** A reviewer's decision on an inbox item — the kebab-case wire form the host's `Decision` expects. */
export type Decision = "approved" | "rejected" | "deferred";
