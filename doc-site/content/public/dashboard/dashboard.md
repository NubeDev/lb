# Dashboard

TODO — public docs for the dashboard/panel surface. Filled from the
`docs/scope/frontend/dashboard/` set as slices ship.

## Native import/export + the Dashboards manager

Every dashboard, a single widget, or any mix, can be exported to a portable **`.lbdash.json` bundle** and
imported back — into the same workspace, another workspace, or another node. The bundle is our own format
(not Grafana JSON): it carries the portable shape only — titles, cells/layout, variables, and per-widget
specs — and **never a workspace or owner**. Import always re-establishes those from your session, so a
bundle you export can only ever land in **your** workspace under **your** account.

- **Manage all dashboards** — the **Manage** button in the dashboard header opens the Dashboards manager
  (`/dashboards/manage`): a searchable table of every dashboard you can reach, with **create, rename,
  duplicate, delete**, plus **Import** and multi-select **Export**.
- **Export** — export the open dashboard (header ↓ icon), one widget (its hover ↓ icon), or several
  dashboards at once (select rows in the manager → **Export**). A `.lbdash.json` file downloads.
- **Import** — paste a bundle or choose a file; a preview shows exactly what it carries (and warns about
  anything it skipped) before you confirm. On an id collision you choose **Keep both** (a fresh, renamed
  copy — the default, never overwrites) or **Overwrite** (only your own records; the server enforces it).

A dashboard exported from Grafana is **not** a Lazybones bundle — importing one here is turned away with a
clear message (a Grafana interchange is a separate, backend-mapped path).

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
