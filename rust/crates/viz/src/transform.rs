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

/// Run the pipeline, capturing the frames AFTER each APPLIED step — the per-step debug view (viz
/// transformations scope; editor-parity step 7). A `disabled` step is skipped and produces NO snapshot
/// (it didn't run). `stop_at`, when `Some(n)`, runs only the first `n` APPLIED steps (0 = the raw input
/// only). The returned Vec is the input snapshot (`step = None`) followed by one snapshot per applied
/// step: `(applied_step_index, frames)` — the pipeline index of the step that produced this snapshot.
pub fn transform_stepwise(
    mut frames: Frames,
    pipeline: &[Transformation],
    stop_at: Option<usize>,
) -> Vec<(Option<usize>, Frames)> {
    let mut out: Vec<(Option<usize>, Frames)> = vec![(None, frames.clone())];
    let mut applied = 0usize;
    for (i, step) in pipeline.iter().enumerate() {
        if step.disabled {
            continue;
        }
        if stop_at.is_some_and(|limit| applied >= limit) {
            break;
        }
        frames = apply_step(frames, step);
        out.push((Some(i), frames.clone()));
        applied += 1;
    }
    out
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

#[cfg(test)]
mod stepwise_tests {
    use super::*;
    use crate::frame::{Field, Frame};
    use serde_json::json;

    fn input() -> Frames {
        vec![Frame::new(vec![
            Field::new("a", vec![json!(3), json!(1), json!(2)]),
            Field::new("b", vec![json!("z"), json!("x"), json!("y")]),
        ])]
    }

    fn step(id: &str, options: Value) -> Transformation {
        Transformation { id: id.into(), options, disabled: false, filter: None, topic: None }
    }

    #[test]
    fn snapshots_input_plus_one_per_applied_step() {
        let pipeline = vec![
            step("sortBy", json!({"sort": [{"field": "a", "desc": false}]})),
            step("limit", json!({"limitField": 2})),
        ];
        let snaps = transform_stepwise(input(), &pipeline, None);
        // input + 2 applied steps = 3 snapshots.
        assert_eq!(snaps.len(), 3);
        assert_eq!(snaps[0].0, None); // the input snapshot
        assert_eq!(snaps[1].0, Some(0)); // after sortBy
        assert_eq!(snaps[2].0, Some(1)); // after limit
        // After sort: a ascending 1,2,3. After limit(2): first two rows.
        assert_eq!(snaps[1].1[0].field("a").unwrap().values, vec![json!(1), json!(2), json!(3)]);
        assert_eq!(snaps[2].1[0].length, 2);
    }

    #[test]
    fn stop_at_bounds_the_applied_steps() {
        let pipeline = vec![
            step("sortBy", json!({"sort": [{"field": "a", "desc": false}]})),
            step("limit", json!({"limitField": 1})),
        ];
        // stopAt = 1 → input + only the first applied step.
        let snaps = transform_stepwise(input(), &pipeline, Some(1));
        assert_eq!(snaps.len(), 2);
        assert_eq!(snaps[1].0, Some(0));
        // limit never ran → 3 rows, not 1.
        assert_eq!(snaps[1].1[0].length, 3);
    }

    #[test]
    fn stop_at_zero_is_the_input_only() {
        let pipeline = vec![step("limit", json!({"limitField": 1}))];
        let snaps = transform_stepwise(input(), &pipeline, Some(0));
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].0, None);
        assert_eq!(snaps[0].1[0].length, 3);
    }

    #[test]
    fn disabled_step_produces_no_snapshot() {
        let mut disabled = step("limit", json!({"limitField": 1}));
        disabled.disabled = true;
        let pipeline = vec![disabled, step("limit", json!({"limitField": 2}))];
        let snaps = transform_stepwise(input(), &pipeline, None);
        // input + ONE applied step (the disabled one is skipped).
        assert_eq!(snaps.len(), 2);
        assert_eq!(snaps[1].0, Some(1));
    }
}
