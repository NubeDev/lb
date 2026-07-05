//! `agent.def.test {id?}` — the **context-proving diagnostic** (agent-catalog test-and-secrets scope).
//! A single-turn invoke of a catalog definition's model with the run's **real context assembled** —
//! the shipped system prompt, the `reachable_tools` menu (the MCP/ACP tool surface), and the
//! `render_catalog` granted-skill list — over a canned self-describe prompt, returning what was
//! assembled so a workspace admin can confirm the agent has its real Lazybones context.
//!
//! **Why one turn with real context, not a bare ping.** A ping proves the endpoint resolves; it does
//! NOT prove the agent knows what it is. Assembling the real context and returning it is the only
//! thing that answers "does it know it has MCP/ACP/skills" against the mock AND a real provider. See
//! the scope's "Intent / approach §1".
//!
//! **Bounded + side-effect-free.** Step ceiling of ONE turn, executes NO tools (the returned
//! `calls` are ignored — the model answers from the injected context), and persists NO session /
//! transcript. It is a diagnostic, not an `agent.invoke`.
//!
//! **The wall is inherited, not widened.** `reachable_tools` + `render_catalog` are both ws- +
//! grant-gated for the CALLER — the test sees exactly the tools/skills a real run for this caller
//! would, never another tenant's. The gate is `mcp:agent.def.test:call` (admin-tier — the test spends
//! model budget); a `builtin.*`/custom id resolves like `agent.def.get` (the same namespaces).
//!
//! **No key echo.** The endpoint's key is NOT injected into the prompt/context — it goes only to the
//! provider transport (the model build). So structurally the returned `answer` cannot contain it;
//! [`crate::agent::resolve_endpoint_key`] resolves it out-of-band. A test asserts the answer is
//! key-free.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde::Serialize;

use crate::agent::{reachable_tools, render_catalog, SYSTEM_PROMPT};
use crate::assets::list_granted_skills;
use crate::boot::Node;

/// The fixed self-describe prompt (scope open-question "canned prompt": a fixed, well-crafted prompt
/// for v1 so the test is comparable across definitions). It asks the model to name what it is and
/// what it can reach — the "does it know it has MCP/ACP/skills" check.
const SELF_DESCRIBE: &str =
    "Who are you, and what tools and skills do you have access to? Describe your Lazybones context.";

/// The derived-actor sub for the test's single turn — audit shows the diagnostic acted, distinct from
/// a real `agent:session`.
const TEST_SUB: &str = "agent:def-test";

/// Cap the context lists returned to the browser so a huge grant set can't bloat the DTO (scope
/// "bound the list length"). Names carry no secret, but the payload stays small.
const MAX_LISTED: usize = 200;

/// What a test returns: the model's `answer`, the resolved `runtime`/`model`, the assembled `context`
/// (tool + skill NAMES, so "it has MCP/ACP/skills" is concrete), and honest `provider_configured` /
/// `ok` flags. Names-only — no key, no secret.
#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    /// The definition id that was tested (resolved from the active pick when `id` was omitted).
    pub id: String,
    /// The model's single-turn text. Against the `UnconfiguredModel`/`MockProvider` this is the
    /// deterministic canned reply; the `context` below is what makes the test meaningful pre-adapter.
    pub answer: String,
    /// The runtime this definition binds.
    pub runtime: String,
    /// The model endpoint's `provider/model` (names only — the display label, never a key).
    pub model: String,
    /// The real assembled context, proving the pipe fed the agent its Lazybones surface.
    pub context: TestContext,
    /// Whether the node's model is a REAL provider vs. the `UnconfiguredModel` placeholder — the
    /// honest "responding via the configured provider" signal (never implies a real LLM answered
    /// when the placeholder did).
    pub provider_configured: bool,
    /// True when the turn completed (a diagnostic success signal). A resolve failure returns an error
    /// before this, so a returned `TestResult` is always `ok: true` in v1.
    pub ok: bool,
}

/// The context the test assembled for the caller — counts + the (bounded) names.
#[derive(Debug, Clone, Serialize)]
pub struct TestContext {
    pub tool_count: usize,
    pub tools: Vec<String>,
    pub skill_count: usize,
    pub skills: Vec<String>,
}

