//! The generic upload lane end to end (node-update scope §Seam 2) — the REAL router, the REAL auth
//! + caps wall, and a REAL in-test [`UploadSink`] whose backend is an in-process buffer.
//!
//! A test-local implementation of the public `UploadSink` trait is the seam working as designed, not
//! a fake backend (rule 9): the bytes really traverse `POST → PATCH → complete`, the offsets really
//! come from the sink, and the 409 really carries the sink's number.
//!
//! Asserted here: begin/append/complete/abort round-trips; a wrong offset yields **409 with the
//! corrected offset**; **resume by digest** returns the existing handle rather than a second partial;
//! the **sink's own cap** is what gates every call in the sequence; the sink's ceiling refuses an
//! oversized declaration at `begin`; and with **no sinks registered the routes do not exist**.

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use bytes::Bytes;
use common::{bearer, gateway, json_post, token, NOW};
use lb_host::{UploadError, UploadHandle, UploadMeta, UploadSink};
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt;

/// The capability THIS sink declares — a string in the platform's existing capability grammar, and
/// nothing core knows about. The sink chooses it, lb only enforces it (rule 10).
const SINK_CAP: &str = "mcp:package.upload:call";
const SINK: &str = "package";
const WS: &str = "nube";

// ---------------------------------------------------------------------------------------------
// A real in-test sink
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct Partial {
    bytes: Vec<u8>,
    digest: Option<String>,
    completed: bool,
}

/// A real `UploadSink` over an in-process backend. Its state IS the truth about offsets, exactly as
/// a real backend's is — the routes hold none.
#[derive(Default)]
struct BufferSink {
    partials: Mutex<HashMap<String, Partial>>,
    /// Monotonic id source — the sink allocates its own durable ids.
    next: Mutex<u64>,
    limit: u64,
    /// Every `append` call's chunk length, so the test can prove lb forwarded BOUNDED pieces rather
    /// than one buffered blob.
    chunk_sizes: Mutex<Vec<usize>>,
}

impl BufferSink {
    fn with_limit(limit: u64) -> Self {
        Self {
            limit,
            ..Default::default()
        }
    }
}

#[async_trait]
impl UploadSink for BufferSink {
    fn required_cap(&self) -> &str {
        SINK_CAP
    }

    fn max_upload_bytes(&self) -> u64 {
        self.limit
    }

    async fn begin(&self, meta: &UploadMeta) -> Result<UploadHandle, UploadError> {
        let mut partials = self.partials.lock().unwrap();
        // RESUME IDENTITY IS THE DIGEST: a begin carrying a digest we already hold a partial for
        // returns the EXISTING handle and its offset — never a second partial of the same artifact.
        if let Some(d) = meta.digest_hex.as_deref() {
            if let Some((id, p)) = partials
                .iter()
                .find(|(_, p)| p.digest.as_deref() == Some(d) && !p.completed)
            {
                return Ok(UploadHandle {
                    id: id.clone(),
                    offset: p.bytes.len() as u64,
                });
            }
        }
        let mut next = self.next.lock().unwrap();
        *next += 1;
        let id = format!("up-{next}");
        partials.insert(
            id.clone(),
            Partial {
                bytes: Vec::new(),
                digest: meta.digest_hex.clone(),
                completed: false,
            },
        );
        Ok(UploadHandle { id, offset: 0 })
    }

    async fn status(&self, id: &str) -> Result<UploadHandle, UploadError> {
        let partials = self.partials.lock().unwrap();
        let p = partials.get(id).ok_or(UploadError::NotFound)?;
        Ok(UploadHandle {
            id: id.to_string(),
            offset: p.bytes.len() as u64,
        })
    }

    async fn append(&self, id: &str, offset: u64, chunk: Bytes) -> Result<u64, UploadError> {
        self.chunk_sizes.lock().unwrap().push(chunk.len());
        let mut partials = self.partials.lock().unwrap();
        let p = partials.get_mut(id).ok_or(UploadError::NotFound)?;
        let expected = p.bytes.len() as u64;
        if offset != expected {
            return Err(UploadError::Offset { expected });
        }
        if expected + chunk.len() as u64 > self.limit {
            return Err(UploadError::TooLarge { limit: self.limit });
        }
        p.bytes.extend_from_slice(&chunk);
        Ok(p.bytes.len() as u64)
    }

