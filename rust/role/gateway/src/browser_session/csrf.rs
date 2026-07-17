//! The CSRF gate for `/api/*` (browser-session scope) — **the gate on this scope shipping at all**,
//! not a follow-up.
//!
//! The moment a cookie authenticates a request, the browser attaches it to cross-origin requests the
//! *attacker's* page made, and `CorsLayer::permissive()` (`server.rs`) would happily hand back the
//! reply. CORS is not a defence here: it governs who may READ the response, while a CSRF write has
//! already landed by then. So `/api/*` is excluded from the permissive layer (`server.rs`) and gated
//! here instead.
//!
//! The rule, on every **unsafe** method (anything but GET/HEAD/OPTIONS):
//!   1. `Sec-Fetch-Site` — if the browser sent it, it is authoritative and unforgeable by page JS.
//!      Only `same-origin` (and `none`, a user-typed navigation) may write.
//!   2. `Origin` — the fallback for browsers/proxies that omit `Sec-Fetch-Site`. It must be present
//!      and must equal this request's own `Host`. A cross-site form POST always carries the
//!      attacker's `Origin`; forging it from page JS is impossible.
//!   3. Neither header ⇒ **reject**. A browser always sends at least one on an unsafe cross-origin
//!      request, so "no evidence of same-origin" is not a pass. (This is why the seam is opt-in: a
//!      bearer-holding CLI keeps talking to the bare routes, which are untouched.)
//!
//! Safe methods are exempt: they carry the cookie too, but a cross-origin *read* cannot be seen by the
//! attacker's page without CORS letting it through — and `/api/*` no longer does.
//!
//! `SameSite=Lax` on the cookie itself is the first line; this is the second. Both, because `Lax` is a
//! same-*site* check (an evil subdomain of the same registrable domain passes it) and this is a
//! same-*origin* check.

use axum::http::{HeaderMap, Method};

/// Is this request allowed to perform an unsafe (state-changing) `/api/*` call?
pub fn is_allowed(method: &Method, headers: &HeaderMap) -> bool {
    // Safe methods: no state change; the strict CORS layer stops the reply being read cross-origin.
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return true;
    }

    let header = |n: &str| headers.get(n).and_then(|v| v.to_str().ok());

    // 1. `Sec-Fetch-Site` is set by the browser itself and cannot be spoofed by page JS.
    if let Some(site) = header("sec-fetch-site") {
        return matches!(site, "same-origin" | "none");
    }

    // 2. Fall back to `Origin` vs `Host`.
    let (Some(origin), Some(host)) = (header("origin"), header("host")) else {
        // 3. No evidence at all ⇒ reject.
        return false;
    };
    origin_matches_host(origin, host)
}

/// Does `Origin` name the same host we were reached on? Compares the origin's authority (host[:port])
/// to the `Host` header, ignoring the scheme — the two agree on port in practice, and a scheme
/// mismatch is a TLS problem, not a CSRF one.
fn origin_matches_host(origin: &str, host: &str) -> bool {
    // `Origin: null` (sandboxed iframe, some redirects) is never same-origin.
    let Some((_scheme, authority)) = origin.split_once("://") else {
        return false;
    };
    !authority.is_empty() && authority.eq_ignore_ascii_case(host)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hdrs(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                v.parse().unwrap(),
            );
        }
        h
    }

    #[test]
    fn safe_methods_always_pass() {
        assert!(is_allowed(&Method::GET, &HeaderMap::new()));
        assert!(is_allowed(&Method::HEAD, &HeaderMap::new()));
    }

    #[test]
    fn same_origin_fetch_passes() {
        assert!(is_allowed(
            &Method::POST,
            &hdrs(&[("sec-fetch-site", "same-origin")])
        ));
        // A user-typed navigation / direct client.
        assert!(is_allowed(
            &Method::POST,
            &hdrs(&[("sec-fetch-site", "none")])
        ));
    }

    #[test]
    fn cross_site_is_rejected_even_with_a_valid_cookie() {
        // THE attack: evil.com POSTs with the victim's cookie riding along.
        assert!(!is_allowed(
            &Method::POST,
            &hdrs(&[("sec-fetch-site", "cross-site")])
        ));
        assert!(!is_allowed(
            &Method::POST,
            &hdrs(&[("sec-fetch-site", "same-site")]),
        ));
    }

    #[test]
    fn origin_fallback_matches_host() {
        assert!(is_allowed(
            &Method::POST,
            &hdrs(&[
                ("origin", "http://pi.local:8391"),
                ("host", "pi.local:8391")
            ])
        ));
    }

    #[test]
    fn foreign_origin_is_rejected() {
        assert!(!is_allowed(
            &Method::POST,
            &hdrs(&[("origin", "http://evil.com"), ("host", "pi.local:8391")])
        ));
        // `Origin: null` must never pass.
        assert!(!is_allowed(
            &Method::POST,
            &hdrs(&[("origin", "null"), ("host", "pi.local:8391")])
        ));
    }

    #[test]
    fn no_evidence_is_rejected() {
        // A bare cross-origin POST from a non-browser carrying a stolen cookie.
        assert!(!is_allowed(
            &Method::POST,
            &hdrs(&[("host", "pi.local:8391")])
        ));
        assert!(!is_allowed(&Method::POST, &HeaderMap::new()));
    }
}
