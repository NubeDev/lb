//! Read an extension's persisted install record — the durable approved-grant set for a workspace
//! (README §6.4, extensions scope). This is what a restart/reload consults instead of re-asking
//! the admin: `install_extension` persisted `requested ∩ admin_approved`, and this returns it,
//! workspace-first (an install in workspace B is invisible here for workspace A — README §7).

use lb_assets::{read_install, Install};
use lb_store::StoreError;

use crate::boot::Node;

/// Return the install record for `ext_id` in workspace `ws`, or `None` if not installed here.
pub async fn installed(node: &Node, ws: &str, ext_id: &str) -> Result<Option<Install>, StoreError> {
    read_install(&node.store, ws, ext_id).await
}
