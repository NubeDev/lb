# Flows — public

Status: **TODO** (stub). Fills in when the flows engine ships.

The visual node-graph flow engine: a `flow:{ws}:{id}` typed node graph authored on a React
Flow canvas, run as a durable resumable `lb-jobs` session, with **extension-contributed backend
node types** (`[[node]]` in `extension.toml`, identical for WASM and native — only the execution
transport differs). A node model + editor + node-registry over the shipped `chains.*` /
`lb-rules` / jobs / dashboard-bridge / grant / undo primitives — not a new engine.

See `scope/flows/flows-scope.md` for the ask.
</content>
