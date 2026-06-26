# UI design scope

Status: draft brief for the first frontend build.

The UI should copy the look and density of
`/home/user/code/rust/lazybones/ui` while staying simple for now. Treat that app
as the source reference for tokens, layout rhythm, component proportions, icons,
and the general "operator console" mood.

## Copy workflow

Use the local reference directly when possible:

```sh
cd /home/user/code/rust/lazybones/ui
```

If a clean working copy is safer, clone or copy the reference into `/tmp`, then
lift only the pieces needed for this repo:

```sh
cp -a /home/user/code/rust/lazybones/ui /tmp/lazybones-ui-reference
```

If the reference is coming from a remote repo instead of the local path, clone it
under `/tmp/lazybones-ui-reference` and work from that throwaway checkout.

Do not make source changes in `/home/user/code/rust/lazybones/ui` while building
the new UI. It is the visual source of truth, not the target.

## Look

- Compact app shell with a left rail and fixed top bar.
- Dark mode first, with a warm paper light mode.
- Warm amber accent, muted status colors, and low-contrast borders.
- Small typography, tabular numbers for metrics, and dashboard cards built for
  scanning.
- Rounded cards and buttons following the reference app's existing radii.
- Lucide icons for navigation, actions, health, and status.

## First screen

Open directly into a platform dashboard. Do not build a landing page.

The dashboard should show:

- current node role: edge, cloud hub, or solo
- store and bus status
- extension runtime status
- MCP tool surface status
- jobs, sync, inbox/outbox, and capability check summaries
- recent platform events as a compact list

## Initial navigation

- Dashboard
- Workspaces
- Extensions
- MCP tools
- Jobs
- Settings

Everything can be static or mock-backed in the first pass, as long as the
structure is ready for real data later.

## Boundaries

- Do not copy the lazybones build-orchestration domain screens wholesale.
- Do not introduce a new visual language.
- Do not add a marketing hero.
- Do not wire real backend calls until the API shape is known.
