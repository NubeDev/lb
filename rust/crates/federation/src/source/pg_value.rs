//! A JSON cell bound as a Postgres `$n` parameter — the value half of `federation.write` /
//! `federation.delete`. Split out of `postgres.rs`, which owns the pool and the `Source` impl: how a
//! value is encoded for the wire is its own responsibility (FILE-LAYOUT §9).

/// An owned cell value bound as a Postgres `$n` parameter (parameterized — never inlined into SQL).
/// Postgres is strictly typed, so we bind each JSON scalar as its natural Postgres type and let the
/// server coerce into the target column (an `i64` into `integer`, `text` into `text`, `f64` into
/// `numeric`/`double`). Structured JSON (arrays/objects) is sent as a `jsonb` value so a `json`/
/// `jsonb` column reads it back structurally.
#[derive(Debug)]
pub(super) enum PgValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Json(serde_json::Value),
}

impl PgValue {
    pub(super) fn from_json(v: &serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => PgValue::Null,
            serde_json::Value::Bool(b) => PgValue::Bool(*b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    PgValue::Int(i)
                } else if let Some(f) = n.as_f64() {
                    PgValue::Float(f)
                } else {
                    PgValue::Text(n.to_string())
                }
            }
            serde_json::Value::String(s) => PgValue::Text(s.clone()),
            other => PgValue::Json(other.clone()),
        }
    }
}

impl tokio_postgres::types::ToSql for PgValue {
    fn to_sql(
        &self,
        ty: &tokio_postgres::types::Type,
        out: &mut tokio_postgres::types::private::BytesMut,
    ) -> Result<tokio_postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        match self {
            PgValue::Null => Ok(tokio_postgres::types::IsNull::Yes),
            PgValue::Bool(b) => b.to_sql(ty, out),
            PgValue::Int(i) => i.to_sql(ty, out),
            PgValue::Float(f) => f.to_sql(ty, out),
            PgValue::Text(s) => s.to_sql(ty, out),
            PgValue::Json(j) => j.to_sql(ty, out),
        }
    }

    fn accepts(_ty: &tokio_postgres::types::Type) -> bool {
        // Accept whatever the target column declares — the server drives coercion. Per-arm encoders
        // above still refuse a genuine mismatch at `to_sql` time.
        true
    }

    tokio_postgres::types::to_sql_checked!();
}
