//! Lazybones extension developer kit.
//!
//! The **stable, published contract** (what an embedder pins by git tag, and what `lb-pack` uses) is
//! the artifact-signing surface only: [`sign_artifact`], [`load_or_create_key`],
//! [`publisher_trust_line`], [`LoadedPublisherKey`], and the signed [`Artifact`] type (re-exported
//! from `lb-registry`, the same type `verify_artifact` consumes — one crypto idiom, no second stack).
//!
//! Everything else (scaffold/build/inspect/templates/toolchains — the node-side devkit MCP verbs'
//! machinery) sits behind the **`devkit-full`** feature. It is on by default so in-workspace
//! consumers are unchanged, but it is *not* a semver contract for embedders: pin with
//! `default-features = false` to depend only on the stable pack surface. The future `lb-ext` CLI
//! (`ext-out-of-tree-scope.md`) stabilizes the rest.

mod hex;
mod manifest_id;
mod publisher_key;
mod signing;

// The stable pack-facing surface.
pub use lb_registry::Artifact;
pub use publisher_key::{load_or_create_key, publisher_trust_line, LoadedPublisherKey};
pub use signing::sign_artifact;

// The unstable node-side devkit surface (`devkit-full`, default-on).
#[cfg(feature = "devkit-full")]
mod artifacts;
#[cfg(feature = "devkit-full")]
mod build;
#[cfg(feature = "devkit-full")]
mod container_toolchain;
#[cfg(feature = "devkit-full")]
mod feature;
#[cfg(feature = "devkit-full")]
mod inspect;
#[cfg(feature = "devkit-full")]
mod model;
#[cfg(feature = "devkit-full")]
mod root;
#[cfg(feature = "devkit-full")]
mod scaffold;
#[cfg(feature = "devkit-full")]
mod template;
#[cfg(feature = "devkit-full")]
mod toolchain;
#[cfg(feature = "devkit-full")]
mod write_file;

#[cfg(feature = "devkit-full")]
pub use build::build_extension;
#[cfg(feature = "devkit-full")]
pub use container_toolchain::{ContainerConfig, ContainerToolchain};
#[cfg(feature = "devkit-full")]
pub use feature::{feature_caps, Feature};
#[cfg(feature = "devkit-full")]
pub use inspect::inspect_extension;
#[cfg(feature = "devkit-full")]
pub use model::{
    BuildArtifact, BuildReport, BuildRequest, BuildStatus, InspectReport, ScaffoldReport,
    ScaffoldRequest, TemplateInfo, Tier, ToolchainReadiness, WriteFileReport,
};
#[cfg(feature = "devkit-full")]
pub use root::{default_devkit_root, resolve_under_root};
#[cfg(feature = "devkit-full")]
pub use scaffold::scaffold_extension;
#[cfg(feature = "devkit-full")]
pub use template::templates;
#[cfg(feature = "devkit-full")]
pub use toolchain::{ProcessToolchain, Toolchain};
#[cfg(feature = "devkit-full")]
pub use write_file::write_file;
