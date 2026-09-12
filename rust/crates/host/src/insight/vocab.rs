//! `insight_vocab_list` / `insight_vocab_set` — read and author the workspace's tag vocabularies
//! over the capability gate (case-plane scope §"Vocabulary").
//!
//! **Both halves are ADMIN**, and the read deliberately so — it sits beside `policy.sla.list` for
//! the same reason. A vocabulary is a workspace-level declaration that reorders everything built on
//! it: adding a category changes what may be raised, and marking one as GATING arms the caveat
//! mechanism across the whole plane. That is a settings-surface read, not a queue read. Nothing a
//! member needs is behind it — `insight.list` already echoes each finding's own category, and the
//! case row carries it too, so a viewer sees the values in use without being handed the declaration.
//!
//! **A cap per verb, and NO gate alias.** Both `mcp:insight.vocab.list:call` and
//! `mcp:insight.vocab.set:call` exist in the admin bundle, so the dispatcher's default convention
//! derives the right cap for each and `tool_gate.rs` needs no arm — which is how `policy.sla.*` and
//! `party.*` are already wired. The alias table is for verbs whose cap is deliberately someone
//! else's (`case.members → case.get`); reaching for it here would have been cleverness buying a
//! shipped-but-unusable risk and an entry in a file already at its size limit.
//!
//! **There is no delete.** Declaring an empty `values` list IS the delete: lb's rule is that an
//! undeclared vocabulary validates nothing, so "no row" and "a row declaring nothing" are the same
//! statement. A second verb that could disagree with that one would be a second answer to one
//! question.

use lb_auth::Principal;
use lb_insights::TagVocab;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Every vocabulary this workspace declares, ordered by key. Gated by `mcp:insight.vocab.list:call`.
pub async fn insight_vocab_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<TagVocab>, InsightSvcError> {
    authorize_tool(principal, ws, "insight.vocab.list").map_err(|_| InsightSvcError::Denied)?;
    Ok(lb_insights::list_vocab(store, ws).await?)
}

/// Declare `vocab` for its own key, replacing whatever was there. Validated BEFORE the write, so a
/// rejected declaration leaves the live one exactly as it was.
pub async fn insight_vocab_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    vocab: &TagVocab,
) -> Result<(), InsightSvcError> {
    authorize_tool(principal, ws, "insight.vocab.set").map_err(|_| InsightSvcError::Denied)?;
    lb_insights::write_vocab(store, ws, vocab).await?;
    Ok(())
}
