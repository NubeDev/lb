//! Built-in external-agent **profiles** and the profile-id → (profile, wrapper) resolution. This is
//! the *swap unit* (external-agent umbrella "What a profile is"): every difference between
//! Open Interpreter, VT Code, and Codex is **data here**, never code in the runtime. Swapping the
//! external agent — the load-bearing requirement — is picking a different `profile_id`, resolved
//! here; the [`AcpRuntime`](crate::AcpRuntime) body is identical for all three.
//!
//! Open Interpreter is the **default** external agent (Apache-2.0 Codex fork, exercised vs Z.AI
//! GLM-4.6, ACP-native). It and Codex share the codex-family [`CodexWrapper`] — they differ **only**
//! by `AgentProfile.binary`, which is the cleanest possible proof of the seam (a second agent for
//! zero new code). VT Code is the alternate over its own [`VtcodeWrapper`].
//!
//! The model endpoint here is *config data* (provider/model/api-key-env NAME). In the shipped topic
//! it points at our gateway's OpenAI-compatible endpoint (model-routing #4); until #4 lands it is the
//! provider/model the binary is told to use, so the path is exercisable without the served gateway.

use lb_external_agent::{AgentProfile, AgentWrapper, CodexWrapper, ModelEndpoint, VtcodeWrapper};

/// The three built-in profile ids. `agent.invoke { runtime: <id> }` selects one; the registry (#1)
/// registers an [`AcpRuntime`](crate::AcpRuntime) per id when the feature is on.
pub const OPEN_INTERPRETER_DEFAULT: &str = "open-interpreter-default";
pub const VTCODE_DEFAULT: &str = "vtcode-default";
pub const CODEX_DEFAULT: &str = "codex-default";

/// The default model endpoint the built-in profiles use when the node config supplies none. Z.AI
/// GLM-4.6 via the **coding** endpoint (the verified, non-throttled path); the api-key env is
/// `ZAI_API_KEY` (the driver passes the *name*, never the value). A node config overrides this.
///
/// `provider` is `zaicoding` — a DISTINCT id, deliberately not codex's built-in `zai` (which points
/// at the throttled standard endpoint + wants `ZHIPU_API_KEY`). `base_url` is the coding-plan endpoint,
/// which the codex wrapper turns into `model_providers.zaicoding.*` overrides (with `wire_api=chat`).
pub fn default_model_endpoint() -> ModelEndpoint {
    ModelEndpoint {
        provider: "zaicoding".to_string(),
        model: "glm-4.6".to_string(),
        api_key_env: "ZAI_API_KEY".to_string(),
        base_url: Some("https://api.z.ai/api/coding/paas/v4".to_string()),
    }
}

/// A resolved external agent: its profile (config data) + the wrapper (argv+decode shim). The runtime
/// holds one of these per registered id. Both are cheap/stateless — a wrapper is zero-sized.
pub struct ResolvedAgent {
    pub profile: AgentProfile,
    pub wrapper: Box<dyn AgentWrapper>,
}

/// Resolve a built-in profile id to its (profile, wrapper) pair over `model`. Returns `None` for an
/// id this crate does not ship a built-in profile for (the node config could add more later; the
/// registry treats an unknown id as an error at the seam, #1). **This is the swap point**: the SAME
/// runtime code drives whichever pair this returns.
pub fn resolve_builtin(id: &str, model: ModelEndpoint) -> Option<ResolvedAgent> {
    match id {
        OPEN_INTERPRETER_DEFAULT => Some(ResolvedAgent {
            profile: AgentProfile::open_interpreter_default(model),
            // Open Interpreter is a Codex fork — the codex-family shim drives it unchanged.
            wrapper: Box::new(CodexWrapper),
        }),
        CODEX_DEFAULT => Some(ResolvedAgent {
            profile: AgentProfile::codex_default(model),
            wrapper: Box::new(CodexWrapper),
        }),
        VTCODE_DEFAULT => Some(ResolvedAgent {
            profile: AgentProfile::vtcode_default(model),
            wrapper: Box::new(VtcodeWrapper),
        }),
        _ => None,
    }
}

/// The built-in profile ids this crate ships, in a stable order. Used by the registration hook to
/// register one runtime per id, and (later) by the `agent.runtimes` read verb (#5).
pub const BUILTIN_IDS: &[&str] = &[OPEN_INTERPRETER_DEFAULT, VTCODE_DEFAULT, CODEX_DEFAULT];
