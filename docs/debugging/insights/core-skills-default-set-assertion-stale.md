# `core_skills_test` hardcoded a 3-element default set that grew to 21 — `cargo test` aborted before the insights suite ran

- Area: insights/skills + test-hygiene
- Status: resolved
- First seen: 2026-07-05
- Resolved: 2026-07-05
- Session: ../../sessions/insights/insights-session.md
- Regression test: `rust/crates/host/tests/core_skills_test.rs::the_default_set_is_the_persona_grounding_set_with_an_env_override`

## Symptom

`cargo test --workspace` exited `101` at `core_skills_test::the_default_set_is_the_read_only_core_skills_with_an_env_override`
with `left: [21 elements incl. core.insights]` ≠ `right: ["core.lb-cli","core.query","core.store-read"]`.
Because `cargo test --workspace` stops at the first failing test binary, **none** of the
alphabetically-later `insights_test` / `insight_routes_test` / `ladder_test` binaries ran — so the
insights suite looked "not run" rather than green.

## Reproduce

`cargo test --workspace` from `rust/`. The `core_skills_test` binary fails the exact-array
`assert_eq!` and cargo aborts the whole run before the insights binaries.

## Investigation

`DEFAULT_CORE_SKILLS` (`crates/host/src/workspaces/default_skills.rs:38`) is the **persona-
grounding set** — every grounding skill a built-in persona in `personas/personas.toml` pins. It was
grown from the old 3-skill read-only set to the full ~21-skill grounding set by the
persona-grounding session, and `core.insights` was added with the insights-analyst persona. The
test assertion (`core_skills_test.rs:310`) was never updated — it still hardcoded the 3-element
array. So the test was stale, not the constant.

Ruled out: the constant (it's correct — the persona-grounding scope decided this set); the seeded
SKILL.md bodies (`creating_a_workspace_applies_the_default_core_skill_grants` iterates the whole set
and each loads — green, proving the SKILL.md corpus is complete incl. `core.insights`). The one
stale line was the assertion.

## Root cause

A brittle whole-array equality on a set that's documented to grow (the persona catalog adds → the
default set adds). The test rotted the moment the catalog grew; `core.insights` was just the entry
that surfaced it for this session.

## Fix

`rust/crates/host/tests/core_skills_test.rs:308` — renamed to
`the_default_set_is_the_persona_grounding_set_with_an_env_override` and replaced the brittle
`assert_eq!(DEFAULT_CORE_SKILLS, &[…3 fixed…])` with **invariants + canonical-member spot-checks**:
non-empty; every id `core.`-prefixed; canonical members present (`core.lb-cli`, `core.query`,
`core.store-read`, `core.panels`, `core.rules`, `core.insights`); `core.secrets` NOT present
(write-driving skills stay out of the read-only defaults). The env-override checks are unchanged.

Rejected alternative: hardcode the new 21-element array. Rejected because it would rot again on
the next persona — the test's *intent* is "the set is the right grounding set", not "the set is
exactly these 21 strings".

## Verification

`cargo test -p lb-host --test core_skills_test` → 11 passed; 0 failed. `cargo test --workspace`
now proceeds past `core_skills_test` and the insights binaries run.

## Prevention

Invariant-based assertions on grown-by-design sets, not exact-array equality. The renamed test
states the growth contract in its comment.
