//! `query.compile {lang, text, target}` → `{sql}` (query scope). The pure dry-run: compile PRQL to
//! the target's dialect (or carry `raw` verbatim) WITHOUT executing — feeds the editor's live
//! preview/validate. Costs no data access and needs NO target cap (it never reaches an engine), only
//! its own `mcp:query.compile:call`. A compile failure is author feedback (`BadInput`), never a deny.

use lb_auth::Principal;
use serde_json::json;
use serde_json::Value;

use super::authorize::authorize;
use super::error::QueryError;
use super::materialize::materialize;
use super::save::validate_lang;
use super::target::QueryTarget;
use crate::boot::Node;

/// Compile `text` (in `lang`) for `target` to SQL, without executing. Returns `{sql}`.
pub async fn query_compile(
    node: &Node,
    caller: &Principal,
    ws: &str,
    lang: &str,
    text: &str,
    target: &str,
) -> Result<Value, QueryError> {
    authorize(caller, ws, "query.compile")?;
    validate_lang(lang)?;
    QueryTarget::parse(target)?;
    let sql = materialize(node, ws, lang, text, target).await?;
    Ok(json!({ "sql": sql }))
}
