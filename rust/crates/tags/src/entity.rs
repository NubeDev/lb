//! Parse an entity reference (`table:id`) into the two parts a record link needs. An entity is any
//! taggable record — `series:node.cpu_temp`, `inbox:c1__42`, `doc:readme`. The id part may contain
//! dots and other punctuation (series names are dotted), so we MUST bind it as a separate string and
//! build the link with the two-arg `type::thing($tb, $id)` form — the one-arg `type::thing("t:id")`
//! mis-parses a dotted id and fails the statement (debugging/tags/dotted-entity-id-needs-two-arg.md).

/// Split `entity` into `(table, id)` on the first `:`. An entity with no `:` is treated as a bare
/// table with an empty id (callers always pass `table:id`, but this never panics).
pub fn entity_parts(entity: &str) -> (&str, &str) {
    entity.split_once(':').unwrap_or((entity, ""))
}
