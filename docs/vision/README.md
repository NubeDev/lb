# Vision

TODO: Capture the platform north star and link numbered design notes from this index.

## Design notes

- [0001 — Platform north star](0001-platform-north-star.md) — what Lazybones is and where it's going.
- [0002 — Worked example: the Coding Agent Workplace extension](0002-coding-agent-workplace.md) — an end-to-end product built purely by composing core primitives (inbox/outbox, jobs, channels, docs/skills, a central AI agent, MCP).
- [0003 — Worked example: the B2B Fleet IoT Dashboard (KFC & McDonald's)](0003-iot-dashboard.md) — the **B2B** shape (one workspace = one company): two restaurant chains run the same extensions on one hub, walled in separate workspaces that never share data; headless edge nodes producing timeseries, the hub owning + serving the cloud dashboard, access scoped per appliance/store; **§6 scales each tenant to a Niagara-style fleet** (peer-to-peer, thousands of stores, external TimescaleDB via jobs, role-gated pages, multi-language).
- [0004 — Worked example: Consumer IoT (the Daikin model)](0004-consumer-iot.md) — the **B2C** sibling of 0003 (one workspace *per household*): a global identity owns a home workspace with its appliances, is invited as a guest into a friend's workspace for the weekend (scoped by grant-by-tag), and leaves with one `members.remove`; surfaces the **org-tier gap** (brand-wide visibility across households) as the single finding.

