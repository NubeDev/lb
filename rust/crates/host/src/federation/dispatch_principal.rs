//! The principal a federation READ dispatches the sidecar under.
//!
//! **The bug this closes.** `federation.query` is a VIEWER-tier cap — `builtin_roles::VIEWER_CAPS`
//! names it, commented "a viewer's tiles read series/federation", because every dashboard tile backed
//! by a registered source runs through it. But the verb performs its work by calling the supervised
//! federation sidecar, and `native::call_sidecar` gates on the CALLER holding `mcp:native.call:call`
//! — which lives in `AUTHOR_CAPS`. So the viewer tier contradicted itself: a viewer could ASK for a
//! federated query and was refused the DISPATCH that answers it. Every viewer, in every workspace,
//! saw empty panels on any dashboard backed by a datasource, with an opaque `denied` and nothing in
//! the UI to explain it. Found 2026-08-03 against a live node.
//!
//! **Why compose rather than move the cap.** Moving `native.call` into `VIEWER_CAPS` would fix the
//! symptom by handing every viewer the generic authority to call ANY native extension's tools — far
//! more than "render the tile you were given". Instead the read path dispatches under the caller's
//! own identity carrying exactly the one extra authority the dispatch needs, and only AFTER the
//! caller has passed the `federation.query` gate. This is the pattern `react_to_profiles`'
//! `reactor_principal` already established for the background pass ("the read privilege the pass
//! needs… and the authority to reach the supervised sidecar that performs it").
//!
//! **No widening.** The composition happens strictly after `authorize(caller, ws,
//! "federation.query")` has succeeded, so it can only ever be reached by a caller already authorized
//! for this verb. It adds ONE cap and nothing else, and the resulting principal is used only to
//! reach the sidecar for this call — it is never stored, never minted into a token, and never
//! returned. A caller who cannot pass the federation gate never gets here.
//!
//! **Identity is preserved.** `call_sidecar` projects the principal it is handed into the call frame
//! (native-caller-identity scope) so the child can enforce per-caller row visibility. The composed
//! principal keeps the caller's own `sub` and `ws` — only the cap set differs — so the frame the
//! child sees is unchanged and per-caller visibility still works.

use lb_auth::Principal;

/// The cap `native::call_sidecar` demands of whoever dispatches to a supervised child.
const NATIVE_CALL: &str = "mcp:native.call:call";

/// `caller`, plus the authority to reach the supervised sidecar that performs the read.
///
/// Returns the caller unchanged when they already hold `native.call` (an author or admin), so the
/// common authoring path keeps the exact principal it had before this existed.
///
/// MUST be called only after the verb's own `authorize` has passed — this function does not gate
/// anything, it composes the dispatch authority for a caller that is already through the wall.
pub fn sidecar_dispatch_principal(caller: &Principal) -> Principal {
    if caller.caps().iter().any(|c| c == NATIVE_CALL) {
        return caller.clone();
    }
    let mut caps: Vec<String> = caller.caps().to_vec();
    caps.push(NATIVE_CALL.to_string());
    Principal::routed(caller.sub().to_string(), caller.ws().to_string(), caps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(caps: &[&str]) -> Principal {
        Principal::routed(
            "user:vera".to_string(),
            "acme".to_string(),
            caps.iter().map(|c| c.to_string()).collect(),
        )
    }

    /// The regression itself: a VIEWER holding `federation.query` but not `native.call` must come out
    /// of this able to reach the sidecar — otherwise their dashboard tiles render empty.
    #[test]
    fn viewer_gains_exactly_the_dispatch_cap() {
        let viewer = principal(&["mcp:federation.query:call"]);
        let dispatch = sidecar_dispatch_principal(&viewer);

        assert!(dispatch.caps().iter().any(|c| c == NATIVE_CALL));
        // Exactly ONE cap added — this must never become a general-purpose widening seam.
        assert_eq!(dispatch.caps().len(), viewer.caps().len() + 1);
        assert!(dispatch
            .caps()
            .iter()
            .any(|c| c == "mcp:federation.query:call"));
    }

    /// Identity is what the child reads out of the call frame to enforce per-caller row visibility
    /// (native-caller-identity scope). Composing authority must not change WHO is calling.
    #[test]
    fn identity_and_workspace_are_preserved() {
        let viewer = principal(&["mcp:federation.query:call"]);
        let dispatch = sidecar_dispatch_principal(&viewer);

        assert_eq!(dispatch.sub(), viewer.sub());
        assert_eq!(dispatch.ws(), viewer.ws());
    }

    /// An author already holds the cap, so the principal must pass through untouched — no duplicate
    /// entry, no reallocation of the authoring path's identity.
    #[test]
    fn author_passes_through_unchanged() {
        let author = principal(&["mcp:federation.query:call", NATIVE_CALL]);
        let dispatch = sidecar_dispatch_principal(&author);

        assert_eq!(dispatch.caps(), author.caps());
        assert_eq!(dispatch.sub(), author.sub());
    }

    /// The composition adds the sidecar reach and NOTHING else — it must never become a way for a
    /// viewer to pick up an authoring verb on the way to a read.
    #[test]
    fn no_author_cap_leaks_in() {
        let viewer = principal(&["mcp:federation.query:call"]);
        let dispatch = sidecar_dispatch_principal(&viewer);

        for author_cap in [
            "mcp:federation.write:call",
            "mcp:datasource.add:call",
            "mcp:rules.save:call",
            "mcp:store.query:call",
        ] {
            assert!(
                !dispatch.caps().iter().any(|c| c == author_cap),
                "dispatch principal must not carry {author_cap}"
            );
        }
    }
}
