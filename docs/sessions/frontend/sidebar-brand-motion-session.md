# Session — sidebar brand box + nav motion (motion.dev)

## Ask

Add tasteful, *clean* (not hacky) motion to two shell surfaces:

1. The workspace identity box at the top-left of the sidebar (the brand mark + name/tagline
   that doubles as the collapse toggle).
2. The sidebar nav itself — **only if it could be done cleanly**, otherwise leave it.

Use the React `motion` library (motion.dev) if it's clean and integrated, not a bolt-on. UX must
feel good.

## What made this "clean" and not a hack

The project already ships `motion@12` as a real dependency behind a single, disciplined seam:

- `ui/src/lib/motion/motion.ts` is the **one** import site of `motion` (a repo grep for
  `from "motion"` outside it is the guard). Everything imports primitives from `@/lib/motion`.
- `useMotionPref` folds `prefers-reduced-motion` **and** the member's theme motion pref
  (`off` / `subtle` / `full`) into `{ enabled, duration(), distance() }`. Every animation is gated
  through it, so the off-switch is trustworthy.

So adding motion here means *using the existing gated seam*, not introducing a new engine or an
ungated animation. That's the bar the ask set.

## Changes

- **`ui/src/features/shell/BrandHeader.tsx`** (new) — extracted the brand identity row out of
  `NavRail` (one-responsibility-per-file). The gradient brand tile now answers hover with a soft
  spring-lift + brightening wash and settles on press (`whileHover`/`whileTap`, a spring). Only the
  mark springs; the name/tagline stay put so text never jitters. Motion `off`/reduced-motion → the
  plain static tile with the same layout (no motion node, no transition), via the `enabled` gate.
  Replaces the old CSS `active:scale`/`hover:brightness` on the whole button.
- **`ui/src/features/shell/NavMenuMotion.tsx`** (new) — `NavMenuMotionItem`, a motion-gated staggered
  entrance for a single rail entry. Renders a `motion.li` carrying `SidebarMenuItem`'s exact markup
  (`data-sidebar="menu-item"` + `group/menu-item relative`) that fades + rises with a delay derived
  from its position (`index * step`, capped at 10 steps so long rails still settle fast). Off →
  the plain `SidebarMenuItem`.
  - Why not the shared `<Stagger>`/`<StaggerItem>`: those emit `<div>`s, which would break the
    `<ul>`/`<li>` list semantics and the sidebar's flex-column + `gap` layout. A per-item delay keeps
    the exact list markup with no motion node on the parent `<ul>`.
  - Why not `asChild`: `SidebarMenu`/`SidebarMenuItem` are bare `<ul>`/`<li>` (no Radix Slot — only
    `SidebarMenuButton` supports `asChild`), so `asChild` would leak as an invalid DOM prop. We mirror
    the `<li>` markup instead.
- **`ui/src/features/shell/NavRail.tsx`** — removed the inline `BrandMark` + header markup and the
  now-unused `Tooltip`/`Stagger` imports; renders `<BrandHeader/>` in the header slot. Threaded a
  per-group `index` through `item()` / `resolvedItem()` and every call site (fallback `SURFACE_GROUPS`,
  resolved menu + nested groups, pinned, ext slots) so each entry staggers by its position within its
  group. The dashboard-kind resolved entry now renders through `NavMenuMotionItem` too.

## Tests

- `npx tsc --noEmit` — clean for the touched files.
- `npx vitest run src/features/shell src/features/admin/nav/NavAdmin.gateway.test.tsx` → **20 passed**
  (incl. `NavRail.test.tsx`, 14). Structure unchanged: `NavRail → SidebarHeader → SidebarMenuItem →
  BrandHeader → Tooltip`.
- `npx vitest run src/lib/motion` → **6 passed** (motion-gate + useMotionPref).

No behavior regressions: the refactor preserves the collapse-toggle wiring, the `collapsible: "none"`
static-brand path, icon-collapsed centering/label-hide, pins, and cap-gated grouping. Motion is
additive and fully gated by the member's preference.

## Notes / follow-ups

- The stagger is a one-shot mount entrance (login / rail mount). It does not re-fire on every
  re-render because the entries keep stable React keys.
- If a future audit wants the brand tile's spring tuned, it's isolated in `BrandHeader.tsx`
  (`stiffness/damping/mass`).
