//! Shared write-time validation for the custom-persona verbs (persona-model scope). Applied in this
//! order on every create/update:
//!   1. **Reserved tier (before the caps gate).** A `builtin.*` id is read-only to users regardless of
//!      caps — rejected `BadInput` (the agent-definition / core-skills `Reserved → BadInput` rule).
//!      Checked first so no broad grant can reach past it.
//!   2. **Glob grammar.** Every `granted_tools` entry is a raw tool id or a **trailing-`*`** glob
//!      (`flows.*`). A bare `*` (an everything-persona) is rejected — "no persona" should be *unset*,
//!      not a wildcard that silently means "widen to the whole menu" (the scope's decided semantics).
//!      A `*` anywhere but the last char is rejected — the glob is prefix-on-the-tool-id, nothing
//!      smarter (no cap-grammar interplay; one meaning, property-tested in the model tests).
//!   3. **`extends` shape.** Self-reference and duplicate parents are rejected; the caller
//!      additionally cycle-checks and depth-caps the closure against the live store (see
//!      [`validate_extends`]). Resolution is at *read* time (parents evolve → children follow), so the
//!      only write-time job is to reject a chain that would become a boot/resolve hazard.
//!
//! Cycle/depth are checked against the persona's *own* namespace on a custom write; a custom persona
//! may extend a built-in (resolved from the reserved namespace at read time) but the cycle walk only
//! needs the custom graph plus the (acyclic, seed-authored) built-in parents.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;

use super::model::{is_builtin, Persona};
use super::store::{get_persona, PERSONA_NS};

/// The maximum `extends` depth resolved at read time. A chain deeper than this is rejected at write —
/// keeps resolution bounded and a mistaken deep chain from becoming a boot hazard (scope Risk).
pub const MAX_EXTENDS_DEPTH: usize = 3;

/// Reject a reserved `builtin.*` id on a write — read-only tier, checked BEFORE the caps gate. A
/// non-builtin id passes.
pub fn reject_reserved(id: &str) -> Result<(), ToolError> {
    if is_builtin(id) {
        return Err(ToolError::BadInput(format!(
            "{id:?} is a reserved built-in persona (read-only to users)"
        )));
    }
    Ok(())
}

/// Validate one `granted_tools` glob/id: trailing-`*` only, never a bare `*`, never a `*` mid-string.
/// A plain id (no `*`) always passes — it is opaque data (rule 10), matched literally at apply time.
pub fn validate_glob(entry: &str) -> Result<(), ToolError> {
    if entry.is_empty() {
        return Err(ToolError::BadInput("granted_tools entry is empty".into()));
    }
    if entry == "*" {
        return Err(ToolError::BadInput(
            "granted_tools may not be a bare '*' — an everything-persona is no persona; leave granted_tools unset for no narrowing".into(),
        ));
    }
    match entry.find('*') {
        // No glob — a literal id.
        None => Ok(()),
        // A `*` must be the single trailing char (prefix match on the tool id).
        Some(pos) if pos == entry.len() - 1 => Ok(()),
        Some(_) => Err(ToolError::BadInput(format!(
            "granted_tools glob {entry:?} must end with a single trailing '*' (prefix match); '*' is not allowed mid-string"
        ))),
    }
}

/// Validate a persona's fields on a custom write (glob grammar + `extends` self/dup shape). The caps
/// gate and reserved-tier check are the verb's job; the cross-store cycle/depth walk is
/// [`validate_extends`] (needs the store).
pub fn validate_fields(persona: &Persona) -> Result<(), ToolError> {
    for entry in &persona.granted_tools {
        validate_glob(entry)?;
    }
    for parent in &persona.extends {
        if parent == &persona.id {
            return Err(ToolError::BadInput(format!(
                "persona {:?} cannot extend itself",
                persona.id
            )));
        }
    }
    // Reject duplicate parents (a set, not a bag — a dup is almost certainly an authoring mistake and
    // would double-count in the union at resolve).
    let mut seen = std::collections::HashSet::new();
    for parent in &persona.extends {
        if !seen.insert(parent.as_str()) {
            return Err(ToolError::BadInput(format!(
                "persona {:?} lists parent {parent:?} more than once",
                persona.id
            )));
        }
    }
    Ok(())
}

/// Walk the `extends` closure of `persona` against the live store, rejecting a cycle or a chain deeper
/// than [`MAX_EXTENDS_DEPTH`]. Reads each parent under `caller` from the workspace namespace, then the
/// reserved built-in namespace (a custom persona may extend a built-in). An unresolvable parent is NOT
/// a write error here (resolve-at-read tolerates a dangling parent, like `active_definition`) — only a
/// cycle or over-deep chain is rejected, because those are the boot/resolve hazards.
pub async fn validate_extends(
    store: &Store,
    _caller: &Principal,
    ws: &str,
    persona: &Persona,
) -> Result<(), ToolError> {
    // DFS from each declared parent; the starting persona is on the stack so a chain back to it is a
    // cycle. Depth is counted in edges from the starting persona.
    let mut stack: Vec<(String, usize)> = persona.extends.iter().map(|p| (p.clone(), 1)).collect();
    let mut on_path: std::collections::HashSet<String> = std::collections::HashSet::new();
    on_path.insert(persona.id.clone());

    while let Some((id, depth)) = stack.pop() {
        if id == persona.id {
            return Err(ToolError::BadInput(format!(
                "persona {:?} has a cyclic extends chain (reaches itself)",
                persona.id
            )));
        }
        if depth > MAX_EXTENDS_DEPTH {
            return Err(ToolError::BadInput(format!(
                "persona {:?} extends chain deeper than {MAX_EXTENDS_DEPTH}",
                persona.id
            )));
        }
        // Resolve the parent: custom (ws) first, then built-in (reserved ns). A dangling parent simply
        // has no further edges to walk (tolerated at write; resolve-at-read warns).
        let parent = if is_builtin(&id) {
            get_persona(store, PERSONA_NS, &id, true)
                .await
                .map_err(|_| ToolError::Denied)?
        } else {
            get_persona(store, ws, &id, false)
                .await
                .map_err(|_| ToolError::Denied)?
        };
        if let Some(parent) = parent {
            for gp in parent.extends {
                if on_path.contains(&gp) && gp != persona.id {
                    // A cycle among ancestors that doesn't pass through the new persona still makes
                    // resolution non-terminating — reject it too.
                    return Err(ToolError::BadInput(format!(
                        "persona {:?} extends chain contains a cycle at {gp:?}",
                        persona.id
                    )));
                }
                stack.push((gp, depth + 1));
            }
        }
    }
    Ok(())
}
