//! Identity, token mint/verify, and the principal (README §6.6, auth-caps scope).
//!
//! A token is an Ed25519-signed JWT carrying a single workspace claim. Edge nodes verify
//! offline with the public key; the hub mints. The token shape is the §13 forever decision —
//! see `claims.rs`.

mod claims;
mod keypair;
mod mint;
mod principal;
mod token;
mod verify;

pub use claims::Claims;
pub use keypair::SigningKey;
pub use mint::mint;
pub use principal::{Principal, Role};
pub use token::claims_unverified;

pub use verify::{verify, AuthError};
