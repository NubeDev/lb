//! The `render_templates` surface, headless (widget-builder scope, "render_templates CRUD"). Proves
//! the mandatory categories against a real store: the CRUD round-trip (`save`→`get`→`list`→`delete`),
//! capability-deny **per verb**, two-workspace isolation, author-ownership (a non-author cannot
//! overwrite/delete another author's template), the size cap, and offline/sync idempotency (the upsert
//! replays). No bus is booted (the verbs are pure store).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    template_delete, template_get, template_list, template_save, Engine, RenderTemplateError,
    TEMPLATE_MAX_BYTES,
};
use lb_store::Store;

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
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const SAVE: &str = "mcp:template.save:call";
const GET: &str = "mcp:template.get:call";
const LIST: &str = "mcp:template.list:call";
const DELETE: &str = "mcp:template.delete:call";
const ALL: &[&str] = &[SAVE, GET, LIST, DELETE];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn crud_round_trip() {
    let ws = "ws-tpl-crud";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    // create
    let t = template_save(
        &store,
        &ada,
        ws,
        "defrost",
        "Defrost Card",
        Engine::Template,
        "<div>{rows[0].value}</div>",
        10,
    )
    .await
    .unwrap();
    assert_eq!(t.id, "defrost");
    assert_eq!(t.author, "user:ada");
    assert_eq!(t.engine, Engine::Template);

    // get reads the code back
    let got = template_get(&store, &ada, ws, "defrost").await.unwrap();
    assert_eq!(got.code, "<div>{rows[0].value}</div>");

    // update (author-only) changes the code, keeps the author
    let upd = template_save(
        &store,
        &ada,
        ws,
        "defrost",
        "Defrost Card v2",
        Engine::Plot,
        "Plot.dot(rows)",
        20,
    )
    .await
    .unwrap();
    assert_eq!(upd.engine, Engine::Plot);
    assert_eq!(upd.author, "user:ada");
    assert_eq!(upd.updated_ts, 20);

    // list shows the summary (no code body in the summary type)
    let rows = template_list(&store, &ada, ws).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "defrost");
    assert_eq!(rows[0].title, "Defrost Card v2");

    // delete tombstones it; get → NotFound, list → empty
    template_delete(&store, &ada, ws, "defrost", 30)
        .await
        .unwrap();
    assert!(matches!(
        template_get(&store, &ada, ws, "defrost").await,
        Err(RenderTemplateError::NotFound)
    ));
    assert!(template_list(&store, &ada, ws).await.unwrap().is_empty());

    // delete is idempotent (already-tombstoned → Ok)
    template_delete(&store, &ada, ws, "defrost", 40)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deny_per_verb() {
    let ws = "ws-tpl-deny";
    let store = Store::memory().await.unwrap();

    // seed one template as a fully-granted author so the read verbs have something to deny against.
    let author = principal("user:ada", ws, ALL);
    template_save(
        &store,
        &author,
        ws,
        "t1",
        "T1",
        Engine::D3,
        "d3.select()",
        1,
    )
    .await
    .unwrap();

    // A principal missing EACH verb is denied for that verb (opaque), holding the others.
    let no_save = principal("user:no-save", ws, &[GET, LIST, DELETE]);
    assert!(matches!(
        template_save(&store, &no_save, ws, "x", "X", Engine::Template, "y", 2).await,
        Err(RenderTemplateError::Denied)
    ));

    let no_get = principal("user:no-get", ws, &[SAVE, LIST, DELETE]);
    assert!(matches!(
        template_get(&store, &no_get, ws, "t1").await,
        Err(RenderTemplateError::Denied)
    ));

    let no_list = principal("user:no-list", ws, &[SAVE, GET, DELETE]);
    assert!(matches!(
        template_list(&store, &no_list, ws).await,
        Err(RenderTemplateError::Denied)
    ));

    let no_delete = principal("user:no-del", ws, &[SAVE, GET, LIST]);
    assert!(matches!(
        template_delete(&store, &no_delete, ws, "t1", 3).await,
        Err(RenderTemplateError::Denied)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation() {
    let store = Store::memory().await.unwrap();
    let ada_a = principal("user:ada", "ws-a", ALL);
    let ben_b = principal("user:ben", "ws-b", ALL);

    // Ada writes in ws-a.
    template_save(
        &store,
        &ada_a,
        "ws-a",
        "shared",
        "A",
        Engine::Template,
        "secret-a",
        1,
    )
    .await
    .unwrap();

    // Ben in ws-b cannot see it (same id, different namespace) — the hard wall.
    assert!(matches!(
        template_get(&store, &ben_b, "ws-b", "shared").await,
        Err(RenderTemplateError::NotFound)
    ));
    assert!(template_list(&store, &ben_b, "ws-b")
        .await
        .unwrap()
        .is_empty());

    // Ben writing his own "shared" in ws-b does not touch ws-a's.
    template_save(
        &store,
        &ben_b,
        "ws-b",
        "shared",
        "B",
        Engine::Template,
        "secret-b",
        1,
    )
    .await
    .unwrap();
    assert_eq!(
        template_get(&store, &ada_a, "ws-a", "shared")
            .await
            .unwrap()
            .code,
        "secret-a"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn author_ownership_blocks_foreign_update_and_delete() {
    let ws = "ws-tpl-owner";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);
    let ben = principal("user:ben", ws, ALL); // fully granted, but NOT the author

    template_save(
        &store,
        &ada,
        ws,
        "t1",
        "Ada's",
        Engine::Template,
        "ada-code",
        1,
    )
    .await
    .unwrap();

    // Ben holds the save/delete caps but is not the author → Denied on both.
    assert!(matches!(
        template_save(
            &store,
            &ben,
            ws,
            "t1",
            "Ben's",
            Engine::Template,
            "ben-code",
            2
        )
        .await,
        Err(RenderTemplateError::Denied)
    ));
    assert!(matches!(
        template_delete(&store, &ben, ws, "t1", 2).await,
        Err(RenderTemplateError::Denied)
    ));

    // Ada's template is untouched. A teammate (Ben) CAN read it (workspace-shared read).
    assert_eq!(
        template_get(&store, &ben, ws, "t1").await.unwrap().code,
        "ada-code"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn over_size_cap_is_rejected() {
    let ws = "ws-tpl-cap";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    let big = "x".repeat(TEMPLATE_MAX_BYTES + 1);
    assert!(matches!(
        template_save(&store, &ada, ws, "big", "Big", Engine::Template, &big, 1).await,
        Err(RenderTemplateError::BadInput(_))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn upsert_is_idempotent_on_replay() {
    // Offline/sync: the same (table,id) upsert replays without duplicating (§6.8). Re-saving the same
    // id at the same logical ts yields the identical record.
    let ws = "ws-tpl-sync";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, ALL);

    let first = template_save(&store, &ada, ws, "t1", "T1", Engine::Plot, "Plot.dot()", 5)
        .await
        .unwrap();
    let replay = template_save(&store, &ada, ws, "t1", "T1", Engine::Plot, "Plot.dot()", 5)
        .await
        .unwrap();
    assert_eq!(first, replay);
    assert_eq!(template_list(&store, &ada, ws).await.unwrap().len(), 1);
}
