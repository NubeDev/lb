//! The **command-palette catalog** service — `tools.catalog`, the one new verb the `/` + `@`
//! command palette reads (channels-command-palette scope). It returns, for the calling principal in
//! this workspace, ONLY the MCP tools they are authorized to call — registered tools ∩ caps held —
//! each with a title, group, and a standard JSON-Schema `input_schema` so the palette renders a
//! guided argument rail.
//!
//! The cardinal rule (scope "Risks"): the catalog MUST advertise a tool only if the call itself
//! would allow it. So for every reachable tool it runs the **SAME `authorize_tool` gate** `call_tool`
//! runs (`lb_mcp::authorize_tool`) — one gate, two callers. The catalog can therefore NEVER offer a
//! tool that then denies, and NEVER hide one that would pass. A denied tool is **absent** from the
//! list (no existence leak — the menu IS the permission model rendered).
//!
//! One responsibility per file (FILE-LAYOUT §3):
//!   - `descriptor` — host-native descriptors + the JSON-Schema arg validator + the canonical
//!     `federation.query` schema (defense-in-depth validation the dispatcher runs too).
//!   - `catalog`    — `tools.catalog`: enumerate reachable tools, gate each, attach its schema.
//!   - `tool`       — the `tools.*` MCP bridge dispatch (host-native, reached through the contract).

mod catalog;
mod descriptor;
mod tool;

pub use catalog::{tools_catalog, ToolsCatalog};
pub(crate) use descriptor::{federation_query_schema, host_descriptors, validate_args};
pub use tool::call_tools_tool;
