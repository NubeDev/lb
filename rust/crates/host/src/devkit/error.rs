use thiserror::Error;

#[derive(Debug, Error)]
pub enum DevkitError {
    #[error("denied")]
    Denied,
    #[error("bad input: {0}")]
    BadInput(String),
    #[error("devkit: {0}")]
    Devkit(String),
    #[error("store: {0}")]
    Store(#[from] lb_store::StoreError),
    #[error("bus: {0}")]
    Bus(String),
}
