# @nube/panel

A reusable, right-docked, **resizable** side panel for React — the ce-wiresheet
`InspectPanel` look rebuilt on **shadcn/ui** primitives, self-themed via scoped
`hsl(var(--lbp-*))` tokens (host-overridable). Dense, sectioned, and — the point —
**resizable**: drag the left edge (or focus it + arrow keys) to widen and reveal more
option columns.

```tsx
import { Panel, Section, PropTable, KV, NavMenu } from "@nube/panel";
import "@nube/panel/style.css";
import "@nube/nav-rail/style.css"; // the section rail's tokens (bundled separately)

<Panel
  open={open}
  onOpenChange={setOpen}
  title="Edit panel"
  description="One editor for add and edit."
  footer={<button onClick={save}>Save</button>}
>
  <div className="grid grid-cols-[9rem_1fr]">
    <NavMenu items={sections} active={tab} onSelect={setTab} />
    <div>
      <Section title="Properties (2)">
        <PropTable
          columns={[{ key: "name" }, { key: "value", ellipsize: true }]}
          rows={[{ id: "a", cells: { name: "speed", value: "42%" } }]}
        />
      </Section>
      <Section title="Metadata">
        <KV k="size" v="96 × 96" />
      </Section>
    </div>
  </div>
</Panel>
```

## Surface

- **`Panel`** — the docked, resizable shell (overlay + focus trap + escape from Radix
  dialog). `initialWidth` / `minWidth` / `maxWidth` bound the drag. Header (title +
  description + `headerAside`), a scrollable body (your `children`), and an optional
  `footer` action row.
- **`Section`** — a titled, dense group (uppercase muted label over children).
- **`PropTable`** — the dense, monospace property/edge table (columns + rows,
  ellipsizable value cells, per-row `tone: "warn"`).
- **`KV`** — a fixed-key-width key/value row.
- **`useResizable` / `ResizeHandle`** — the width controller + edge affordance, if you
  want to drive resize yourself.
- **`NavMenu`** — re-exported from `@nube/nav-rail` for the section rail.

## Theming

Every color is `hsl(var(--lbp-*))` scoped to the `.lb-panel` root the `Panel` puts on
its surface. Re-skin without forking:

```css
.lb-panel { --lbp-accent: 280 80% 60%; }
.lb-panel.theme-light { /* … */ }
```

## Library-stylesheet discipline

The bundle ships **theme + utilities only, NO preflight** — a library must not reset its
host (this exact bug bit `@nube/nav-rail`; see `nav-rail.css`). Verify:
`grep -c '@layer base' dist/panel.css` → `0`.

Dev/type React are pinned to `react@^18.3.1` / `@types/react@^18.3.12` /
`lucide-react@^0.460.0` to match the `ui` app (mismatched `@types/react` splits the
world and breaks lucide typecheck).
