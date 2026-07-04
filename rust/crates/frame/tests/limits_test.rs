//! The Frame input governors carry the scope defaults (Phase 0 names them; Phase 2 enforces them).

use lb_frame::FrameLimits;

#[test]
fn frame_limits_default_is_the_scope_value() {
    let l = FrameLimits::default();
    assert_eq!(l.max_frame_rows, 200_000);
    assert_eq!(l.max_frame_cells, 2_000_000);
    assert_eq!(l.max_string_bytes, 256 * 1024);
}
