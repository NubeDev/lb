//! Real [`Provider`](crate::Provider) adapters — the backends the gateway swaps behind the one
//! contract (ai-gateway scope: "one gateway contract, many implementations"). Each file is one
//! adapter; the mock lives at the crate root (it is a test fixture, not a real backend).

mod openai_compat;
mod strip_think;

pub use openai_compat::OpenAiCompat;
