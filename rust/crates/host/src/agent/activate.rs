//! `skill.activate` — the model-activated skill, a **loop-internal** tool (agent-run scope Part 5).
//!
//! DESIGN: why this is intercepted inside the loop rather than dispatched out through `lb_mcp::call`.
//! `skill.activate` must mutate the RUN: append `SkillActivated` to the durable transcript (so the
//! activation survives resume — Part 0) and inject the skill body into the model's context for the
//! following turns. But `lb_mcp::call` dispatches a tool *generically*, with no `job_id` and no
//! handle on the run's transcript/cursor/message list — those live only in the loop (`run.rs`).
//! Routing `skill.activate` out to an extension would therefore have nowhere to record the
//! activation or grow the context. So the loop treats it as a **built-in**: when the model proposes
//! a call to `SKILL_ACTIVATE`, the loop calls [`activate_skill`] here (which enforces the S4 grant
//! via `load_skill`), records the transcript event + injects the body itself, and feeds back an ok
//! result — exactly the shape the scope prescribes ("treat skill.activate specially INSIDE the
//! loop … rather than routing it out").
//!
//! The grant wall is intact: [`activate_skill`] loads under the DERIVED principal through
//! `load_skill`, whose gate 3 is the workspace grant — an ungranted skill is `Denied` (opaque),
//! fed back to the model as an error result, never activated and never recorded. This is the same
//! deny `load_skill` enforces everywhere; the catalog only ever lists granted skills, so a denied
//! activation means the model named a skill outside its catalog (or one revoked mid-run).
//!
//! Activation is idempotent: re-activating the same id reloads the body (cheap, grant re-checked)
//! and `rehydrate` already de-dups `active_skills`, so a duplicate `SkillActivated` is harmless.

use lb_auth::Principal;
use lb_store::Store;

use super::model_access::CallOutcome;
use crate::assets::{load_skill, AssetError};

/// The qualified name of the loop-internal skill-activation tool. The loop matches proposed calls
/// against this and intercepts them; it is NOT a registry/extension tool.
pub const SKILL_ACTIVATE: &str = "skill.activate";

/// The outcome of intercepting one `skill.activate` call: the tool result to feed the model, and —
/// on success — the activated skill `id` + its `body` (so the loop can record `SkillActivated` and
/// inject the body into context). On denial/bad-input, `activated` is `None` and `outcome` carries
/// the error the model sees (the loop records the failed `ToolResult` but no activation).
pub struct Activation {
    pub outcome: CallOutcome,
    /// `Some((id, body))` only when the grant check passed and the skill loaded.
    pub activated: Option<(String, String)>,
}

/// Handle one proposed `skill.activate` call under the derived `agent` principal. `id` is the
/// skill id from the call args (`{"id": "<skill>"}`). Grant-gated through `load_skill` — an
/// ungranted skill is `Denied`, surfaced as an error result, not an activation.
pub async fn activate_skill(
    store: &Store,
    agent: &Principal,
    ws: &str,
    call_id: &str,
    args: &str,
) -> Activation {
    let id = serde_json::from_str::<serde_json::Value>(args)
        .ok()
        .and_then(|v| v.get("id").and_then(|i| i.as_str()).map(str::to_string));
    let id = match id {
        Some(id) => id,
        None => {
            return Activation {
                outcome: CallOutcome {
                    id: call_id.to_string(),
                    ok: None,
                    error: Some("skill.activate: missing string arg 'id'".to_string()),
                },
                activated: None,
            }
        }
    };

    // Grant check lives here: load_skill's gate 3 is the workspace grant. Denied → error result.
    match load_skill(store, agent, ws, &id, None).await {
        Ok(skill) => Activation {
            outcome: CallOutcome {
                id: call_id.to_string(),
                ok: Some(format!("activated skill {id}")),
                error: None,
            },
            activated: Some((id, skill.body)),
        },
        Err(e) => Activation {
            outcome: CallOutcome {
                id: call_id.to_string(),
                ok: None,
                error: Some(match e {
                    AssetError::Denied => format!("skill.activate denied: {id} is not granted"),
                    AssetError::NotFound => format!("skill.activate: {id} not found"),
                    AssetError::Store(s) => format!("skill.activate failed: {s}"),
                }),
            },
            activated: None,
        },
    }
}
