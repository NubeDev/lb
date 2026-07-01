//! Host service for the local-only extension SDK verbs.
//!
//! The service is intentionally thin: `lb-devkit` owns filesystem rendering/building, while this module
//! owns the Lazybones gates, job record, and bus log publication.

mod authorize;
mod build;
mod error;
mod inspect;
mod root;
mod scaffold;
mod templates;
mod tool;

pub use authorize::authorize_devkit;
pub use build::{devkit_build, BuildStarted};
pub use error::DevkitError;
pub use inspect::devkit_inspect;
pub use root::{devkit_root, DevkitRoot};
pub use scaffold::devkit_scaffold;
pub use templates::devkit_templates;
pub use tool::call_devkit_tool;
