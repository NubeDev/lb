# An extension cannot list its own store table under an enforced row policy

- Date: 2026-11-13
- Area: auth-caps / store (`host/src/store_query`)
- Status: fixed

## Symptom

The `schedule-groups` extension installed on a node whose estate datasource has an enforced row
policy, then failed every page load and background pass with `host denied the call
(capability/workspace gate)`. The same build works on a node with no row policy.

## Root cause

The extension keeps its state in its own store tables and reads them whole (`SELECT ... FROM
<its table> ORDER BY k LIMIT $page START $start`) through `store.query`. Its backend calls as
`ext:<id>`, whose entity scope under an enforced policy is "nothing", and `store.query` is refused
outright for any restricted caller (a statement can reach other tables through record links). The
own-records exception added in
[extension-refused-its-own-tables-under-row-policy.md](extension-refused-its-own-tables-under-row-policy.md)
covers only `store.get`, which needs the record id up front, so there was no way to enumerate a
table the extension owns.

## Fix

New `store.list {table, after?, limit?}` → `{table, rows: [{id, value}], next}`: one named table, a
page at a time on `lb_store::scan`'s id cursor (page capped at `MAX_SCAN_LIMIT`), no query text. It
shares `store.get`'s gate (`authorize_table_read` in `store_query/get.rs`): `mcp:store.list:call`,
never a secret-plane table, `store:<table>:read`, and the entity limit with the same exception for
an extension whose caps name the table exactly. `store.query` stays refused.

## Regression test

`host/tests/federation_ext_own_tables_test.rs`:
`store_list_pages_an_extensions_own_table_and_limits_everyone_else` (paging, another workspace's row
never listed, wildcard extension / restricted person / no table cap / no verb cap all refused).
