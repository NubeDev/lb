// View/DTO types for the channel surface — mirror the Rust `lb_inbox::Item` one-to-one
// (FILE-LAYOUT: the type has the same name across the tool, the DTO, and the client).

/** A normalized channel/inbox item, as the node speaks it. */
export interface Item {
  id: string;
  channel: string;
  author: string;
  body: string;
  /** Logical ordering timestamp (caller-injected, not wall-clock). */
  ts: number;
}

/** A registered channel — mirrors the Rust `ChannelRecord` (collaboration scope, slice 2). */
export interface ChannelRecord {
  /** The channel id (e.g. `general`). */
  id: string;
  /** The principal that first registered it. */
  created_by: string;
  /** A constant discriminant (`channel`). */
  kind: string;
  /** Logical ordering timestamp. */
  ts: number;
}
