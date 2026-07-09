# Frontend scope — Admin/console page layout patterns (copy-this-shape)

Status: scope (the ask + the recipe). Promotes to `public/frontend/` once a second console
(beyond the Access console) adopts it. Companion to `ui-standards-scope.md` (which owns the
"shadcn-first / one look / responsive" rules); this doc is the **concrete layout recipes** a
list-or-list/detail admin page copies so every console reads as one system.

The Access console (`ui/src/features/admin/`) is the reference implementation. These patterns
were extracted from it so the next console (settings, a workspace admin, an extension's admin
UI, …) can be built by copying shapes instead of re-deriving them. Every className below is
token-only (`bg`, `fg`, `muted`, `accent`, `border`, `panel`, `panel-2`) — no hex, light +
dark handled by the tokens.

---

## When to use which pattern

| Your page is… | Use |
|---|---|
| A tabbed console (several views under one header) | **P1 Floating content panel** + one tab per view |
| A single list ("things + New thing") | **P2 List-toolbar tab** |
| A list on the left, an editor on the right | **P3 List/detail split** (build each side from P2) |
| A "New X" that opens a form without leaving the list | **P4 Inline create-form flow** |

The building blocks are already committed — do **not** re-invent them:

- `ui/src/components/ui/table.tsx` — the shared `Table` (pass `sticky` on `TableHeader`).
- `ui/src/features/admin/AdminToolbar.tsx` — the shared top bar (search + action slot).
- `ui/src/components/app/empty-state.tsx` — `AppEmptyState` for empty/filtered/loading-done.
- `ui/src/components/ui/tabs.tsx` — the `Tabs` primitive (flex-height-aware `TabsContent`).
- `ui/src/components/app/page-header.tsx` — `AppPageHeader` (full-bleed, owns its hairline).
- `ui/src/lib/session` — `CAP` + `hasCap(caps, CAP.x)` to gate a control for DISPLAY.

---

## P1 — The floating content panel (tabbed console)

**Goal:** the tab strip and the content below it read as two matching floating panels, echoing
the NavRail's floating-card treatment, instead of the content sitting flat on the page.

**Recipe** (`ui/src/features/admin/AdminView.tsx` is the reference):

- Keep `<AppPageHeader>` **full-bleed** at the top — never wrap it in the panel; it owns its
  accent wash + two-hue bottom hairline and must span edge-to-edge.
- The `<Tabs>` gets a shared float gutter: `min-h-0 flex-1 gap-0 px-2 pb-2`
  (`gap-0` because the strip margin + panel now own the spacing).
- The `<TabsList>` floats with a matching gutter: `mx-0 mb-2 mt-2 flex-wrap`.
- Wrap **all** `<TabsContent>` panels in one panel div that mirrors the tab-strip material:

```tsx
<div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border border-border bg-panel-2/70">
  <TabsContent value="…" className="min-h-0 flex-1"> … </TabsContent>
  …
</div>
```

**Why each token:**
- `rounded-lg border border-border bg-panel-2/70` = the exact `TabsList` recipe → same material,
  so the two panels visually rhyme (the whole point).
- `overflow-hidden` clips each tab's toolbar top-edge and the table's sticky header/rows to the
  corner radius — no square-corner clash, and no per-tab rounding needed.
- `min-h-0 flex-1 flex-col` preserves the flex-height chain so each tab's inner
  `overflow-y-auto` still owns the bounded scroll.

