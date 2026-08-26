//! The **importer's principal** — a deliberately narrow machine identity for the poll.
//!
//! The mail-source scope asked for "a narrow machine principal: each source's poll job runs as an
//! api-key principal granted exactly {media put, doc put, extract call} in one workspace. Deny path:
//! the poller can never read the corpus back." This is that, minted per pass rather than stored as
//! an api key — the poller is the node acting on its own durable configuration, the same shape the
//! flow and retention reactors already use (`flows::reactor_caps`).
//!
//! ### Why this is not `reactor_caps()`
//!
//! It would have been one line to reuse the flow reactor's bundle, and it would have been wrong: it
//! carries `store:*:read`, `store:*:write`, `mcp:store.query:call` and more — i.e. an inbound mail
//! path that could read every record in the workspace. The bundle below is the *entire* set of
//! things importing a message needs, and the property worth testing is what is missing from it:
//! **the importer cannot read a doc, list a source, or query the store.** A mailbox is an untrusted
//! ingress (anyone who can email the address reaches it), so the blast radius of a bug in the
//! import path is bounded by this list and nothing else.
//!
//! Per lb#167's lesson, this function is `pub` so tests mint the **production** principal instead of
//! a hand-copied list — a mirrored list stops testing this the moment the two drift.

use lb_auth::Principal;

/// The importer's identity. Not a user; the node acting on a mail source an admin registered.
pub const MAIL_IMPORT_SUB: &str = "node:mail";

/// Exactly what importing one message needs, and nothing else.
pub fn mail_import_caps() -> Vec<String> {
    vec![
        // Store the raw message and each attachment as a workspace asset. The verb cap and the
        // store-surface cap are BOTH required — `put_asset` re-checks `store:asset/{id}:write`
        // inside, and the single-segment `store:*:write` wildcard does not span a `asset/{id}`
        // resource path (the trap `media.read` fell into; see builtin_roles' note).
        "mcp:assets.put_asset:call".into(),
        "store:asset/*:write".into(),
        // Project the arrival onto the inbox.
        "mcp:inbox.record:call".into(),
        // Turn a decoded attachment into series samples.
        "mcp:ingest.write:call".into(),
    ]
}

/// Mint the importer for `ws`. The workspace is carried on the principal — the hard wall — so an
/// importer minted for ws A cannot touch ws B even if a source record said otherwise.
pub fn mail_import_principal(ws: &str) -> Principal {
    Principal::routed(MAIL_IMPORT_SUB, ws.to_string(), mail_import_caps())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_caps::{check, Action, Decision, Request, Surface};

    #[test]
    fn the_importer_cannot_read_the_corpus_it_writes_into() {
        let importer = mail_import_principal("acme");
        for (surface, resource, action) in [
            (Surface::Store, "doc/anything", Action::Read),
            (Surface::Store, "asset/anything", Action::Read),
            (Surface::Mcp, "store.query", Action::Call),
            (Surface::Mcp, "mail.source.list", Action::Call),
            (Surface::Mcp, "inbox.list", Action::Call),
        ] {
            let req = Request::new("acme", surface, resource, action);
            assert!(
                matches!(check(&importer, &req), Decision::Denied(_)),
                "the mail importer must not reach {resource}"
            );
        }
    }

    #[test]
    fn the_importer_holds_exactly_the_four_things_an_import_needs() {
        let importer = mail_import_principal("acme");
        for (surface, resource, action) in [
            (Surface::Store, "asset/mail-raw-x", Action::Write),
            (Surface::Mcp, "assets.put_asset", Action::Call),
            (Surface::Mcp, "inbox.record", Action::Call),
            (Surface::Mcp, "ingest.write", Action::Call),
        ] {
            let req = Request::new("acme", surface, resource, action);
            assert!(
                matches!(check(&importer, &req), Decision::Allowed),
                "the mail importer needs {resource}"
            );
        }
    }

    #[test]
    fn an_importer_minted_for_one_workspace_reaches_no_other() {
        let importer = mail_import_principal("acme");
        let req = Request::new("other", Surface::Mcp, "ingest.write", Action::Call);
        assert!(matches!(check(&importer, &req), Decision::Denied(_)));
    }
}
