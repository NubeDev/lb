//! The **extraction service** — derive markdown docs from binary media (doc-extraction scope).
//! Beside the doc verbs (`assets/`) because a derived doc IS a doc; separated into its own module
//! because extraction is a distinct verb family (`docs.*`) with its own chokepoint, job, ledger,
//! and derivation edges. Generic over mime, never a domain noun (rule 10).
//!
//! The layering mirrors the pure/host split the scope asks for:
//!   - `lb_extract` (the pure crate) knows *bytes → markdown*, nothing of the host;
//!   - this module is the host seam: the capability chokepoint (`authorize`), the `docs.extract`
//!     batch job (`extract`), the per-item derivation (`derive`), the provenance/idempotency
//!     ledger (`ledger` + `model`), and the `derived_from` edges — plus the `docs.*` MCP bridge
//!     (`tool`). One responsibility per file (FILE-LAYOUT).

mod authorize;
mod derive;
mod error;
mod extract;
mod ledger;
mod model;
mod tool;

pub use error::ExtractSvcError;
pub use extract::{docs_extract, ExtractResult};
pub use model::{ExtractRequest, Extraction, ItemOutcome, DERIVED_FROM, EXTRACTION_TABLE};
pub use tool::{call_docs_tool, extract_descriptor};

// Re-exported for the ledger/idempotency integration tests (the record count is the idempotency
// assertion) and any future orphan-GC over the derivation edge.
pub use ledger::get_extraction;
