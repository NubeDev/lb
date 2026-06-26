//! Registry record shapes (registry scope): the signed [`Artifact`], the [`VerifiedArtifact`]
//! capability newtype, and the bytes-free [`CatalogEntry`].

mod artifact;
mod catalog;

pub use artifact::{Artifact, VerifiedArtifact};
pub use catalog::{CatalogEntry, Visibility};
