//! The store-surface gate every schedule verb runs: `store:schedule:read` / `store:schedule:write`
//! in the caller's workspace. A deny is opaque (`FlowsError::Denied`).

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};

use super::SCHEDULE_TABLE;
use crate::flows::error::FlowsError;

pub(super) fn authorize_write(principal: &Principal, ws: &str) -> Result<(), FlowsError> {
    authorize(principal, ws, Action::Write)
}

pub(super) fn authorize_read(principal: &Principal, ws: &str) -> Result<(), FlowsError> {
    authorize(principal, ws, Action::Read)
}

fn authorize(principal: &Principal, ws: &str, action: Action) -> Result<(), FlowsError> {
    match check(
        principal,
        &Request::new(ws, Surface::Store, SCHEDULE_TABLE, action),
    ) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(FlowsError::Denied),
    }
}