    async fn complete(&self, id: &str, meta: &UploadMeta) -> Result<Value, UploadError> {
        let mut partials = self.partials.lock().unwrap();
        let p = partials.get_mut(id).ok_or(UploadError::NotFound)?;
        p.completed = true;
        // The sink's verdict, reported by lb verbatim. Verification is the backend's job.
        Ok(json!({
            "stored": true,
            "bytes": p.bytes.len(),
            "sha_declared": meta.digest_hex,
            "content": String::from_utf8_lossy(&p.bytes),
        }))
    }

    async fn abort(&self, id: &str) -> Result<(), UploadError> {
        self.partials
            .lock()
            .unwrap()
            .remove(id)
            .map(|_| ())
            .ok_or(UploadError::NotFound)
    }
}

// ---------------------------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------------------------

/// A real gateway with the sink registered, plus a token holding the sink's declared cap.
async fn app(sink: Arc<BufferSink>) -> (axum::Router, String, Arc<BufferSink>) {
    let (gw, key) = gateway().await;
    let gw = gw.with_upload_sinks(vec![(
        SINK.to_string(),
        sink.clone() as Arc<dyn UploadSink>,
    )]);
    let tok = token(&key, "user:test", WS, &[SINK_CAP]);
    (router(gw), tok, sink)
}

