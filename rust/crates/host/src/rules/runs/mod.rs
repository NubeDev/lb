//! Job-backed rule runs (long-running-rules-scope): `rules.run_async` + the `rules.runs.*`
//! observe/control family. One verb per file (FILE-LAYOUT); the worker drives the eval, the
//! registry holds the live cooperative controls, the seam persists checkpoints/progress onto the
//! `lb-jobs` transcript.

mod cancel;
mod get;
mod list;
mod payload;
mod registry;
mod resume;
mod seam;
mod start;
mod suspend;
mod worker;

pub use cancel::rules_runs_cancel;
pub use get::rules_runs_get;
pub use list::rules_runs_list;
pub use registry::RuleRunMap;
pub use resume::rules_runs_resume;
pub use start::rules_run_async;
pub use suspend::rules_runs_suspend;
pub use worker::RULE_RUN_KIND;
