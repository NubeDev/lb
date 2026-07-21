//! `lb-viz` — the pure transformation library (viz transformations scope). The ONE implementation of
//! Grafana's transformer set (reduce/organize/filter/groupBy/joinByField/calculateField/sortBy/limit/
//! merge/seriesToRows) over a canonical columnar [`Frame`], for the whole platform. The structural
//! twin of `lb-prefs`: a pure lib (no store, no bus, no I/O) compiled into every node, run by the host
//! `viz.query` verb so every client (web shell, React Native, email, webhook) renders identical data
//! without re-implementing the pipeline.
//!
//! Entry: [`transform`] runs an ordered [`Transformation`] pipeline over [`Frames`]. The row↔frame
//! adapters at the lib's edge ([`Frame::from_rows`]/[`Frame::to_rows`]) let the resolver feed tool
//! results in and hand renderer-shaped rows out. FILE-LAYOUT: one transformer per `transforms/*.rs`.

mod config;
mod frame;
mod reducer;
mod transform;
mod transforms;

pub use config::{Matcher, Transformation};
pub use frame::{Field, FieldType, Frame, FrameState, FrameStatus, Frames};
pub use reducer::reduce_field;
pub use transform::{transform, transform_stepwise};
