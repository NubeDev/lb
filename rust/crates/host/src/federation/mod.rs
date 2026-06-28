//! The **federation** host service — the host side of the `federation` native (Tier-2) extension
//! (datasources scope). It owns the capability gates, the `datasource:{ws}:{name}` registry record,
//! the `net:*` pre-connect enforcement, the DSN secret-mediation, and the routing to the supervised
//! sidecar that embeds DataFusion. SurrealDB stays the authority (rule 2 — never a DataFusion
//! source); external DBs are federated sources reached only through these gated verbs (rule 5/6/7).
//!
//! One responsibility per file (FILE-LAYOUT §3):
//!   - `record`    — the `datasource:{ws}:{name}` store record (kind + endpoint ref + secret ref).
//!   - `authorize` — the `mcp:<verb>:call` gate (workspace-first).
//!   - `net`       — pre-connect `net:*` enforcement (the endpoint must be in the install grant).
//!   - `secret`    — DSN mediation out of `lb-secrets` under the extension's own grant.
//!   - `validate`  — host-side SELECT-only pre-check (the sidecar re-validates with `sqlparser`).
//!   - `add`/`remove`/`list`/`test` — the admin CRUD + connectivity probe.
//!   - `query`     — `federation.query` (the read-first verb).
//!   - `mirror`    — `federation.mirror` (the durable, resumable `lb-jobs` copy-in).
//!   - `tool`      — the `federation.*` / `datasource.*` MCP bridge dispatch.

mod add;
mod authorize;
mod error;
mod list;
mod mirror;
mod net;
mod query;
mod record;
mod remove;
mod secret;
mod test;
mod tool;
mod validate;

pub use add::datasource_add;
pub use error::FederationError;
pub use list::{datasource_list, DatasourceSummary};
pub use mirror::federation_mirror;
pub use query::federation_query;
pub use record::{resolve as resolve_datasource, Datasource};
pub use remove::datasource_remove;
pub use test::datasource_test;
pub use tool::call_federation_tool;
