//! `new_sid` — mint an opaque session id (browser-session scope).
//!
//! This is the ONLY thing the browser holds, so it is the whole authority a stolen cookie carries: it
//! must be unguessable. 256 bits from the OS CSPRNG, hex-encoded.
//!
//! Explicitly **not** the dev plugins' `s${counter}_${Date.now()}` (`cc-app/ui/vite-dev-auth.ts:46`,
//! and its ems twin), which is trivially guessable — fine there only because it never leaves a dev box
//! and its own comment says so ("Not cryptographic"). Shipping that shape into a product is the exact
//! failure this scope exists to prevent.

use rand::RngCore;

/// Session-id entropy. 256 bits: overkill is free here, and the id is the only credential in the
/// browser.
const SID_BYTES: usize = 32;

/// A fresh, unguessable session id (64 hex chars). Sourced from `OsRng` — the OS CSPRNG — never a
/// counter, a timestamp, or a `ThreadRng` seeded from one.
pub fn new_sid() -> String {
    let mut bytes = [0u8; SID_BYTES];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let mut out = String::with_capacity(SID_BYTES * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sid_is_256_bits_of_hex() {
        let sid = new_sid();
        assert_eq!(sid.len(), SID_BYTES * 2, "64 hex chars");
        assert!(sid.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn sids_do_not_repeat() {
        // A counter-based sid (the dev shape) would collide trivially here.
        let a: std::collections::HashSet<String> = (0..256).map(|_| new_sid()).collect();
        assert_eq!(a.len(), 256, "every sid is distinct");
    }
}
