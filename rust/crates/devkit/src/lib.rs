//! Lazybones extension developer kit.
//!
//! The first surface is artifact signing: one shared implementation for `lb-pack` and the host-side
//! SDK publish path, so the digest + Ed25519 idiom stays exactly the one the registry verifies.

mod artifacts;
mod build;
mod container_toolchain;
mod feature;
mod hex;
mod inspect;
mod manifest_id;
mod model;
mod publisher_key;
mod root;
mod scaffold;
mod signing;
mod template;
mod toolchain;

pub use build::build_extension;
pub use container_toolchain::{ContainerConfig, ContainerToolchain};
pub use feature::{feature_caps, Feature};
pub use inspect::inspect_extension;
pub use model::{
    Artifact, BuildReport, BuildRequest, BuildStatus, InspectReport, ScaffoldReport,
    ScaffoldRequest, TemplateInfo, Tier, ToolchainReadiness,
};
pub use publisher_key::{load_or_create_key, publisher_trust_line, LoadedPublisherKey};
pub use root::{default_devkit_root, resolve_under_root};
pub use scaffold::scaffold_extension;
pub use signing::sign_artifact;
pub use template::templates;
pub use toolchain::{ProcessToolchain, Toolchain};
