# Node roles

TODO: Document the shipped node roles, config switches, and edge/cloud behavior differences.

Planned (scope written, not yet shipped):
- **Deployment personas** — `hub` / `appliance` / `workstation` / `browser` / `mobile` (README §5).
- **Appliance ↔ hub connection** — role/Zenoh config, appliance **API tokens**, and
  **restricted access** to an appliance (admin sees all; users granted specific ones); see
  `scope/node-roles/node-connection-scope.md`.
- **Fleet presence** — admin roster of connected nodes via Zenoh liveliness; see
  `scope/node-roles/fleet-presence-scope.md`.

