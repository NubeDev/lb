# Control Engine

TODO — filled when the `control-engine` extension ships. See the ask in
[`scope/control-engine/control-engine-scope.md`](../../scope/control-engine/control-engine-scope.md).

The `control-engine` native (Tier-2) extension bridges Control Engine (CE) instances into a
workspace as a caps-gated MCP surface (`ce.*`): a local CE over `localhost` REST/WS, and remote
CEs on **appliance** LB nodes reached by routed MCP over Zenoh. The visual editor is the vendored
`@nube/ce-wiresheet` package, mounted as the extension's federated page and driven through the
MCP bridge.