**Scope note:** apply this in the *console's own* `*View.tsx`, not in the shared `Tabs`
primitive — other screens using `Tabs` keep the default. (CLAUDE §10: no core primitive change
for one feature's look.)

---

## P2 — The list-toolbar tab (a single "things + New thing" view)

**Goal:** every list view has the identical top bar and table, so tabs don't drift apart.

**Recipe** (`ui/src/features/admin/ApiKeysAdmin.tsx`, `WorkspacesAdmin.tsx` are references):

```tsx
<div className="flex min-h-0 flex-1 flex-col">
  {error && ( /* role="alert" destructive strip, see below */ )}

  <AdminToolbar
    search={filter}
    onSearch={setFilter}
    searchPlaceholder="Filter …"
    action={ /* the New-X button; see P4 for the toggle */ }
  />

  <div className="min-h-0 flex-1 overflow-y-auto">
    {visible.length === 0 ? (
      <AppEmptyState icon={Icon} title={filter ? "No … match." : "No … yet."} description={…} />
    ) : (
      <Table aria-label="…">
        <TableHeader sticky>
          <TableRow>
            <TableHead>…</TableHead>
            <TableHead className="text-right">Actions</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>{visible.map((row) => ( <TableRow key={row.id}> … </TableRow> ))}</TableBody>
      </Table>
    )}
  </div>
</div>
```

**Invariants (hold these across every list tab):**
- Scroll container is **edge-to-edge** (`overflow-y-auto`, **no** horizontal padding) — the
  cell's own `px-3` is the only inset. Do **not** add `px-4` to the scroll wrapper; that was the
  cause of the "API Keys/Nav tables are indented differently" bug. If a non-table child (a
  create form, a banner) needs inset, put the padding **on that child** (`mx-4 mt-3`), not the
  container.
- First column `className="font-medium text-fg"`; secondary columns `text-muted`; id/prefix
  columns `font-mono text-muted`; actions column `className="text-right"` with the buttons in a
  `<span className="flex justify-end gap-1">`.
- Row action buttons: `variant="ghost"` for benign (Edit/Rotate/Make default), `variant="destructive"`
  for Delete/Revoke/Purge. Never a `red-…` literal — the `destructive` variant owns the token.
- Error strip: `role="alert" className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-xs text-destructive"`.

---

## P3 — The list/detail split (roster + editor, aligned)

**Goal:** a roster on the left and an editor on the right whose **top bars sit on the exact same
row**. This is the pattern that's easy to get subtly wrong.

**Recipe** (`ui/src/features/admin/RolesAdmin.tsx` is the reference):

```tsx
<div className="flex min-h-0 flex-1 flex-col md:flex-row">
  {/* LEFT: roster — stacks above the editor on a phone */}
  <div className="flex min-w-0 flex-1 flex-col border-b border-border md:max-w-[32rem] md:border-b-0 md:border-r">
    <AdminToolbar … />                              {/* the bar */}
    <div className="min-h-0 flex-1 overflow-y-auto"> … Table … </div>   {/* the scroll body */}
  </div>

  {/* RIGHT: editor — SAME two-part structure so the bars align */}
  <div className="flex min-w-0 flex-1 flex-col">
    {!selected ? (
      <AppEmptyState … />
    ) : (
      <>
        {/* the bar — mirrors AdminToolbar's box exactly */}
        <div className="flex min-h-12 items-center gap-2 border-b border-border bg-panel px-4 py-2">
          <h2 className="text-sm font-semibold text-fg">Edit …</h2>
          <div className="ml-auto flex items-center gap-2"> … Save / Cancel … </div>
        </div>
        {/* the scroll body */}
        <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-4 py-4"> … form … </div>
      </>
    )}
  </div>
</div>
```

**The alignment rule (this is the whole trick):**
- **Both panels are `flex flex-col`.** Both lead with a **top bar as the first flex child**,
  then a separate scroll region. Same structure ⇒ same geometry ⇒ same row.
- **Both bars share the same box:** `min-h-12` (48px) + `py-2` + `border-b border-border` +
  `bg-panel`. `AdminToolbar` already carries `min-h-12`; give the editor bar the same. The
  `min-h-12` pins the height regardless of content (a bar with only a title + button is
  otherwise shorter than one with an `h-8` search input).
- **Do NOT** make the editor header a `sticky` element with `-mt-4`/`-mx-4` negative margins
  inside a padded scroll pane. That hack offsets its effective top from the left toolbar and is
  exactly what made Roles look "uneven". A plain first-child bar stays put anyway (it's outside
  the scroll region), so you keep the pinned-Save behavior without the misalignment.
- Responsive: `flex-col md:flex-row` (stacks on phone), `md:max-w-[32rem]` on the roster,
  border flips `border-b` → `md:border-r`. No fixed `w-1/2`.

---

## P4 — The inline create-form flow ("New X" without leaving the list)

**Goal:** clicking "New X" reveals a form in place (list stays mounted, search bar stays put),
and the button flips to "Cancel". Consistent across every "create" affordance.

**Recipe** (`ui/src/features/admin/ApiKeysAdmin.tsx`, `WorkspacesAdmin.tsx`, `nav/NavAdmin.tsx`):

- Toolbar action toggles label + variant on a `creating` flag; the `Plus` icon shows **only**
  in the "New X" state (the "＋ Cancel" bug — icon left on when the label flipped — is fixed by
  this):

```tsx
action={
  canCreate && (
    <Button
      variant={creating ? "outline" : "default"}
      size="sm"
      aria-label="new …"
      onClick={() => setCreating((c) => !c)}
    >
      {creating ? "Cancel" : (<><Plus size={13} /> New …</>)}
    </Button>
  )
}
```

- The form renders as the **first child of the scroll body**, inset with `mx-4 mt-3` (NOT via
  container padding — keeps the table edge-to-edge, per P2):

```tsx
<div className="min-h-0 flex-1 overflow-y-auto">
  {creating && (
    <div className="mx-4 mt-3 space-y-3 rounded-md border border-border bg-panel px-3 py-3">
      <label className="block text-xs text-muted">Field
        <Input aria-label="…" className="mt-1" value={…} onChange={…} />
      </label>
      <Button size="sm" aria-label="create …" disabled={!valid} onClick={() => void save()}>Create …</Button>
    </div>
  )}
  … empty-state / Table …
</div>
```

- **Cap-gate the button for display:** `const canCreate = hasCap(caps, CAP.xCreate)` and render
  the action only when `canCreate`. Pass `caps` down from the console `*View.tsx`. This is
  **display convenience only** — the gateway re-checks the verb server-side (never the boundary).
- Slugify ids that are keys (e.g. a workspace id = the SurrealDB namespace):
  `value.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")`. Free-text names
  stay verbatim; default a blank name to the id.

For a large editor (like Nav's) instead of a small form, keep the same toolbar-stays-mounted
idea: render the editor **below the always-mounted toolbar** and flip the action to "Cancel" —
don't replace the whole roster (incl. its search bar) with the editor.

---

## The pre-flight checklist (run before calling an admin page done)

1. **Panels rhyme** — content sits in the P1 floating panel; it matches the tab strip material.
2. **Bars align** — in a list/detail split, both top bars are `min-h-12 py-2 border-b bg-panel`
   first-children (P3). No sticky/negative-margin header.
3. **Tables are edge-to-edge** — no horizontal padding on the scroll container; insets live on
   non-table children via `mx-4` (P2/P4).
4. **Create flow is consistent** — `Plus` only in the "New" state; button flips to "Cancel";
   form is inline; button cap-gated with `hasCap` (P4).
5. **Tokens only** — no `red-…`/hex; `destructive` variant for destructive actions.
6. **Responsive** — `flex-col md:flex-row`, no fixed `w-1/2`/`w-64`; multi-column stacks.
7. **Shared primitives** — shadcn `Table`/`Button`/`Input`/`Select`/`Checkbox` + `AdminToolbar`
   + `AppEmptyState`; no raw `<table>`/`<button>`/`<input>`; no local page header (the console's
   `AppPageHeader` + tab strip own it).
8. **Real-gateway test** — a `*.gateway.test.tsx` drives the change against a spawned node and
   asserts observable DOM (CLAUDE §9 — no fakes). Cap-gated controls get the real cap passed in
   (`caps={[CAP.x]}`) so the control renders.

---

## Open questions / follow-ups

- Promote to `public/frontend/` once a **second** console adopts P1–P4 (proves they generalize).
- The editor bar in P3 is intentionally not sticky anymore; if a future editor is short enough
  that a sticky Save is wanted back, do it *inside the scroll body* — never by offsetting the bar.
- `Select` stays a native `<select>` (accessible/mobile choice, 32 files); a `Combobox`
  migration is a separate app-wide task, out of scope here.
