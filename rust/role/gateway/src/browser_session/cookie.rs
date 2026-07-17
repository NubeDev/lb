//! Cookie parse + emit for the browser session (browser-session scope). lb had **no cookie code
//! anywhere** before this (`grep -rn cookie rust/**/*.rs` → zero), so this file is deliberately the
//! boring, well-trodden shape: read one name out of a `Cookie` header, emit one `Set-Cookie` with the
//! flags that make the session survivable and non-readable by JS.
//!
//! The attributes, and why each is load-bearing:
//!   - `HttpOnly` — JS cannot read it. The entire point: XSS can *ride* the session but cannot
//!     *exfiltrate* the credential, which a `localStorage` token hands over wholesale.
//!   - `SameSite=Lax` — the browser will not attach it to a cross-site POST. First line of CSRF
//!     defence; `csrf.rs` is the second, because `Lax` alone is a same-*site* check, not same-*origin*,
//!     and pre-2020 / non-default-Lax browsers exist.
//!   - `Path=/` — the shell's routes and `/api/*` share the cookie.
//!   - `Secure` — set only when the deployment is TLS, since an ARM/Pi box on plain http would
//!     otherwise silently drop the cookie and "login does nothing" all over again.

use axum::http::HeaderMap;

/// The session cookie name. `__Host-` is deliberately NOT used: it mandates `Secure`, which would
/// break the plain-http LAN/Pi deploys this seam exists to serve.
pub const COOKIE_NAME: &str = "lb_session";

/// Read the session id out of a request's `Cookie` header, if present.
pub fn read_sid(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let part = part.trim();
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        if k.trim() == COOKIE_NAME {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// The `Set-Cookie` value that establishes `sid`. `secure` comes from config (TLS ⇒ true).
pub fn set_cookie(sid: &str, secure: bool) -> String {
    let mut c = format!("{COOKIE_NAME}={sid}; Path=/; HttpOnly; SameSite=Lax");
    if secure {
        c.push_str("; Secure");
    }
    c
}

/// The `Set-Cookie` value that clears the session (logout, or a rejected/rotated sid).
pub fn clear_cookie(secure: bool) -> String {
    let mut c = format!("{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    if secure {
        c.push_str("; Secure");
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with(cookie: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(axum::http::header::COOKIE, cookie.parse().unwrap());
        h
    }

    #[test]
    fn reads_its_own_cookie_among_others() {
        let h = headers_with("theme=dark; lb_session=abc123; other=1");
        assert_eq!(read_sid(&h).as_deref(), Some("abc123"));
    }

    #[test]
    fn absent_or_empty_is_none() {
        assert_eq!(read_sid(&HeaderMap::new()), None);
        assert_eq!(read_sid(&headers_with("theme=dark")), None);
        assert_eq!(read_sid(&headers_with("lb_session=")), None);
        // A cookie whose NAME merely ends with ours must not match.
        assert_eq!(read_sid(&headers_with("not_lb_session=abc")), None);
    }

    #[test]
    fn set_cookie_is_httponly_and_lax() {
        let c = set_cookie("sid1", false);
        assert!(c.contains("HttpOnly"), "JS must never read the session");
        assert!(c.contains("SameSite=Lax"));
        assert!(c.contains("Path=/"));
        assert!(!c.contains("Secure"), "plain-http deploys must still work");
    }

    #[test]
    fn secure_is_set_under_tls() {
        assert!(set_cookie("sid1", true).contains("; Secure"));
    }

    #[test]
    fn clear_expires_immediately() {
        assert!(clear_cookie(false).contains("Max-Age=0"));
    }
}
