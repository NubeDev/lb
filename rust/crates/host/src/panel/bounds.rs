//! Panel spec bounds (library-panels scope: "same record-growth bounds `dashboard.save` applies to
//! cells"). A panel stores exactly the non-layout v3 spec, so it reuses the dashboard's shared
//! [`crate::dashboard::check_spec_bounds`] check verbatim — one authority for the `transformations[]`
//! + `fieldConfig` caps, wrapped here in [`PanelError::BadInput`].

use super::error::PanelError;
use super::model::PanelSpec;

/// Reject a panel whose v3 spec would exceed the panel-model caps (bounded growth keeps the record
/// small for the roster read), reusing the dashboard cell check.
pub fn check_spec_bounds(spec: &PanelSpec, label: &str) -> Result<(), PanelError> {
    crate::dashboard::check_spec_bounds(&spec.transformations, &spec.field_config, label)
        .map_err(PanelError::BadInput)
}
