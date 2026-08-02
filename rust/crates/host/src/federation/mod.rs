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
//!   - `schema`    — `federation.schema` (native table/column discovery for the no-SQL UI).
//!   - `sample`    — `federation.sample` (one AI-ready snapshot: tables + FKs + sample rows).
//!   - `mirror`    — `federation.mirror` (the durable, resumable `lb-jobs` copy-in).
//!   - `profile*`  — `federation.profile`/`profile_get`/`profile_refresh` + the
//!                   `datasource_profile:{ws}:{source}` record and its freshness reactor. Compiled
//!                   only under the `datasource-profile` feature (off by default).
//!   - `tool`      — the `federation.*` / `datasource.*` MCP bridge dispatch.

mod add;
mod authorize;
mod dbschema_delete;
mod dbschema_get;
mod dbschema_list;
mod dbschema_record;
mod dbschema_save;
mod delete;
mod error;
mod export;
mod install;
mod list;
mod migrate;
mod mirror;
mod net;
mod query;
#[cfg(feature = "datasource-profile")]
mod profile;
#[cfg(feature = "datasource-profile")]
mod profile_get;
#[cfg(feature = "datasource-profile")]
mod profile_record;
#[cfg(feature = "datasource-profile")]
mod profile_refresh;
#[cfg(feature = "datasource-profile")]
mod react_to_profiles;
mod record;
mod remove;
mod sample;
mod schema;
mod secret;
mod test;
mod tool;
mod validate;
mod write;

pub use add::datasource_add;
pub use dbschema_delete::dbschema_delete;
pub use dbschema_get::dbschema_get;
pub use dbschema_list::dbschema_list;
#[allow(unused_imports)]
pub use dbschema_record::{schema_tag, DbSchemaRecord, SCHEMA_VERSION, TABLE as DBSCHEMA_TABLE};
pub use dbschema_save::dbschema_save;
pub use delete::{delete_descriptor, federation_delete};
pub use error::FederationError;
pub use export::{export_descriptor, federation_export, ExportFrom};
pub use install::{install_federation, Installed, SeedSource};
pub use list::{datasource_list, DatasourceSummary};
pub use migrate::{federation_migrate, migrate_descriptor};
pub use mirror::federation_mirror;
pub use net::enforce_endpoint;
pub use query::{federation_query, query_descriptor};
#[cfg(feature = "datasource-profile")]
pub use profile::{federation_profile, profile_descriptor, ProfileBounds};
#[cfg(feature = "datasource-profile")]
pub use profile_get::{federation_profile_get, profile_get_descriptor};
#[cfg(feature = "datasource-profile")]
#[allow(unused_imports)]
pub use profile_record::{
    define_profile_index, profile_tag, resolve as resolve_profile, DatasourceProfile,
    PROFILE_RECORD_VERSION, TABLE as PROFILE_TABLE,
};
#[cfg(feature = "datasource-profile")]
pub use profile_refresh::{
    federation_profile_refresh, profile_refresh_descriptor, PROFILE_JOB_KIND,
};
#[cfg(feature = "datasource-profile")]
pub use react_to_profiles::{
    react_to_profiles, spawn_profile_reactors, ProfilePass, ProfileReactorConfig, PROFILE_PERIOD,
};
pub use record::{
    datasource_tag, put as put_datasource, resolve as resolve_datasource, Datasource, TABLE,
};
pub use remove::datasource_remove;
pub use sample::{federation_sample, sample_descriptor};
pub use schema::{federation_schema, schema_descriptor};
pub use test::datasource_test;
pub use tool::call_federation_tool;
pub use write::{federation_write, write_descriptor};
