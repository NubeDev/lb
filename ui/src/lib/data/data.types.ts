// The DB-browser wire shapes — mirror the gateway's `store.*` route responses (data-console scope).
// The Data page is the admin, READ-ONLY raw-store lens: a table picker with counts, a paged row
// grid, and a react-flow relation graph. There is NO write shape here by design (edits go through
// the domain verbs, never the raw grid).

/** One table + its row count (the picker). Mirrors `lb_store::TableCount`. */
export interface TableCount {
  table: string;
  count: number;
}

/** One scanned record: its full `table:id` and its stored fields. Heterogeneous — the grid infers a
 *  column union and renders nested values as JSON. Mirrors `lb_store::Row`. */
export interface Row {
  id: string;
  data: Record<string, unknown>;
}

/** A bounded page of a scan + the cursor for the next page (`null` at the end). Mirrors
 *  `lb_store::Page`. */
export interface Page {
  rows: Row[];
  next: string | null;
}

/** A react-flow node: the record id (used directly as the node id) + a `kind` (its table) to style
 *  by. Mirrors `lb_store::GraphNode`. */
export interface GraphNode {
  id: string;
  kind: string;
}

/** A react-flow edge between two record ids, labelled by the relation it came from. Mirrors
 *  `lb_store::GraphEdge`. */
export interface GraphEdge {
  source: string;
  target: string;
  label: string;
}

/** The graph slice: deduped nodes + relation edges, ready for react-flow. */
export interface Graph {
  nodes: GraphNode[];
  edges: GraphEdge[];
}
