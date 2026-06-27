// The system-map wire shapes — mirror the gateway's `/system/*` route responses (system-map scope),
// which are the host `lb_host::System*` types serialized. The System page is the admin, READ-ONLY
// workspace topology + status console: a per-subsystem status grid + a react-flow wiring graph, both
// projected from one live snapshot. There is NO write shape here by design (read-only map).

/** A coarse per-subsystem health rollup. `idle` = up but nothing flowing (an empty queue is healthy,
 *  not a fault); kept distinct from `ok`/`degraded`/`down`. Mirrors `lb_host::Health`. */
export type Health = "ok" | "idle" | "degraded" | "down";

/** One labelled number on a subsystem card (a count, a role, "native ×3"). Mirrors `lb_host::Metric`. */
export interface Metric {
  label: string;
  value: string;
}

/** The status of one subsystem — one card in the grid, one node in the graph. `id` is the stable key
 *  the topology edges reference; `group` buckets the card (motion/state/workflow/runtime). Mirrors
 *  `lb_host::ServiceStatus`. */
export interface ServiceStatus {
  id: string;
  label: string;
  group: string;
  health: Health;
  detail: string;
  metrics: Metric[];
}

/** The workspace-scoped status snapshot. `role` is the node's configured posture (label only — config,
 *  not a code branch). Mirrors `lb_host::SystemOverview`. */
export interface SystemOverview {
  ws: string;
  role: string;
  services: ServiceStatus[];
}

/** The full detail of ONE subsystem — the same card the grid shows, plus a subsystem-specific `extra`
 *  blob the grid has no room for. The detail view a no-page card (gateway/bus/mcp) drills into. For
 *  `bus`, `extra` carries the live peer/router zid lists; `{}` otherwise. Mirrors
 *  `lb_host::SubsystemDetail`. */
export interface SubsystemDetail {
  ws: string;
  role: string;
  service: ServiceStatus;
  /** Subsystem-specific detail. For `bus`: `{ peer_zids: string[]; router_zids: string[] }`. */
  extra: Record<string, unknown>;
}

/** A topology node — a projection of a `ServiceStatus` minus the metrics. Mirrors `lb_host::TopoNode`. */
export interface TopoNode {
  id: string;
  label: string;
  group: string;
  health: Health;
}

/** A directed topology edge: `from` reaches `to`, annotated by how. Mirrors `lb_host::TopoEdge`. */
export interface TopoEdge {
  from: string;
  to: string;
  label: string;
}

/** Nodes + wiring edges for the react-flow view. Mirrors `lb_host::SystemTopology`. */
export interface SystemTopology {
  ws: string;
  role: string;
  nodes: TopoNode[];
  edges: TopoEdge[];
}
