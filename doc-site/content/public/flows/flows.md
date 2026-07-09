# Flows

TODO — public docs for the flows engine. Filled from the `docs/scope/flows/` set as slices ship.

Shipped so far (promote from the session docs):

- N independent triggers per flow, each firing its own subgraph (`flow-multi-trigger-reactive`).
- The `{payload, topic}` message envelope + auto-wire on connect (`flow-message-envelope`).

In flight / scoped, not yet shipped:

- Port-labelled edges + per-input-port join policy (`all` join vs `any` funnel) — the Node-RED
  multi-input model (`flow-input-ports-scope.md`).
</content>