/// Run the context-proving test for definition `id` (or the workspace's active `agent.config` pick
/// when `id` is `None`) in `ws` for `caller`. Gated by `mcp:agent.def.test:call`.
///
/// It assembles the caller's REAL run context (system prompt + reachable tools + granted skills) and
/// runs ONE model turn over the node's default model with the canned self-describe prompt — no tool
/// execution, no durable run. The endpoint's key is resolved for the model build (out-of-band), never
/// injected into the context, so the answer cannot echo it.
pub async fn agent_def_test(
    node: &std::sync::Arc<Node>,
    caller: &Principal,
    ws: &str,
    id: Option<&str>,
) -> Result<TestResult, ToolError> {
    // Gate: the test spends model budget, so it rides its own admin-tier cap (distinct from the
    // read-ish `agent.def.list`) — opaque `Denied`. Workspace-first is inside `authorize_tool`.
    authorize_tool(caller, ws, "agent.def.test").map_err(|_| ToolError::Denied)?;

    // (1) Resolve the target definition: the given id, or the workspace's active pick. The shared
    // `resolve_active_definition` re-runs `agent_def_get`'s OWN member gate + the built-in/custom
    // namespace split (the hard wall), so the test can never reach a definition the caller couldn't
    // `get`. Promoted out of this file so the UI badge, rules, and the model resolver all agree.
    let def = crate::agent::resolve_active_definition(node, caller, ws, id).await?;

    // (2) Assemble the REAL run context for the caller, exactly as `run.rs` does at run start — under
    // the caller's own identity (so the member-scoped grants resolve to the human behind the run) with
    // the caller's caps (the test never widens the wall). Both reads are best-effort context, never a
    // gate: a caller with fewer grants simply sees fewer tools/skills (§2.2, inherits the wall).
    let actor = caller.derive(TEST_SUB, caller.caps().to_vec());

    let mut tools: Vec<String> = reachable_tools(node, &actor, ws)
        .await
        .into_iter()
        .map(|t| t.name)
        .collect();
    let tool_count = tools.len();
    tools.truncate(MAX_LISTED);

    let mut skills: Vec<String> = list_granted_skills(&node.store, &actor, ws)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|e| e.id)
        .collect();
    let skill_count = skills.len();
    skills.truncate(MAX_LISTED);

    // (3) Build the single-turn conversation: the shipped system prompt + the assembled context
    // messages (the tool/skill catalogs, framed exactly as the loop injects them) + the canned prompt.
    // The KEY is never a message — it goes only to the provider transport (the model build), so the
    // answer is structurally key-free.
    let mut messages: Vec<(String, String)> = vec![("system".into(), SYSTEM_PROMPT.to_string())];
    if let Some(catalog) = render_catalog(node, &actor, ws)
        .await
        .map_err(|_| ToolError::Denied)?
    {
        messages.push(("system".into(), catalog));
    }
    messages.push(("user".into(), SELF_DESCRIBE.to_string()));

    // (4) Run one bounded turn against the definition's ACTUAL runtime.
    //
    // Two paths, because a definition binds one of two runtime kinds — the test must exercise the SAME
    // one a real run would, or it is not a test of THIS definition:
    //   - **External** (e.g. `open-interpreter-default`): drive that registered runtime for one bounded
    //     turn via `invoke_via_runtime`. This spawns the real agent subprocess, resolves the sealed key
    //     the SAME way a real run does (host-mediated, secret → env), and returns its real answer — the
    //     only thing that proves the sealed key + external provider actually work. Requires the caller
    //     hold `mcp:agent.invoke:call` (the run gate); absent it, an honest error, never a placeholder.
    //   - **In-house `default`**: one turn over the node's `default_model` (the in-house `ModelAccess`),
    //     as before — its `is_configured()` is the honest "a real provider is wired" signal.
    let registry = node.runtimes();
    let is_external =
        def.runtime != crate::agent::DEFAULT_RUNTIME && registry.ids().contains(&def.runtime);

    let (answer, provider_configured) = if is_external {
        // Drive the real external runtime for one bounded self-describe turn. The runtime resolves the
        // definition's sealed model key itself (host-mediated) and spawns the agent — a true end-to-end
        // test of the SELECTED runtime, not the in-house placeholder.
        let job_id = format!("{ws}:agent-def-test:{}", def.id);
        let out = crate::agent::invoke_via_runtime(
            node,
            &registry,
            Some(&def.runtime),
            // `agent.def.test` is a runtime/model diagnostic (a self-describe turn), NOT a persona
            // preview — drive with no persona so the test isolates "does this (runtime, model, key)
            // work". Previewing the active persona's context is a separate follow-up (scope open-Q 2).
            None,
            caller,
            &caller.caps().to_vec(),
            ws,
            &job_id,
            SELF_DESCRIBE,
            crate::agent::Substrate::default(),
            None,
            &[],
            0,
        )
        .await;
        match out {
            Ok(answer) => (answer, true),
            // A missing binary / unresolved key / run failure surfaces as an honest error string in the
            // answer with `provider_configured=false` — never a silent placeholder that looks configured.
            Err(e) => (format!("external runtime test failed: {e}"), false),
        }
    } else {
        // In-house `default`: one turn over the node's default model, ignoring proposed calls.
        let model = registry.default_model();
        let configured = model.is_configured();
        let turn = model
            .turn_boxed(ws, &messages, &[], &[], &format!("{ws}:agent-def-test"))
            .await;
        (turn.content, configured)
    };

    Ok(TestResult {
        id: def.id,
        answer,
        runtime: def.runtime,
        model: format!(
            "{}/{}",
            def.model_endpoint.provider, def.model_endpoint.model
        ),
        context: TestContext {
            tool_count,
            tools,
            skill_count,
            skills,
        },
        provider_configured,
        ok: true,
    })
}
