// A REAL in-memory `InsightsClient` — not a mock of node behaviour, but a genuine implementation of
// the injected transport seam over an array of records. It's the package's own reference client: tests
// drive it, and a host demo (a Storybook, a standalone extension without a node yet) can seed real
// records into it. `ack`/`resolve` mutate the backing records the way the node's verbs do, so a
// widget's optimistic-refresh path exercises the real state transition.
//
// This is NOT a `*.fake.ts` (CLAUDE §9): there is no node to run in-process here — the client IS the
// boundary the package is defined against, and this is one honest implementation of it. A `denyClient`
// helper models the capability-deny path (a granted-subset workspace) so tests can assert the honest
// empty/error surface.

import type {
  Insight,
  InsightsClient,
  ListPage,
  ListQuery,
  OccurrencePage,
} from "./types";

/** Build a real in-memory client over a seeded set of insights (newest-first by `last_ts`). */
export function memoryClient(seed: Insight[]): InsightsClient {
  const rows = [...seed];

  function ordered(): Insight[] {
    return [...rows].sort((a, b) => b.last_ts - a.last_ts || b.id.localeCompare(a.id));
  }

  return {
    async list(query: ListQuery): Promise<ListPage> {
      let items = ordered();
      if (query.status) items = items.filter((i) => i.status === query.status);
      if (query.severity) items = items.filter((i) => i.severity === query.severity);
      if (query.origin_ref)
        items = items.filter((i) => i.origin.ref.includes(query.origin_ref!));
      const limit = query.limit ?? 50;
      const page = items.slice(0, limit);
      const next =
        items.length > limit
          ? { ts: page[page.length - 1].last_ts, id: page[page.length - 1].id }
          : undefined;
      // Strip `evidence` exactly as the node's `insight.list` does — it is echoed by `get` only.
      // This client is the real boundary, not a mock of it, so a consumer that (wrongly) binds a
      // trend straight from a list row must fail here the same way it fails against a live node.
      const stripped = page.map(({ evidence: _evidence, ...rest }) => rest as Insight);
      return { items: stripped, next };
    },
    async get(id: string): Promise<Insight | null> {
      return rows.find((i) => i.id === id) ?? null;
    },
    async ack(id: string): Promise<void> {
      const row = rows.find((i) => i.id === id);
      if (row) row.status = "acked";
    },
    async resolve(id: string): Promise<void> {
      const row = rows.find((i) => i.id === id);
      if (row) row.status = "resolved";
    },
    async occurrences(): Promise<OccurrencePage> {
      return { items: [] };
    },
  };
}

/** A client whose reads reject — models a workspace granted no `insight.list` cap. The hooks must
 *  surface this as an honest error, never a fabricated list. */
export function denyClient(): InsightsClient {
  const denied = () => Promise.reject(new Error("Denied: mcp:insight.list:call"));
  return {
    list: denied,
    get: denied,
    ack: denied,
    resolve: denied,
    occurrences: denied,
  };
}
