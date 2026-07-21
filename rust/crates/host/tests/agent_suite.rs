//! Aggregated `agent` integration tests.
//!
//! Cargo compiles every top-level `tests/*.rs` as its own crate, statically linking the whole
//! dependency graph (SurrealDB, Zenoh, wasmtime) into each one — ~1 GB per binary. The 29 agent
//! test files cost ~28 GB of `target/` on their own.
//!
//! Declaring them as modules of ONE harness keeps the file layout intact (one responsibility per
//! file, per `docs/FILE-LAYOUT.md`) while producing a single binary. The `#[path]` attributes mean
//! no file was renamed; `agent/` is just a directory Cargo does not auto-discover as targets.

#[path = "agent/agent_active_model_test.rs"]
mod agent_active_model_test;
#[path = "agent/agent_answer_fallback_test.rs"]
mod agent_answer_fallback_test;
#[path = "agent/agent_compact_test.rs"]
mod agent_compact_test;
#[path = "agent/agent_config_test.rs"]
mod agent_config_test;
#[path = "agent/agent_dangling_test.rs"]
mod agent_dangling_test;
#[path = "agent/agent_decision_test.rs"]
mod agent_decision_test;
#[path = "agent/agent_def_test_test.rs"]
mod agent_def_test_test;
#[path = "agent/agent_default_runtime_test.rs"]
mod agent_default_runtime_test;
#[path = "agent/agent_defs_test.rs"]
mod agent_defs_test;
#[path = "agent/agent_exfil_test.rs"]
mod agent_exfil_test;
#[path = "agent/agent_external_substrate_test.rs"]
mod agent_external_substrate_test;
#[path = "agent/agent_hardening_error_test.rs"]
mod agent_hardening_error_test;
#[path = "agent/agent_in_house_wiring_test.rs"]
mod agent_in_house_wiring_test;
#[path = "agent/agent_isolation_test.rs"]
mod agent_isolation_test;
#[path = "agent/agent_loop_detector_test.rs"]
mod agent_loop_detector_test;
#[path = "agent/agent_memory_test.rs"]
mod agent_memory_test;
#[path = "agent/agent_offline_test.rs"]
mod agent_offline_test;
#[path = "agent/agent_page_context_test.rs"]
mod agent_page_context_test;
#[path = "agent/agent_persona_catalog_test.rs"]
mod agent_persona_catalog_test;
#[path = "agent/agent_persona_coding_test.rs"]
mod agent_persona_coding_test;
#[path = "agent/agent_persona_session_test.rs"]
mod agent_persona_session_test;
#[path = "agent/agent_persona_test.rs"]
mod agent_persona_test;
#[path = "agent/agent_rehydrate_test.rs"]
mod agent_rehydrate_test;
#[path = "agent/agent_routed_test.rs"]
mod agent_routed_test;
#[path = "agent/agent_runtime_seam_test.rs"]
mod agent_runtime_seam_test;
#[path = "agent/agent_runtimes_test.rs"]
mod agent_runtimes_test;
#[path = "agent/agent_skill_test.rs"]
mod agent_skill_test;
#[path = "agent/agent_test.rs"]
mod agent_test;
#[path = "agent/agent_watch_test.rs"]
mod agent_watch_test;
