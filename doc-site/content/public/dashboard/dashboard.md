# Dashboard

TODO — public docs for the dashboard/panel surface. Filled from the
`docs/scope/frontend/dashboard/` set as slices ship.

## New-panel wizard — binding a panel to a saved rule

The new-panel wizard's first step, **1. Source**, offers three intent buckets: **Insights** (a
findings triage list, no data source), **Workspace source** (anything already in this workspace —
a saved **rule**, **series**, or **saved query**), and **Datasource** (author a query against a
registered datasource). To bind a panel to a saved **rule**, pick **Workspace source**: the source
list opens with the **Rules** group first, and picking a rule sets the panel's source to
`{tool:"rules.run", args:{rule_id, route:false}}` (read-only — a 30 s auto-refresh never routes the
rule's findings). A workspace with no saved rules (or without the `mcp:rules.list:call` grant) shows
"No saved rules yet — create one in Rules", so the path is discoverable before the first rule exists.

In flight / scoped, not yet shipped:

- Rules power widgets: a panel/widget bound to a saved rule (`rules.run`) renders the rule's rows
  through the standard read path, with read-only (`route:false`) panel runs and chart-return
  helpers in the cage (`rules-for-widgets-scope.md`).
