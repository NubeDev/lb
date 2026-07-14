//! diskwarden's pure scan/reclaim core.
//!
//! No CLI, no tray, no notifications, no timer — this crate only knows how to find
//! reclaimable disk and (when the policy says so) free it. The `app` crate wraps it.
//!
//! The safety posture in one line: **scanning is read-only and unconditional;
//! deleting requires an explicit per-reclaimer opt-in** (`policy::Policy::gate`).

pub mod candidate;
pub mod policy;
pub mod reclaimer;
pub mod reclaimers;
pub mod report;
pub mod size;

pub use candidate::Candidate;
pub use policy::{Gate, Policy};
pub use reclaimer::{Reclaimer, ScanCtx};
pub use report::{Finding, ScanReport};
