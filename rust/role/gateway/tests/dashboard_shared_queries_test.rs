//! A board's NAMED QUERIES must survive the REST save (shared-queries scope).
//!
//! This exists because the field shipped broken once already. `POST /dashboards` deserializes into a
//! NAMED struct and forwards only the fields it lists, so a key the struct does not carry is dropped
//! two layers before the host ever sees it. The host tests passed, the MCP path worked, and the UI
//! still lost the author's configuration on reload — the same shape of failure the transport
//! whitelist has produced for `kind`, the heading fields, `varsDisplay`, `time` and `compact`.
//!
//! The panel half rides `cell.options`, which is an opaque map, so it is asserted here too: the two
//! halves are useless apart, and a board whose queries persist while its bindings vanish is no
//! better than one that stores neither.

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use tower::ServiceExt;

const CAPS: &[&str] = &["mcp:dashboard.get:call", "mcp:dashboard.save:call"];

fn alerts() -> Value {
    json!({ "alerts": {
        "tool": "insight.list",
        "args": { "tags": { "kind": "alert" }, "limit": 200, "counts": true }
    }})
}

/// One counter panel, bound to `alerts` and drawing a single number out of it.
fn bound_cell() -> Value {
    json!({
        "i": "total", "x": 0, "y": 0, "w": 3, "h": 3, "v": 2,
        "widget_type": "widget", "view": "stat", "binding": {},
        "options": { "sharedQuery": { "from": "alerts", "pick": { "path": "counts.total" } } }
    })
}

async fn save(gw: &lb_role_gateway::Gateway, tok: &str, body: Value) -> StatusCode {
    lb_role_gateway::router(gw.clone())
        .oneshot(bearer(json_post("/dashboards", body), tok))
        .await
        .unwrap()
        .status()
}

async fn get(gw: &lb_role_gateway::Gateway, tok: &str, id: &str) -> Value {
    let resp = lb_role_gateway::router(gw.clone())
        .oneshot(bearer(get_req(&format!("/dashboards/{id}")), tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    json_body(resp).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn named_queries_and_their_bindings_survive_the_rest_save() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:test", "nube", CAPS);

    assert_eq!(
        save(
            &gw,
            &tok,
            json!({
                "id": "b", "title": "B", "queries": alerts(), "cells": [bound_cell()]
            })
        )
        .await,
        StatusCode::OK
    );

    let d = get(&gw, &tok, "b").await;
    assert_eq!(
        d["queries"],
        alerts(),
        "the board's queries must round-trip verbatim"
    );
    assert_eq!(
        d["cells"][0]["options"]["sharedQuery"],
        json!({ "from": "alerts", "pick": { "path": "counts.total" } }),
        "the panel's binding must round-trip with it",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_layout_save_preserves_the_queries_it_does_not_mention() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:test", "nube", CAPS);

    save(
        &gw,
        &tok,
        json!({ "id": "b", "title": "B", "queries": alerts(), "cells": [] }),
    )
    .await;
    // A drag or resize sends cells and no `queries` at all. Blanking the block here would mean moving
    // one panel silently deletes the board's data wiring.
    save(&gw, &tok, json!({ "id": "b", "title": "B", "cells": [] })).await;

    assert_eq!(
        get(&gw, &tok, "b").await["queries"],
        alerts(),
        "omitting must PRESERVE"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_empty_object_reaches_the_host_as_the_explicit_clear() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:test", "nube", CAPS);

    save(
        &gw,
        &tok,
        json!({ "id": "b", "title": "B", "queries": alerts(), "cells": [] }),
    )
    .await;
    // `{}` is how an author removes the last named query. It must not be mistaken for "absent", or
    // the block becomes permanent once written.
    save(
        &gw,
        &tok,
        json!({ "id": "b", "title": "B", "queries": {}, "cells": [] }),
    )
    .await;

    assert_eq!(
        get(&gw, &tok, "b").await["queries"],
        json!({}),
        "an empty object clears"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_board_that_never_had_queries_reads_as_none() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:test", "nube", CAPS);
    save(
        &gw,
        &tok,
        json!({ "id": "old", "title": "Old", "cells": [] }),
    )
    .await;
    assert_eq!(get(&gw, &tok, "old").await["queries"], json!({}));
}
