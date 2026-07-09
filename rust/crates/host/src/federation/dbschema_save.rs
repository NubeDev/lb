//! `dbschema.save {name, schema}` (member) — upsert a designed-schema record (schema-designer
//! scope). Member-gated (`mcp:dbschema.save:call`, workspace-first): designing is harmless, so any
//! author may save; APPLYING the design is the admin-only step (`federation.migrate`, open-question
//! lean #1). Validates the record's structure before persist so a bad name/type never lands.
//!
//! The record is the product (the canvas is one editor); `dbschema.save` is callable by the agent
//! just as well as by the UI (MCP is the universal contract, rule 7).

use lb_auth::Principal;
use serde_json::Value;

use super::authorize::authorize;
use super::dbschema_record::{put, validate_record, DbSchemaRecord};
use super::error::FederationError;
use crate::boot::Node;

/// Upsert the `db_schema:{ws}:{name}` record from the `schema` JSON (the `{tables, fks, layout}`
/// shape). `name` is the workspace-unique schema id; `ts` is the caller's logical timestamp.
/// Validates structure first — a bad identifier/type is a clean `BadInput`, never a panic.
pub async fn dbschema_save(
    node: &Node,
    caller: &Principal,
    ws: &str,
    name: &str,
    schema: &Value,
    ts: u64,
) -> Result<(), FederationError> {
    authorize(caller, ws, "dbschema.save")?;

    // Decode the incoming record shape. Unknown fields are ignored (forward-compat); a missing
    // required field is a `BadInput` with the field name so the author fixes the right thing.
    let mut rec: DbSchemaRecord = serde_json::from_value(schema.clone())
        .map_err(|e| FederationError::BadInput(e.to_string()))?;
    // Override the name + tag with the caller's authoritative values (the record's own `name`
    // field is display data; the store key is `name` arg — they must agree, and the arg wins so
    // a caller can't forge a different key).
    if rec.name.is_empty() {
        rec.name = name.to_string();
    }
    if rec.name != name {
        return Err(FederationError::BadInput(format!(
            "record name `{}` does not match the `name` arg `{name}`",
            rec.name
        )));
    }
    rec.version = super::dbschema_record::SCHEMA_VERSION;
    rec.tag = super::dbschema_record::schema_tag();
    rec.removed = false;
    rec.ts = ts;

    validate_record(&rec).map_err(FederationError::BadInput)?;
    put(&node.store, ws, &rec).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn save_decodes_full_shape() {
        let schema = json!({
            "name": "shop",
            "version": 1,
            "tables": [{
                "name": "users",
                "columns": [{"name": "id", "type": "integer", "nullable": false}],
                "pk": ["id"]
            }],
            "fks": [],
            "layout": {"users": {"x": 100.0, "y": 50.0}},
            "ts": 1
        });
        let rec: DbSchemaRecord = serde_json::from_value(schema).unwrap();
        assert_eq!(rec.tables.len(), 1);
        assert_eq!(rec.tables[0].pk, vec!["id".to_string()]);
        assert_eq!(rec.layout.get("users").unwrap().x, 100.0);
    }
}
