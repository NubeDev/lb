//! The static-root **method-mismatch** fallback (spa-static-hosting scope): decide whether a request
//! that matched a path but no handler *for its method* is a browser **navigation** (→ serve the SPA's
//! `index.html`) or an **API call** (→ the 405 the router would have returned anyway).
//!
//! Why this exists: `Router::fallback_service` fires only when NO route matched the path at all. An
//! SPA route that collides with an lb route of a different method therefore never reaches the shell —
//! the router 405s first. `GET /login` is exactly that case (lb registers `POST /login`), so a
//! deployed shell served its whole UI and could not render a login page. See the scope for the full
//! diagnosis (`docs/scope/frontend/spa-static-hosting-scope.md`).
//!
//! The rule, in HTTP terms only — no path list, no host knowledge (rule 10):
//!
//! > method-mismatch **+** `GET`/`HEAD` **+** `Accept` *explicitly* prefers `text/html`
//! > → `200 index.html`. Anything else → the 405, `Allow` intact.
//!
//! **`Allow` is axum's job, not ours.** `RouteFuture::poll` calls `set_allow_header` on the way OUT,
//! wrapping whatever this fallback returns (`axum-0.8.9/src/routing/route.rs:164`), and it skips only
//! when the response *already* carries `Allow`. So returning a bare 405 gets the correct
//! `Allow: POST` for free — and setting one here by hand would SUPPRESS axum's real value. The
//! scope's chief worry ("axum does not hand `Allow` to the fallback, derive it from the router") is
//! inverted: the risk is over-writing it, not losing it. `static_root_method_mismatch_test.rs` pins
//! the header so a future refactor can't silently drop it.
//!
//! HEAD needs no special case either: axum strips the body of a top-level HEAD response itself, so
//! the same `index.html` reply serves `HEAD` correctly (`200`, `Content-Length`, empty body).

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, Method, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use tower::util::ServiceExt;
use tower_http::services::ServeFile;

use crate::state::Gateway;

/// Does `Accept` **explicitly** prefer HTML?
///
/// Deliberately strict: `*/*` (curl's default) and `application/*` do NOT count. A browser navigation
/// always sends an explicit `text/html` (or `application/xhtml+xml`); API clients never do. Treating
/// `*/*` as html-preferring would make `curl -X GET /mcp/call` return a web page — the single most
/// likely implementation bug in this rule, and the reason this is a named predicate with its own test
/// rather than an inline `contains("html")`.
///
/// A `q=0` on the HTML type is a rejection, not a preference, so it does not count either.
fn prefers_html(headers: &HeaderMap) -> bool {
    let Some(accept) = headers.get(header::ACCEPT).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    accept.split(',').any(|part| {
        let mut segs = part.split(';').map(str::trim);
        let media = segs.next().unwrap_or("");
        if !media.eq_ignore_ascii_case("text/html")
            && !media.eq_ignore_ascii_case("application/xhtml+xml")
        {
            return false;
        }
        // `text/html;q=0` explicitly refuses HTML — honour it rather than serve a page nobody wanted.
        !segs.any(|p| {
            p.strip_prefix("q=")
                .map(str::trim)
                .is_some_and(|q| q.parse::<f32>().is_ok_and(|q| q <= 0.0))
        })
    })
}

/// The `method_not_allowed_fallback` handler, mounted ONLY when `static_root` is `Some`
/// (`server.rs`). With no static root the router keeps today's behaviour byte-for-byte.
pub async fn spa_or_405(State(gw): State<Gateway>, req: Request<Body>) -> Response {
    let is_navigation = matches!(*req.method(), Method::GET | Method::HEAD);

    // `static_root` is Some at every mount site; `as_ref()` keeps this total rather than panicking if
    // that ever changes.
    let Some(dir) = gw.static_root.as_ref().as_ref() else {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    };

    if !is_navigation || !prefers_html(req.headers()) {
        // The API contract, unchanged: axum attaches the real `Allow` to this on the way out.
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    match ServeFile::new(dir.join("index.html")).oneshot(req).await {
        Ok(resp) => resp.into_response(),
        // No readable index.html => this isn't a shell we can serve; the 405 is still the honest
        // answer (and still gets its `Allow`). Never a 500 for a browser hitting an API path.
        Err(_) => StatusCode::METHOD_NOT_ALLOWED.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accept(v: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(header::ACCEPT, v.parse().unwrap());
        h
    }

    #[test]
    fn browser_navigation_accept_prefers_html() {
        // The real Chrome/Firefox navigation header.
        assert!(prefers_html(&accept(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"
        )));
    }

    #[test]
    fn wildcard_is_not_html_preferring() {
        // curl's default. MUST be false, or `curl -X GET /mcp/call` starts returning a web page.
        assert!(!prefers_html(&accept("*/*")));
    }

    #[test]
    fn api_clients_are_not_html_preferring() {
        assert!(!prefers_html(&accept("application/json")));
        assert!(!prefers_html(&HeaderMap::new()));
        // `application/*` must not sneak through a substring check.
        assert!(!prefers_html(&accept("application/*")));
    }

    #[test]
    fn explicit_html_rejection_is_not_a_preference() {
        assert!(!prefers_html(&accept("text/html;q=0")));
    }

    #[test]
    fn xhtml_and_casing_count() {
        assert!(prefers_html(&accept("application/xhtml+xml")));
        assert!(prefers_html(&accept("TEXT/HTML")));
    }
}
