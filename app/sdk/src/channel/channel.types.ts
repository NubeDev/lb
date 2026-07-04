// View/DTO types for the channel surface — mirror `ui/src/lib/channel/channel.types.ts` and the
// Rust `lb_inbox::Item` one-to-one (same name across the tool, the DTO, and both shells).

/** A normalized channel/inbox item, as the node speaks it. */
export interface Item {
  id: string;
  channel: string;
  author: string;
  body: string;
  /** Logical ordering timestamp (caller-injected, not wall-clock). */
  ts: number;
}

/** A registered channel — mirrors the Rust `ChannelRecord`. */
export interface ChannelRecord {
  id: string;
  created_by: string;
  kind: string;
  ts: number;
}
