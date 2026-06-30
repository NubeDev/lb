//! An [`AgentProfile`] — the *config data* that turns one external-agent binary into a specific
//! agent (external-agent-scope §"What a profile is"). It is deliberately **pure data**: which binary
//! to spawn, the model endpoint it talks to, and the resume key. Tools + persona (the grant-gated
//! parts) live with the capability wall (#3) and are not modelled here yet — this slice is the
//! *driving* seam, not the wall.
//!
//! The driver in [`crate::driver`] turns a profile + a goal into a spawned subprocess. Nothing here
//! reaches the network or the store; it is the swappable knob the umbrella scope calls "a profile
//! decision, not code".

/// How a profile resolves the model the external agent uses. In the shipped topic this points at
/// **our** gateway's OpenAI-compatible endpoint (model-routing, #4); standalone-here it is just the
/// provider/model the binary is told to use, so the crate is exercisable against a real vtcode without
/// the gateway yet built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelEndpoint {
    /// Provider id the agent CLI understands (e.g. `"zai"`, `"anthropic"`).
    pub provider: String,
    /// Model id the agent CLI understands (e.g. `"glm-5.2"`, `"claude-sonnet-4-6"`).
    pub model: String,
    /// Name of the env var the agent reads the API key from (e.g. `"ZAI_API_KEY"`). The driver does
    /// **not** hold the key value — only the var name, so the secret never lands in a profile struct.
    pub api_key_env: String,
}

/// One configured external agent. `id` is what `agent.invoke { runtime: id }` selects once the seam
/// (#1) is wired; today it only labels the profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProfile {
    /// Registry id — the `runtime` selector (e.g. `"vtcode-default"`).
    pub id: String,
    /// Absolute path (or `PATH`-resolvable name) of the agent binary to spawn.
    pub binary: String,
    /// The model the agent runs against.
    pub model: ModelEndpoint,
}

impl AgentProfile {
    /// The documented default: VT Code over the given model, the umbrella's reference profile. Used by
    /// the real-subprocess smoke test and as the example a node config would mirror. Pair with
    /// [`VtcodeWrapper`](crate::wrappers::VtcodeWrapper).
    pub fn vtcode_default(model: ModelEndpoint) -> Self {
        Self {
            id: "vtcode-default".to_string(),
            binary: "vtcode".to_string(),
            model,
        }
    }

    /// A codex **example** profile — same shape, different binary. Codex is a *future* target, not a
    /// shipped integration (see [`CodexWrapper`](crate::wrappers::CodexWrapper)); this constructor
    /// exists so the "swap" (pair a different wrapper + profile, no driver change) is expressible and
    /// tested, proving the seam accounts for codex before it is actually wired.
    pub fn codex_default(model: ModelEndpoint) -> Self {
        Self {
            id: "codex-default".to_string(),
            binary: "codex".to_string(),
            model,
        }
    }

    /// An **Open Interpreter** profile — `interpreter` is an Apache-2.0 Rust *fork of Codex* ("a coding
    /// agent for low-cost models"), so it shares Codex's `exec --json` + ACP wire and reuses the **same**
    /// [`CodexWrapper`](crate::wrappers::CodexWrapper) — *only the binary differs*. This is the cleanest
    /// demonstration of the seam: a whole second agent for **zero** new code, just a profile row. Also a
    /// future target (not driven against a real binary here yet).
    pub fn open_interpreter_default(model: ModelEndpoint) -> Self {
        Self {
            id: "open-interpreter-default".to_string(),
            binary: "interpreter".to_string(),
            model,
        }
    }
}
