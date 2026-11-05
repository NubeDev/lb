//! The **entity marker** on a menu row (entity-scoped-data scope): `entity: {table, id}` says "this
//! row is access to entity `id` of `table`". It is the `nav` source of a principal's entity scope —
//! the entities marked on the menu a principal was HANDED are the entities they may read.
//!
//! A marker is a separate, explicit field rather than an inference from `vars` (a row's `siteRef`
//! var): vars are presentation bindings that also pre-select things, so treating them as access can
//! let a harmless shortcut quietly grant data. A workspace that would rather keep one value per row
//! opts in per policy with the `nav_pin` source ([`collect_pinned`]): it accepts that trade, and the
//! marker can no longer disagree with the pin. Opaque data to the core (rule 10) — lb never learns
//! what a "site" is.

use serde::{Deserialize, Serialize};

use super::model::NavItem;

/// One entity a menu row grants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavEntity {
    pub table: String,
    pub id: String,
}

/// Every entity of `table` marked anywhere in the STORED rows `items` — any depth, any folder, labels
/// and order ignored — added to `out`. The rows are the handed menu as the admin authored it: marking
/// a row is the grant, whether or not its own destination is readable.
pub fn collect_entities(
    items: &[NavItem],
    table: &str,
    out: &mut std::collections::BTreeSet<String>,
) {
    for item in items {
        if let Some(e) = &item.entity {
            if e.table == table && !e.id.is_empty() {
                out.insert(e.id.clone());
            }
        }
        collect_entities(&item.items, table, out);
    }
}

/// Every value pinned to `var` anywhere in the STORED rows `items`, added to `out` — the `nav_pin`
/// source. Same reach as [`collect_entities`]: any depth, the menu as authored. Values are taken as
/// written, because an id must equal the data's tag value byte for byte; only a blank pin is skipped.
pub fn collect_pinned(items: &[NavItem], var: &str, out: &mut std::collections::BTreeSet<String>) {
    for item in items {
        if let Some(value) = item.vars.get(var) {
            if !value.trim().is_empty() {
                out.insert(value.clone());
            }
        }
        collect_pinned(&item.items, var, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(entity: Option<(&str, &str)>, items: Vec<NavItem>) -> NavItem {
        NavItem {
            entity: entity.map(|(t, i)| NavEntity {
                table: t.into(),
                id: i.into(),
            }),
            items,
            ..Default::default()
        }
    }

    #[test]
    fn entities_are_collected_at_any_depth_once_each() {
        let menu = vec![
            row(
                None,
                vec![
                    row(Some(("site", "A")), vec![]),
                    row(None, vec![row(Some(("site", "B")), vec![])]),
                ],
            ),
            row(Some(("site", "A")), vec![]),
            row(Some(("region", "north")), vec![]),
        ];
        let mut out = Default::default();
        collect_entities(&menu, "site", &mut out);
        assert_eq!(out.into_iter().collect::<Vec<_>>(), vec!["A", "B"]);
    }

    fn pinned(var: &str, value: &str, items: Vec<NavItem>) -> NavItem {
        NavItem {
            vars: [(var.to_string(), value.to_string())].into(),
            items,
            ..Default::default()
        }
    }

    #[test]
    fn pins_are_collected_at_any_depth_for_the_named_var_only() {
        let menu = vec![
            row(
                None,
                vec![pinned("siteRef", "A", vec![pinned("siteRef", "B", vec![])])],
            ),
            pinned("siteRef", "A", vec![]),
            pinned("stateRef", "VIC", vec![]),
            pinned("siteRef", "  ", vec![]),
            // A marker grants nothing to this source.
            row(Some(("site", "C")), vec![]),
        ];
        let mut out = Default::default();
        collect_pinned(&menu, "siteRef", &mut out);
        assert_eq!(out.into_iter().collect::<Vec<_>>(), vec!["A", "B"]);
    }

    #[test]
    fn a_pin_keeps_its_exact_bytes() {
        let mut out = Default::default();
        collect_pinned(
            &[pinned("siteRef", "Lot 2A )\t", vec![])],
            "siteRef",
            &mut out,
        );
        assert!(out.contains("Lot 2A )\t"));
    }
}
