//! grafana-conv-app — the thin Rust seam around the mapper crate (grafana-
//! conversion scope, Stage 0). One route: `POST /convert` with a Grafana
//! dashboard JSON body → `{ dashboard, report }` JSON response.
//!
//! This is the browser build's seam; a Tauri command wrapper for desktop reuses
//! the same `mapper::convert` (the open question "Serve seam shape" is resolved
//! here: Axum + a future Tauri command around one native crate). No state, no
//! auth — the tool owns no token / workspace / store.

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower_http::cors::CorsLayer;

#[derive(Debug, Deserialize)]
struct ConvertRequest {
    /// The raw Grafana dashboard JSON (envelope-wrapped or bare).
    grafana: Value,
}

#[derive(Debug, Serialize)]
struct ConvertResponse {
    dashboard: Value,
    report: grafana_conv_mapper::ConversionReport,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Clone, Default)]
struct AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let addr = std::env::var("GRAFANA_CONV_ADDR").unwrap_or_else(|_| "127.0.0.1:7878".into());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    eprintln!("grafana-conv serving POST /convert on http://{addr}");
    axum::serve(listener, app()).await.unwrap();
}

fn app() -> Router {
    Router::new()
        .route("/convert", post(convert))
        .layer(CorsLayer::permissive())
        .with_state(AppState)
}

async fn convert(State(_s): State<AppState>, Json(req): Json<ConvertRequest>) -> impl IntoResponse {
    let body = match serde_json::to_string(&req.grafana) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("encode input: {e}"),
                }),
            )
                .into_response();
        }
    };
    match grafana_conv_mapper::convert(&body) {
        Ok((dashboard, report)) => {
            let dashboard = serde_json::to_value(&dashboard).unwrap_or(Value::Null);
            (StatusCode::OK, Json(ConvertResponse { dashboard, report })).into_response()
        }
        Err(grafana_conv_mapper::ConvertError::Unsupported(msg)) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse { error: msg }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("{e}"),
            }),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn convert_route_maps_a_real_dashboard() {
        let resp = app()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/convert")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"grafana": {"title": "X", "panels": [{"type":"stat","gridPos":{"x":0,"y":0,"w":6,"h":4},"targets":[{"refId":"A"}]}]}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["dashboard"]["title"], "X");
        assert_eq!(v["dashboard"]["cells"][0]["view"], "stat");
    }

    #[tokio::test]
    async fn convert_route_rejects_v2() {
        let resp = app()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/convert")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"grafana": {"kind": "Dashboard", "spec": {}}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
