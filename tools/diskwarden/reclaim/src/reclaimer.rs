//! The one seam every cleaner implements.
//!
//! Adding a cleaner = adding one file under `reclaimers/` that implements this
//! trait, plus one line in `reclaimers::all()`. Nothing else in the tool learns its
//! name: `scan` and `clean` iterate the trait objects and treat the id as opaque
//! data. That is what keeps a new reclaimer from touching the core.
//!
//! The trait splits *finding* from *freeing* on purpose. `scan` is read-only and is
//! what the cron timer and the tray run. `reclaim` is the only method that deletes,
//! and the driver only calls it for candidates the policy explicitly enabled.

use std::path::PathBuf;

use crate::candidate::Candidate;

/// What a scan is allowed to look at.
#[derive(Debug, Clone)]
pub struct ScanCtx {
    /// Directories to search under (e.g. `~/code`).
    pub roots: Vec<PathBuf>,
    /// "Now", as seconds since the unix epoch. Injected so age policy is testable
    /// without sleeping or touching the wall clock.
    pub now_secs: u64,
}

/// A category of reclaimable disk.
pub trait Reclaimer: Send + Sync {
    /// Stable, kebab-case id. Also the policy key (`[reclaimer.<id>]`).
    fn id(&self) -> &'static str;

    /// One line for the UI.
    fn describe(&self) -> &'static str;

    /// Find everything this reclaimer could free. READ-ONLY: implementations must
    /// not delete, move, or write anything.
    fn scan(&self, ctx: &ScanCtx) -> anyhow::Result<Vec<Candidate>>;

    /// Free one candidate. Returns bytes actually freed.
    ///
    /// Only ever called for a candidate this reclaimer produced, and only after the
    /// policy gate passed. Implementations must be idempotent: reclaiming something
    /// already gone is `Ok(0)`, not an error.
    fn reclaim(&self, candidate: &Candidate) -> anyhow::Result<u64>;
}
