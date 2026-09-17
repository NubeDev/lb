//! Unit tripwires for the [`super::gate_tool_for`] alias table — one module per verb family.
//!
//! Split out of `tool_gate.rs` when the alias table's own 400-line FILE-LAYOUT budget ran out. They
//! are FAST tripwires and nothing more: each asserts the string a verb's gate resolves to. None of
//! them proves the whole path works — a missing alias is `Denied`, not `NotFound`, so only a
//! POSITIVE integration call by a principal holding just the aliased cap catches that. Those live
//! with their planes (`plane_walls.rs` for the case verbs).

#[cfg(test)]
mod media_gate_tests {
    use super::super::gate_tool_for;

    /// **THE REGRESSION**: the outer gate asked for a cap that exists in no role bundle, so the
    /// whole upload surface was unreachable for every caller while the read verbs worked. Each of
    /// these three re-checks `media.upload` INSIDE itself (`begin.rs`, `chunk.rs`, `commit.rs`);
    /// the outer gate must ask the same question or the two gates disagree and the strictest wins.
    #[test]
    fn the_upload_phases_ride_the_one_upload_cap() {
        for verb in [
            "media.upload_begin",
            "media.chunk_write",
            "media.upload_commit",
        ] {
            assert_eq!(
                gate_tool_for(verb),
                "media.upload",
                "{verb} must gate on mcp:media.upload:call — no per-phase cap is minted"
            );
        }
    }

    /// `media_list` checks `media.get` inside (`get.rs`), so the outer gate must too — otherwise a
    /// caller holding only `mcp:media.get:call` passes the outer gate and is denied within.
    #[test]
    fn list_gates_on_the_same_cap_its_body_checks() {
        assert_eq!(gate_tool_for("media.list"), "media.get");
    }

    /// The verbs whose literal name IS their cap must stay unaliased — over-aliasing would widen
    /// `read`/`delete` onto a grant their bodies never check.
    #[test]
    fn the_self_named_media_verbs_are_untouched() {
        for verb in ["media.read", "media.get", "media.delete"] {
            assert_eq!(gate_tool_for(verb), verb);
        }
    }
}

#[cfg(test)]
mod ext_boards_gate_tests {
    use super::super::gate_tool_for;

    /// The two host-authored-ext-nav-boards verbs ride EXISTING nav caps — the read with every
    /// member's `nav.resolve`, the write with the admin's `nav.save`. No `nav.ext_boards.*` cap is
    /// minted, so without these aliases the outer gate would deny both for every caller while the
    /// direct-call tests stayed green (they never cross this gate).
    #[test]
    fn the_ext_board_verbs_ride_the_existing_nav_caps() {
        assert_eq!(gate_tool_for("nav.ext_boards.get"), "nav.resolve");
        assert_eq!(gate_tool_for("nav.ext_boards.set"), "nav.save");
    }

    /// The read must NOT ride the authoring cap: a board an admin places is rendered in EVERY
    /// reached member's rail, so gating its read on `nav.save` would make the feature invisible to
    /// exactly the people it exists for.
    #[test]
    fn the_read_is_member_level_not_admin() {
        assert_ne!(gate_tool_for("nav.ext_boards.get"), "nav.save");
    }
}

#[cfg(test)]
mod case_gate_tests {
    use super::super::gate_tool_for;

    /// The picker's roster rides the cap of the page it serves. A fast tripwire only — it does not
    /// replace the POSITIVE integration test in `plane_walls.rs`, which is what proves the whole
    /// path (gate + dispatch + verb) actually resolves for a caller holding only `case.list`.
    #[test]
    fn the_assign_picker_rides_the_queue_read_cap() {
        assert_eq!(gate_tool_for("case.assignees"), "case.list");
        // And it is NOT the write cap: a viewer who may see the queue may see who could own it.
        assert_ne!(gate_tool_for("case.assignees"), "case.workflow");
    }
}

#[cfg(test)]
mod report_gate_tests {
    use super::super::gate_tool_for;

    /// `report.export` reached the JSON bridge in the reports scope's Track A, and the FIRST
    /// question a new host-native verb has to answer is the one this file exists for: which cap
    /// does the outer gate actually demand, and does anything grant it?
    ///
    /// The answer here is "its own, and yes" — `mcp:report.export:call` is a concrete cap in the
    /// AUTHOR bundle (`authz/builtin_roles.rs`, beside `report.save`/`report.share`), deliberately
    /// NOT covered by any `mcp:*.*:call` wildcard because view-without-export is a real posture.
    /// So the fall-through arm is correct and no alias is needed.
    ///
    /// This test pins that fall-through rather than asserting nothing, because the failure it
    /// guards against is silent in both directions: an alias added later would quietly widen the
    /// export gate to whatever it aliased onto (handing export to everyone who can READ a report),
    /// and the absence of an alias is otherwise indistinguishable from nobody having thought about
    /// it. Every one of the four incidents this module documents was invisible until someone drove
    /// the verb on a live node.
    #[test]
    fn export_gates_on_its_own_concrete_cap() {
        assert_eq!(gate_tool_for("report.export"), "report.export");
    }

    /// The read verbs are viewer-level and must not be dragged up to the author's export cap.
    #[test]
    fn the_read_verbs_are_untouched() {
        for verb in ["report.get", "report.list"] {
            assert_eq!(gate_tool_for(verb), verb);
            assert_ne!(gate_tool_for(verb), "report.export");
        }
    }
}
