//! `LB_STORE_MAX_BYTES` → `BootConfig::store_budget_bytes` (disk-budget scope, slice 1). The env
//! seam is the ONE place `LB_*` is read; this pins the three cases the scope's testing plan names:
//! unset ⇒ `None` (today's behaviour), set ⇒ the byte count, malformed ⇒ warn and fall back to
//! `None` — never a panic in boot config.
//!
//! One `#[test]` on purpose: env is process-global, so the cases run in sequence rather than
//! racing each other across cargo's test threads (the `credential_mode_test` precedent).

use lb_node::BootConfig;

#[test]
fn store_budget_parses_unset_set_and_malformed() {
    // Unset ⇒ no budget: the flat 256 MiB advisory, no marks, nothing auto-triggers.
    std::env::remove_var("LB_STORE_MAX_BYTES");
    assert_eq!(
        BootConfig::from_env().store_budget_bytes,
        None,
        "unset ⇒ today's exact behaviour (decision 2: no auto-derivation)"
    );

    // Empty / whitespace-only is unset, not zero.
    for blank in ["", "   "] {
        std::env::set_var("LB_STORE_MAX_BYTES", blank);
        assert_eq!(BootConfig::from_env().store_budget_bytes, None);
    }

    // Set ⇒ a plain byte count, surrounding whitespace tolerated.
    std::env::set_var("LB_STORE_MAX_BYTES", "4294967296");
    assert_eq!(
        BootConfig::from_env().store_budget_bytes,
        Some(4 * 1024 * 1024 * 1024)
    );
    std::env::set_var("LB_STORE_MAX_BYTES", " 1048576 ");
    assert_eq!(BootConfig::from_env().store_budget_bytes, Some(1_048_576));

    // Malformed ⇒ warn + fall back to `None`. The assertion is that this RETURNS at all: a panic
    // here would take the binary down at boot over a typo.
    for bad in [
        "4GB",
        "-1",
        "1.5",
        "not-a-number",
        "99999999999999999999999",
    ] {
        std::env::set_var("LB_STORE_MAX_BYTES", bad);
        assert_eq!(
            BootConfig::from_env().store_budget_bytes,
            None,
            "malformed '{bad}' falls back to no budget"
        );
    }

    // Leave the env as we found it for anything else in this binary.
    std::env::remove_var("LB_STORE_MAX_BYTES");
    assert_eq!(BootConfig::from_env().store_budget_bytes, None);
}
