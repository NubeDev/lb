//! The forms surface, headless (forms scope). Proves the mandatory categories against a real store:
//! the CRUD round-trip (with `def` byte-identical), capability-deny **per verb**, two-workspace
//! isolation, UPSERT semantics (owner preserved on re-save), the admin `delete_any` override, and the
//! `call_forms_tool` MCP-boundary round-trip (camelCase wire JSON). A form is a simple owner/workspace
//! asset — no gate-3 visibility — so the sharing/gate-3 tests the dashboard suite carries are absent
//! by design.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_forms_tool, forms_delete, forms_get, forms_list, forms_save, Form, FormError};
use lb_store::Store;
use serde_json::json;

/// A principal `sub` in workspace `ws` holding `caps`.
fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const GET: &str = "mcp:forms.get:call";
const LIST: &str = "mcp:forms.list:call";
const SAVE: &str = "mcp:forms.save:call";
const DELETE: &str = "mcp:forms.delete:call";
const ALL: &[&str] = &[GET, LIST, SAVE, DELETE];

/// A representative `def` — the `options.form` shape (schema/ui/submit/mode/recordSource/
/// optionsSources/success). The host treats it as opaque; the test asserts it survives byte-identical.
fn sample_def() -> serde_json::Value {
    json!({
        "schema": { "type": "object", "properties": { "name": { "type": "string" } } },
        "ui": { "name": { "widget": "text" } },
        "submit": { "tool": "store.write", "argsTemplate": { "table": "intake" } },
        "mode": "create",
        "recordSource": { "tool": "store.query", "args": { "sql": "SELECT * FROM intake" } },
        "optionsSources": { "country": { "tool": "store.query", "args": {} } },
        "success": { "message": "Saved" }
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn crud_round_trip_def_survives() {
    let ws = "ws-form-crud";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);
    let def = sample_def();

    // create
    let f = forms_save(&store, &test, ws, "intake", "Intake", def.clone(), 10)
        .await
        .unwrap();
    assert_eq!(f.title, "Intake");
    assert_eq!(f.owner, "user:test");
    assert_eq!(f.schema_version, 1);
    assert_eq!(f.def, def, "def stored byte-identical on save");

    // get reflects it, def survives byte-identical
    let got = forms_get(&store, &test, ws, "intake").await.unwrap();
    assert_eq!(got.def, def, "def survives save→get byte-identical");
    assert_eq!(got.updated_ts, 10);

    // update (same id) — title + def change, owner preserved
    let def2 = json!({ "schema": { "type": "object" }, "mode": "edit" });
    forms_save(&store, &test, ws, "intake", "Intake v2", def2.clone(), 20)
        .await
        .unwrap();
    let got = forms_get(&store, &test, ws, "intake").await.unwrap();
    assert_eq!(got.title, "Intake v2");
    assert_eq!(got.def, def2);
    assert_eq!(got.owner, "user:test", "owner preserved across update");
    assert_eq!(got.updated_ts, 20);

    // list includes it (summary, no def body)
    let roster = forms_list(&store, &test, ws).await.unwrap();
    assert!(roster
        .iter()
        .any(|s| s.id == "intake" && s.title == "Intake v2"));

    // delete → list excludes it; get is NotFound
    forms_delete(&store, &test, ws, "intake", 30).await.unwrap();
    let roster = forms_list(&store, &test, ws).await.unwrap();
    assert!(!roster.iter().any(|s| s.id == "intake"));
    assert!(matches!(
        forms_get(&store, &test, ws, "intake").await.unwrap_err(),
        FormError::NotFound
    ));

    // re-delete is an idempotent no-op
    forms_delete(&store, &test, ws, "intake", 40).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_verb_is_denied_without_its_cap() {
    let ws = "ws-form-deny";
    let store = Store::memory().await.unwrap();
    // Test owns a form (so get has a target); the denied principal holds NO forms cap.
    let test = principal("user:test", ws, ALL);
    forms_save(&store, &test, ws, "intake", "Intake", sample_def(), 1)
        .await
        .unwrap();

    let nobody = principal("user:nobody", ws, &[]);
    assert!(matches!(
        forms_get(&store, &nobody, ws, "intake").await.unwrap_err(),
        FormError::Denied
    ));
    assert!(matches!(
        forms_list(&store, &nobody, ws).await.unwrap_err(),
        FormError::Denied
    ));
    assert!(matches!(
        forms_save(&store, &nobody, ws, "x", "X", sample_def(), 1)
            .await
            .unwrap_err(),
        FormError::Denied
    ));
    assert!(matches!(
        forms_delete(&store, &nobody, ws, "intake", 1)
            .await
            .unwrap_err(),
        FormError::Denied
    ));
}

/// A non-owner holding the SAVE cap still cannot overwrite someone else's form (owner-only update).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn non_owner_cannot_overwrite() {
    let ws = "ws-form-owner";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);
    forms_save(&store, &test, ws, "intake", "Intake", sample_def(), 1)
        .await
        .unwrap();

    let mallory = principal("user:mallory", ws, ALL);
    assert!(matches!(
        forms_save(&store, &mallory, ws, "intake", "hijack", json!({}), 2)
            .await
            .unwrap_err(),
        FormError::Denied
    ));
    // Untouched — the denied save did not change the owner's form.
    let got = forms_get(&store, &test, ws, "intake").await.unwrap();
    assert_eq!(got.title, "Intake");
    assert_eq!(got.owner, "user:test");
}

/// The `delete_any` admin override: a non-owner with only the base DELETE cap stays denied; granting
/// `forms.delete_any` too lets them tombstone someone else's form. Mirrors `dashboard.delete`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_any_cap_lets_a_non_owner_admin_delete() {
    let ws = "ws-form-delete-any";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);
    forms_save(&store, &test, ws, "intake", "Intake", sample_def(), 1)
        .await
        .unwrap();

    let admin_without = principal("user:admin", ws, &[GET, LIST, DELETE]);
    assert!(matches!(
        forms_delete(&store, &admin_without, ws, "intake", 2)
            .await
            .unwrap_err(),
        FormError::Denied
    ));
    assert!(forms_list(&store, &test, ws)
        .await
        .unwrap()
        .iter()
        .any(|s| s.id == "intake"));

    let admin_with = principal(
        "user:admin",
        ws,
        &[GET, LIST, DELETE, "mcp:forms.delete_any:call"],
    );
    forms_delete(&store, &admin_with, ws, "intake", 3)
        .await
        .unwrap();
    assert!(!forms_list(&store, &test, ws)
        .await
        .unwrap()
        .iter()
        .any(|s| s.id == "intake"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation() {
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", "ws-a", ALL);
    let ben = principal("user:ben", "ws-b", ALL);

    forms_save(&store, &test, "ws-a", "intake", "Intake A", sample_def(), 1)
        .await
        .unwrap();

    // Ben (ws-B) cannot get ws-A's form (a different namespace → not found) and his roster is empty.
    assert!(matches!(
        forms_get(&store, &ben, "ws-b", "intake").await.unwrap_err(),
        FormError::NotFound
    ));
    assert!(forms_list(&store, &ben, "ws-b").await.unwrap().is_empty());
}

/// The MCP boundary: `call_forms_tool` round-trips the wire JSON — camelCase `def` passes through
/// untouched and `updated_ts` is present on the returned record (the shape the UI/agents consume).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn call_forms_tool_wire_round_trip() {
    let ws = "ws-form-wire";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);
    let def = sample_def();

    // save via the MCP bridge.
    let saved = call_forms_tool(
        &store,
        &test,
        ws,
        "forms.save",
        &json!({ "id": "intake", "title": "Intake", "def": def, "now": 100 }),
    )
    .await
    .unwrap();
    // The returned record carries the camelCase wire shape.
    assert_eq!(saved["def"], def, "def passes through byte-identical");
    assert_eq!(
        saved["updated_ts"],
        json!(100),
        "updated_ts present on wire"
    );
    assert_eq!(saved["schemaVersion"], json!(1), "schemaVersion camelCase");
    assert_eq!(saved["owner"], json!("user:test"));

    // get via the bridge returns the same def.
    let got = call_forms_tool(&store, &test, ws, "forms.get", &json!({ "id": "intake" }))
        .await
        .unwrap();
    assert_eq!(got["def"], def);

    // list via the bridge returns the summary under `forms`.
    let listed = call_forms_tool(&store, &test, ws, "forms.list", &json!({}))
        .await
        .unwrap();
    let rows = listed["forms"].as_array().expect("forms is an array");
    assert!(rows
        .iter()
        .any(|r| r["id"] == json!("intake") && r["title"] == json!("Intake")));
    // The summary is cheap — no def body on a roster row.
    assert!(rows.iter().all(|r| r.get("def").is_none()));

    // A JSON-ENCODED-STRING `def` (the AI-caller shape) decodes to the same value at the boundary.
    let saved2 = call_forms_tool(
        &store,
        &test,
        ws,
        "forms.save",
        &json!({ "id": "intake2", "title": "Intake2", "def": def.to_string(), "now": 200 }),
    )
    .await
    .unwrap();
    assert_eq!(
        saved2["def"], def,
        "stringified def decodes to the same object"
    );

    // delete via the bridge is idempotent and returns ok.
    let del = call_forms_tool(
        &store,
        &test,
        ws,
        "forms.delete",
        &json!({ "id": "intake", "now": 300 }),
    )
    .await
    .unwrap();
    assert_eq!(del, json!({ "ok": true }));
}

/// A round-trip preserving the exact `Form` model (not just wire JSON) — the persisted record's
/// `deleted` flag flips on delete and the tombstone is what hides it from get/list.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn tombstone_hides_from_get_and_list() {
    let ws = "ws-form-tomb";
    let store = Store::memory().await.unwrap();
    let test = principal("user:test", ws, ALL);

    let saved: Form = forms_save(&store, &test, ws, "intake", "Intake", sample_def(), 1)
        .await
        .unwrap();
    assert!(!saved.deleted);

    forms_delete(&store, &test, ws, "intake", 2).await.unwrap();
    assert!(matches!(
        forms_get(&store, &test, ws, "intake").await.unwrap_err(),
        FormError::NotFound
    ));
    assert!(forms_list(&store, &test, ws).await.unwrap().is_empty());

    // A save with the same id after delete resurrects it (create — the tombstone is treated as absent).
    let resurrected = forms_save(&store, &test, ws, "intake", "Intake again", json!({}), 3)
        .await
        .unwrap();
    assert!(!resurrected.deleted);
    assert_eq!(
        forms_get(&store, &test, ws, "intake").await.unwrap().title,
        "Intake again"
    );
}
