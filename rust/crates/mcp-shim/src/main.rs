//! The bin entry — reads env and hands control to [`serve`]. Kept thin so all logic is testable
//! in lib tests (the binary is just the env wiring + the tokio runtime).

use lb_mcp_shim::serve;

#[tokio::main]
async fn main() {
    if let Err(e) = serve().await {
        eprintln!("lb-mcp-shim: {e}");
        std::process::exit(1);
    }
}
