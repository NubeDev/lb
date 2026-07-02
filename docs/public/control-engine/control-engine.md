# Control Engine

TODO — filled when the `control-engine` extension ships. The scope is co-located with the extension
(it is 100% an extension): `rust/extensions/control-engine/docs/control-engine-scope.md`.
The generic live-feed primitive it builds on: `docs/scope/extensions/extension-watch-scope.md`.

The `control-engine` native (Tier-2) extension bridges Control Engine (CE) instances into a
workspace as a caps-gated MCP surface (`ce.*`): a local CE over `localhost` REST/WS, and remote
CEs on **appliance** LB nodes reached by routed MCP over Zenoh. The visual editor is the vendored
`@nube/ce-wiresheet` package, mounted as the extension's federated page and driven through the
MCP bridge.