fn patch_req(uri: &str, range: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method("PATCH")
        .uri(uri)
        .header("content-range", range)
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

// ---------------------------------------------------------------------------------------------
// Round trip
// ---------------------------------------------------------------------------------------------

/// begin → append → append → complete, with the bytes arriving intact and the offset advancing off
/// the SINK's count, not lb's.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn begin_append_complete_round_trips() {
    let (app, tok, sink) = app(Arc::new(BufferSink::with_limit(1024))).await;

    let resp = app
        .clone()
        .oneshot(bearer(
            json_post(
                &format!("/uploads/{SINK}"),
                json!({"size": 10, "digest_hex": "abc"}),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let begun = body_json(resp).await;
    let id = begun["id"].as_str().unwrap().to_string();
    assert_eq!(begun["offset"], json!(0));

    for (range, chunk, expect) in [("bytes 0-4/10", "HELLO", 5), ("bytes 5-9/10", "WORLD", 10)] {
        let resp = app
            .clone()
            .oneshot(bearer(
                patch_req(&format!("/uploads/{SINK}/{id}"), range, chunk),
                &tok,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "PATCH {range}");
        assert_eq!(body_json(resp).await["offset"], json!(expect));
    }

    // GET reports the sink's offset — lb holds no upload state of its own.
    let resp = app
        .clone()
        .oneshot(bearer(
            common::get_req(&format!("/uploads/{SINK}/{id}")),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(body_json(resp).await["offset"], json!(10));

    let resp = app
        .clone()
        .oneshot(bearer(
            json_post(
                &format!("/uploads/{SINK}/{id}/complete"),
                json!({"size": 10, "digest_hex": "abc"}),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let verdict = body_json(resp).await;
    // lb reports the sink's verdict VERBATIM — it neither verifies nor rewrites it.
    assert_eq!(verdict["stored"], json!(true));
    assert_eq!(verdict["bytes"], json!(10));
    assert_eq!(verdict["content"], json!("HELLOWORLD"));
    assert_eq!(verdict["sha_declared"], json!("abc"));

    assert!(
        !sink.chunk_sizes.lock().unwrap().is_empty(),
        "the sink must have been driven through append, not handed a buffered body"
    );
}

/// A `PATCH` whose range does not begin at the sink's current offset is a **409 carrying the correct
/// offset**, so a client that lost track resumes without guessing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_wrong_offset_is_409_with_the_corrected_offset() {
    let (app, tok, _) = app(Arc::new(BufferSink::with_limit(1024))).await;
    let begun = body_json(
        app.clone()
            .oneshot(bearer(
                json_post(&format!("/uploads/{SINK}"), json!({"size": 10})),
                &tok,
            ))
            .await
            .unwrap(),
    )
    .await;
    let id = begun["id"].as_str().unwrap().to_string();

    app.clone()
        .oneshot(bearer(
            patch_req(&format!("/uploads/{SINK}/{id}"), "bytes 0-4/10", "HELLO"),
            &tok,
        ))
        .await
        .unwrap();

    // The client thinks it is at 0; the sink is at 5.
    let resp = app
        .clone()
        .oneshot(bearer(
            patch_req(&format!("/uploads/{SINK}/{id}"), "bytes 0-4/10", "HELLO"),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = body_json(resp).await;
    assert_eq!(body["error"], json!("offset"));
    assert_eq!(
        body["offset"],
        json!(5),
        "the 409 carries the SINK's offset"
    );
}

/// A `begin` re-sent with the same `digest_hex` returns the EXISTING handle and offset — never a
/// second partial of the same artifact on the backend's disk.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resume_by_digest_returns_the_existing_handle() {
    let (app, tok, _) = app(Arc::new(BufferSink::with_limit(1024))).await;
    let first = body_json(
        app.clone()
            .oneshot(bearer(
                json_post(
                    &format!("/uploads/{SINK}"),
                    json!({"size": 10, "digest_hex": "deadbeef"}),
                ),
                &tok,
            ))
            .await
            .unwrap(),
    )
    .await;
    let id = first["id"].as_str().unwrap().to_string();
    app.clone()
        .oneshot(bearer(
            patch_req(&format!("/uploads/{SINK}/{id}"), "bytes 0-4/10", "HELLO"),
            &tok,
        ))
        .await
        .unwrap();

    // A client that lost its id (browser refresh) re-begins with the same digest.
    let again = body_json(
        app.clone()
            .oneshot(bearer(
                json_post(
                    &format!("/uploads/{SINK}"),
                    json!({"size": 10, "digest_hex": "deadbeef"}),
                ),
                &tok,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        again["id"],
        json!(id),
        "the same handle, not a second partial"
    );
    assert_eq!(again["offset"], json!(5), "and its real offset");
}

/// `DELETE` aborts: the partial is gone and a later `PATCH` against it is `404`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn abort_discards_the_partial() {
    let (app, tok, _) = app(Arc::new(BufferSink::with_limit(1024))).await;
    let begun = body_json(
        app.clone()
            .oneshot(bearer(
                json_post(&format!("/uploads/{SINK}"), json!({"size": 10})),
                &tok,
            ))
            .await
            .unwrap(),
    )
    .await;
    let id = begun["id"].as_str().unwrap().to_string();

    let resp = app
        .clone()
        .oneshot(bearer(
            common::delete_req(&format!("/uploads/{SINK}/{id}")),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .clone()
        .oneshot(bearer(
            patch_req(&format!("/uploads/{SINK}/{id}"), "bytes 0-4/10", "HELLO"),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------------------------
// The wall
// ---------------------------------------------------------------------------------------------

/// **The sink's OWN cap is what gates the lane**, and it is checked on EVERY call in the sequence —
/// not only at `begin` — so a session that loses its grant mid-upload stops there. Here the
/// mid-sequence caller simply never had it: a token with a rich but unrelated cap set is `403` at
/// begin, at append, at status, at complete and at abort.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_sinks_own_cap_gates_every_call_in_the_sequence() {
    let (gw, key) = gateway().await;
    let sink = Arc::new(BufferSink::with_limit(1024));
    let gw = gw.with_upload_sinks(vec![(SINK.to_string(), sink as Arc<dyn UploadSink>)]);
    let app = router(gw);

    // A well-provisioned caller who happens NOT to hold the sink's cap.
    let weak = token(
        &key,
        "user:member",
        WS,
        &[
            "mcp:dashboard.list:call",
            "store:*:read",
            "mcp:other.upload:call",
        ],
    );

    let resp = app
        .clone()
        .oneshot(bearer(
            json_post(&format!("/uploads/{SINK}"), json!({"size": 10})),
            &weak,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "begin");

    // The mid-sequence calls are refused identically — the id is never even looked at.
    for req in [
        patch_req(&format!("/uploads/{SINK}/up-1"), "bytes 0-4/10", "HELLO"),
        common::get_req(&format!("/uploads/{SINK}/up-1")),
        json_post(
            &format!("/uploads/{SINK}/up-1/complete"),
            json!({"size": 10}),
        ),
        common::delete_req(&format!("/uploads/{SINK}/up-1")),
    ] {
        let resp = app.clone().oneshot(bearer(req, &weak)).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "every call in the sequence re-checks the sink's cap"
        );
    }
}

/// An unauthenticated caller gets no lane at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unauthenticated_caller_is_refused() {
    let (app, _tok, _) = app(Arc::new(BufferSink::with_limit(1024))).await;
    let resp = app
        .oneshot(json_post(&format!("/uploads/{SINK}"), json!({"size": 10})))
        .await
        .unwrap();
    assert!(
        resp.status() == StatusCode::UNAUTHORIZED || resp.status() == StatusCode::FORBIDDEN,
        "an unauthenticated upload must be refused, got {}",
        resp.status()
    );
}

/// A declared size over the sink's ceiling is refused at `begin` — before a single byte is accepted.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_oversized_declaration_is_refused_at_begin() {
    let (app, tok, _) = app(Arc::new(BufferSink::with_limit(16))).await;
    let resp = app
        .oneshot(bearer(
            json_post(&format!("/uploads/{SINK}"), json!({"size": 1_000_000})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

/// A stream that exceeds its own declared `Content-Range` is cut off mid-append — the client's
/// framing is the contract it asked to be held to.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stream_exceeding_its_declared_range_is_cut_off() {
    let (app, tok, _) = app(Arc::new(BufferSink::with_limit(1024))).await;
    let begun = body_json(
        app.clone()
            .oneshot(bearer(
                json_post(&format!("/uploads/{SINK}"), json!({"size": 10})),
                &tok,
            ))
            .await
            .unwrap(),
    )
    .await;
    let id = begun["id"].as_str().unwrap().to_string();
    let resp = app
        .oneshot(bearer(
            // Declares 2 bytes, sends 10.
            patch_req(
                &format!("/uploads/{SINK}/{id}"),
                "bytes 0-1/10",
                "HELLOWORLD",
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// A malformed (or missing) `Content-Range` is a `400` — lb never guesses an offset, because
/// guessing is how a resumable upload silently corrupts an artifact.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_missing_content_range_is_refused() {
    let (app, tok, _) = app(Arc::new(BufferSink::with_limit(1024))).await;
    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/uploads/{SINK}/up-1"))
        .body(Body::from("HELLO"))
        .unwrap();
    let resp = app.oneshot(bearer(req, &tok)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// An UNREGISTERED sink name is `404` — the registry is the only thing that makes a lane exist, and
/// the core names none of them (rule 10).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unknown_sink_name_is_404() {
    let (app, tok, _) = app(Arc::new(BufferSink::with_limit(1024))).await;
    let resp = app
        .oneshot(bearer(
            json_post("/uploads/firmware", json!({"size": 10})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------------------------
// Unconfigured
// ---------------------------------------------------------------------------------------------

/// With NO sinks registered the routes are not mounted at all — every existing node's router is
/// byte-for-byte unchanged. An unmounted route cannot be a surface.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_sinks_means_no_routes() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = token(&key, "user:test", WS, &[SINK_CAP]);
    let resp = app
        .oneshot(bearer(
            json_post(&format!("/uploads/{SINK}"), json!({"size": 10})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "with no sinks the upload lane does not exist"
    );
    let _ = NOW;
}
