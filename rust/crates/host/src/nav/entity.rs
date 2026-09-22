//! The **entity marker** on a menu row (entity-scoped-data scope): `entity: {table, id}` says "this
//! row is access to entity `id` of `table`". It is the `nav` source of a principal's entity scope —
//! the entities marked on the menu a principal was HANDED are the entities they may read.
//!
//! Deliberately a separate, explicit field rather than an inference from `vars` (a row's `siteRef`
//! var): vars are presentation bindings that also pre-select things, so treating them as access would
//! let a harmless shortcut quietly grant data. Opaque data to the core (rule 10) — lb never learns
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
}
