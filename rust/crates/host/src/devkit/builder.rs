//! Selects the [`Toolchain`](lb_devkit::Toolchain) a devkit build runs under — **config, not a code
//! branch** (README §3 rule 1; devkit-container-build-scope.md). `LB_DEVKIT_BUILDER=container`
//! opts a node into hermetic container builds (needs a container runtime); anything else (including
//! unset) keeps the existing in-process `ProcessToolchain`. `build_extension` never knows which one
//! ran.

use lb_auth::Principal;
use lb_devkit::{ContainerConfig, ContainerToolchain, ProcessToolchain, Toolchain};
use lb_secrets::get as secret_get;

use crate::boot::Node;

/// The `lb-secrets` path a build-scoped git token is read from (a distinct, build-only grant —
/// never the app's GitHub-bridge credential; see scope doc "Where the build token lives").
const GIT_TOKEN_SECRET_PATH: &str = "devkit/build-git-token";

/// The devkit build's own principal for the secret read — mediated server-side, never the caller's
/// authority (same shape as `federation::secret::mediate_dsn`).
const DEVKIT_BUILD_SUB: &str = "ext:devkit";

pub fn container_enabled() -> bool {
    std::env::var("LB_DEVKIT_BUILDER")
        .map(|v| v == "container")
        .unwrap_or(false)
}

/// Build the configured [`Toolchain`] for `ws`. Reads the optional build-scoped git token from
/// `lb-secrets` under the devkit build's own mediated principal — a private-dep build without the
/// secret configured still runs, and fails cleanly inside the container ("missing build credential")
/// rather than a raw git 401 (scope doc "Deny / failure paths").
pub async fn select_toolchain(node: &Node, ws: &str) -> Box<dyn Toolchain + Send + Sync> {
    if !container_enabled() {
        return Box::new(ProcessToolchain);
    }
    let mediator = Principal::routed(
        DEVKIT_BUILD_SUB,
        ws.to_string(),
        vec!["secret:devkit/*:get".into()],
    );
    let git_token = secret_get(&node.store, &mediator, ws, GIT_TOKEN_SECRET_PATH)
        .await
        .ok();
    let image = std::env::var("LB_DEVKIT_BUILD_IMAGE").unwrap_or_else(|_| "lazybones-build".into());
    let cache_volume =
        std::env::var("LB_DEVKIT_CACHE_VOLUME").unwrap_or_else(|_| "lazybones-cargo-cache".into());
    Box::new(ContainerToolchain {
        config: ContainerConfig {
            image,
            cache_volume,
            git_token,
        },
    })
}
