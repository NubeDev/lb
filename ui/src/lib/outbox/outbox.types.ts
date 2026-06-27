// View/DTO types for the outbox status surface — mirror the Rust `Effect` + `OutboxStatus`
// (collaboration scope, slice 4). Read-only: the UI shows effects grouped by lifecycle stage.

/** A must-deliver effect, as the node speaks it (a subset of fields the status view renders). */
export interface Effect {
  id: string;
  target: string;
  action: string;
  /** Where the effect is in its delivery lifecycle (kebab-case discriminant). */
  status: "pending" | "delivered" | "failed" | "dead-lettered";
  attempts: number;
  ts: number;
}

/** A workspace's outbox snapshot grouped by lifecycle — mirrors the Rust `OutboxStatus`. */
export interface OutboxStatus {
  pending: Effect[];
  delivered: Effect[];
  dead_lettered: Effect[];
}
