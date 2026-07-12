//! External-agent umbrella **swap test** (the load-bearing requirement) + the runtime-seam #1
//! registration + the run-lifecycle #5 scratch-dir isolation seal. These are offline projection/
//! config tests — no model, no subprocess (rule 9: a config + registry boundary, not a fake backend).
//! The real-subprocess run is the opt-in smoke in `smoke_test.rs`.
//!
//! The swap proof: the SAME code path drives Open Interpreter and VT Code purely by changing the
//! profile id — no code change. Here that is `resolve_builtin(id)` returning different (binary,
//! wrapper) pairs while `AcpRuntime` is one type; and `register(..)` populating a host registry with
//! both, so `agent.invoke { runtime: <id> }` selects either through the identical seam.

use std::sync::Arc;

use lb_host::{ErasedModel, RuntimeRegistry, Turn, TurnError};
use lb_role_external_agent::profiles::{
    default_model_endpoint, resolve_builtin, CODEX_DEFAULT, OPEN_INTERPRETER_DEFAULT,
    VTCODE_DEFAULT,
};
use lb_role_external_agent::scratch::ScratchDir;
use lb_role_external_agent::{register, AcpRuntime};

// --- The swap proof: one code path, profile-only difference ------------------------------------

#[test]
fn open_interpreter_and_vtcode_are_a_profile_change_only() {
    let oi = resolve_builtin(OPEN_INTERPRETER_DEFAULT, default_model_endpoint())
        .expect("open-interpreter is a built-in profile");
    let vt = resolve_builtin(VTCODE_DEFAULT, default_model_endpoint())
        .expect("vtcode is a built-in profile");

    // Same seam, DIFFERENT binary + wrapper — the entire difference is DATA in the profile/wrapper,
    // never code. This is the umbrella swap test in miniature.
    assert_eq!(oi.profile.binary, "interpreter");
    assert_eq!(vt.profile.binary, "vtcode");
    assert_ne!(oi.profile.binary, vt.profile.binary);
    assert_eq!(oi.wrapper.id(), "codex"); // OI is a Codex fork → codex-family shim
    assert_eq!(vt.wrapper.id(), "vtcode");
}

#[test]
fn open_interpreter_and_codex_differ_only_by_binary_same_wrapper() {
    // The cleanest swap: OI and Codex share the codex-family wrapper; ONLY the binary differs — a
    // second agent for zero new code.
    let oi = resolve_builtin(OPEN_INTERPRETER_DEFAULT, default_model_endpoint()).unwrap();
    let cx = resolve_builtin(CODEX_DEFAULT, default_model_endpoint()).unwrap();
    assert_eq!(oi.wrapper.id(), cx.wrapper.id(), "same codex-family shim");
    assert_ne!(
        oi.profile.binary, cx.profile.binary,
        "only the binary differs"
    );
    assert_eq!(cx.profile.binary, "codex");
}

#[test]
fn an_unknown_profile_id_is_not_a_builtin() {
    assert!(resolve_builtin("nope", default_model_endpoint()).is_none());
}

// --- Runtime-seam #1: register the external runtimes into a host registry ----------------------

/// A trivial erased model for the default entry (never called in these config tests — the external
/// runtimes don't use `ModelAccess`, and we don't drive the default here).
struct NullModel;
impl ErasedModel for NullModel {
    fn turn_boxed<'a>(
        &'a self,
        _ws: &'a str,
        _messages: &'a [(String, String)],
        _tools: &'a [lb_host::AllowedTool],
        _prior: &'a [lb_host::CallOutcome],
        _key: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Turn, TurnError>> + Send + 'a>>
    {
        Box::pin(async {
            Ok(Turn {
                content: String::new(),
                calls: vec![],
                done: true,
            })
        })
    }

    fn is_configured(&self) -> bool {
        false
    }
}

#[test]
fn register_populates_the_registry_with_all_builtin_profiles_plus_default() {
    let mut registry = RuntimeRegistry::with_default(Arc::new(NullModel));
    // Feature-ON node's registration hook does exactly this.
    register(&mut registry, default_model_endpoint(), None);

    let ids = registry.ids();
    // default is always present; the three built-in external profiles are registered on top.
    for expected in [
        "default",
        OPEN_INTERPRETER_DEFAULT,
        VTCODE_DEFAULT,
        CODEX_DEFAULT,
    ] {
        assert!(
            ids.contains(&expected.to_string()),
            "registry has {expected}"
        );
    }

    // Each external id resolves to a runtime whose `id()` echoes the profile id — the SAME AcpRuntime
    // type behind every entry (the "one runtime, profiles for difference" invariant).
    for id in [OPEN_INTERPRETER_DEFAULT, VTCODE_DEFAULT, CODEX_DEFAULT] {
        let rt = registry.resolve(Some(id)).expect("registered");
        assert_eq!(rt.id(), id);
    }
}

#[test]
fn a_default_only_registry_has_no_external_entries() {
    // The feature-OFF posture: without calling `register`, the registry is default-only.
    let registry = RuntimeRegistry::with_default(Arc::new(NullModel));
    assert_eq!(registry.ids(), vec!["default".to_string()]);
    assert!(registry.resolve(Some(OPEN_INTERPRETER_DEFAULT)).is_err());
}

#[test]
fn acp_runtime_new_is_none_for_an_unknown_profile() {
    assert!(AcpRuntime::new("nope", default_model_endpoint(), std::env::temp_dir()).is_none());
}

// --- Run-lifecycle #5: the per-run scratch-dir isolation seal ----------------------------------

#[test]
fn two_runs_in_different_workspaces_get_disjoint_scratch_dirs() {
    let base = std::env::temp_dir().join("lb-extagent-test-ws");
    let a = ScratchDir::create(&base, "ws-a", "job-1").expect("scratch a");
    let b = ScratchDir::create(&base, "ws-b", "job-1").expect("scratch b");
    // Distinct roots per workspace — a ws-A run's tree can never appear under ws-B.
    assert_ne!(a.path(), b.path());
    assert!(a.path().to_string_lossy().contains("ws-a"));
    assert!(b.path().to_string_lossy().contains("ws-b"));
    assert!(a.path().exists() && b.path().exists());
}

#[test]
fn two_runs_in_the_same_workspace_get_separate_scratch_dirs() {
    // Same-ws separation: two concurrent runs in one workspace still don't stomp each other.
    let base = std::env::temp_dir().join("lb-extagent-test-samews");
    let r1 = ScratchDir::create(&base, "ws-x", "job-1").unwrap();
    let r2 = ScratchDir::create(&base, "ws-x", "job-2").unwrap();
    assert_ne!(r1.path(), r2.path());
}

#[test]
fn a_hostile_workspace_or_job_id_cannot_escape_the_base() {
    // Path-traversal seal: `..` / separators are sanitized to `_`, so the scratch dir is always
    // strictly under `base` — a hostile ws/job id can't climb out.
    let base = std::env::temp_dir().join("lb-extagent-test-escape");
    let s = ScratchDir::create(&base, "../../etc", "../../../root").expect("sanitized");
    assert!(
        s.path().starts_with(&base),
        "scratch stays under base: {:?}",
        s.path()
    );
    assert!(!s.path().to_string_lossy().contains(".."));
}
