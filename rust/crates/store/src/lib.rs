//! The datastore — embedded SurrealDB, the one source of truth on every node (README §6.1).
//!
//! Tenancy mapping (§7): **workspace = SurrealDB namespace**. A [`Store`] handle is opened
//! once; each operation is scoped to a workspace, which selects the namespace before the
//! query runs. That makes workspace isolation *structural* at the store layer — a query for
//! workspace A physically cannot read namespace B's records.
//!
//! State only (§3.3): the store holds state; motion is the bus's job. No pub/sub here.

mod list;
mod open;
mod read;
mod record;
mod write;

pub use list::list;
pub use open::{Store, StoreError};
pub use read::read;
pub use write::write;
