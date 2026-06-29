//! The ordered pipeline (viz transformations scope, "An ordered pipeline, bounded by construction").
//! `transform(frames, &[Transformation])` applies each step **in array order**: a `disabled` step is
//! skipped (kept for round-trip), a step with a `filter` Matcher is scoped to the matching frames
//! (non-matching frames pass through untouched), and an **unsupported id is preserved-but-skipped**
//! (the resolver/editor surface the "unsupported — preserved, not applied" note; here we just don't
//! mangle the frames). One responsibility: dispatch + ordering + filter scoping. Each transformer's
//! math lives in its own `transforms/<id>.rs` (FILE-LAYOUT: one transformer per file).

use serde_json::Value;

use crate::config::{Matcher, Transformation};
use crate::frame::Frames;
use crate::transforms;

/// Run the pipeline. Pure + deterministic: same frames + same config → same output, no input mutation
/// (each step takes frames by value and returns new frames).
pub fn transform(mut frames: Frames, pipeline: &[Transformation]) -> Frames {
    for step in pipeline {
        if step.disabled {
            continue;
        }
        frames = apply_step(frames, step);
    }
    frames
}

/// Apply one step, honoring its `filter` Matcher. A frame-level filter (`byFrameRefID`) splits the
/// frames: matching frames go through the transformer, the rest pass through and are re-appended in
/// order. With no filter, the whole frame set goes through. (A field-level filter is the transformer's
/// own concern via its options; the step `filter` is the frame scope.)
fn apply_step(frames: Frames, step: &Transformation) -> Frames {
    match &step.filter {
        Some(m) if is_frame_filter(m) => {
            let (matched, passthrough): (Frames, Frames) =
                frames.into_iter().partition(|f| m.matches_frame(&f.ref_id));
            let mut out = dispatch(&step.id, matched, &step.options);
            out.extend(passthrough);
            out
        }
        _ => dispatch(&step.id, frames, &step.options),
    }
}

/// A step `filter` we interpret at the frame level (only `byFrameRefID` scopes whole frames here).
fn is_frame_filter(m: &Matcher) -> bool {
    m.id == "byFrameRefID"
}

/// The id → transformer dispatch. Each arm calls one `transforms::<id>::apply`. An unknown id returns
/// the frames unchanged (preserved-but-not-applied — honest degradation, never an error or a fake).
fn dispatch(id: &str, frames: Frames, options: &Value) -> Frames {
    match id {
        "reduce" => transforms::reduce::apply(frames, options),
        "organize" => transforms::organize::apply(frames, options),
        "filterFieldsByName" => transforms::filter_by_name::apply(frames, options),
        "filterByValue" => transforms::filter_by_value::apply(frames, options),
        "groupBy" => transforms::group_by::apply(frames, options),
        "joinByField" | "seriesToColumns" => transforms::join_by_field::apply(frames, options),
        "calculateField" => transforms::calculate_field::apply(frames, options),
        "sortBy" => transforms::sort_by::apply(frames, options),
        "limit" => transforms::limit::apply(frames, options),
        "merge" => transforms::merge::apply(frames, options),
        "seriesToRows" => transforms::series_to_rows::apply(frames, options),
        // Deferred/unknown id: preserved in config (the caller keeps it), not applied here.
        _ => frames,
    }
}
