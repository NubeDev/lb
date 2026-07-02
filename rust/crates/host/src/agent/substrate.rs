//! The agent's substrate — the granted **skill** and shared **doc** it reads before/while running
//! (agent scope: "loads granted skills and reads shared docs as its substrate, never bypassing the
//! gates"). Both go through the SAME S4 host verbs (`load_skill`, `get_doc`), under an
//! **on-behalf-of** principal: the **caller's identity** (so the S4 membership/ownership gate 3
//! resolves as the caller) with the **intersected caps** (so capability gate 2 can never widen).
//!
//! Why the caller's identity and not `agent:session`: gate 3 for docs is owner / shared-team /
//! linked-channel, and gate 3 for skills is the workspace grant. The agent reads what the *caller*
//! may read — it has no privileged back door, but it is also not a stranger to the caller's own
//! docs. Capabilities still bound it to `agent ∩ caller` (it cannot read a doc the agent's own
//! grant excludes, even one the caller owns). That is the precise on-behalf-of contract.

use lb_auth::Principal;
use lb_store::Store;

use super::error::AgentError;
use crate::assets::{get_doc, load_skill, AssetError};

/// Build the on-behalf-of principal for substrate reads: the caller's `sub` (membership resolves as
/// the caller) with `agent_caps ∩ caller.caps` (capabilities can never widen). `derive` already
/// computes the intersection and inherits the caller's ws; we override the sub to the caller's so
/// gate 3 sees the caller — never a privileged `agent:*` actor.
fn on_behalf(caller: &Principal, agent_caps: &[String]) -> Principal {
    caller.derive(caller.sub(), agent_caps.to_vec())
}

/// Load the granted skill `id` (latest version) for the agent, on the caller's behalf. Denied
/// (opaque) if the workspace did not grant it or the intersection lacks the skill read capability.
pub async fn load_substrate_skill(
    store: &Store,
    caller: &Principal,
    agent_caps: &[String],
    ws: &str,
    id: &str,
) -> Result<String, AgentError> {
    let actor = on_behalf(caller, agent_caps);
    let skill = load_skill(store, &actor, ws, id, None)
        .await
        .map_err(asset_to_agent)?;
    Ok(skill.body)
}

/// Read the shared doc `id` for the agent, on the caller's behalf. Denied (opaque) unless the
/// caller is the owner / a shared-team member / a linked-channel grantee — the S4 three-gate read,
/// unchanged, resolved as the caller (with the agent's intersected capability still bounding it).
pub async fn read_substrate_doc(
    store: &Store,
    caller: &Principal,
    agent_caps: &[String],
    ws: &str,
    id: &str,
) -> Result<String, AgentError> {
    let actor = on_behalf(caller, agent_caps);
    let doc = get_doc(store, &actor, ws, id)
        .await
        .map_err(asset_to_agent)?;
    Ok(doc.content)
}

/// Collapse the asset gate's outcome onto the agent error — a `Denied`/`NotFound` stays opaque, so
/// the agent leaks no more than a human would (capability-first, §3.5).
fn asset_to_agent(e: AssetError) -> AgentError {
    match e {
        AssetError::Denied => AgentError::Denied,
        AssetError::NotFound => AgentError::NotFound,
        AssetError::TooLarge => AgentError::Denied,
        AssetError::Reserved => AgentError::Denied,
        AssetError::Store(s) => AgentError::Store(s),
    }
}
