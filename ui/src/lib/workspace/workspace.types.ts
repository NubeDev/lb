// View/DTO types for the workspace surface — mirror the Rust `WorkspaceRecord` one-to-one
// (FILE-LAYOUT: same name across tool, DTO, and client).

/** A workspace in the node directory, as the node speaks it. */
export interface WorkspaceRecord {
  /** The workspace id — the SurrealDB namespace, the hard wall (§7). */
  ws: string;
  /** A human-friendly display name for the switcher. */
  name: string;
  /** A constant discriminant (`workspace`). */
  kind: string;
  /** Logical ordering timestamp (caller-injected). */
  ts: number;
}
