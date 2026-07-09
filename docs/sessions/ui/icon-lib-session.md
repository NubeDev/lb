# Session: reusable icon lib (`ui/src/lib/icons`)

## Ask
A developer-facing icon library: a name→icon resolver, plus a searchable picker with a
nice UX and an adjustable page size (default 10).

## What shipped
`ui/src/lib/icons/` (one responsibility per file, FILE-LAYOUT):
- `resolve.ts` — `resolveIcon(name)` / `isIconName(name)`. Maps a stable string name to a
  lucide-react component via lucide's exported `icons` record. Accepts kebab-case
  (`git-branch`), the PascalCase key (`GitBranch`), or the `Lucide`-prefixed alias.
  Returns `null` for unknown names (caller renders a fallback).
- `catalog.ts` — `ICON_CATALOG` (all ~1500 lucide glyphs as kebab-case entries, deduped
  + sorted, derived from lucide keys so it never rots) and `searchIcons(query, limit)`
  with ranked matching (exact > name-prefix > token-prefix > substring).
- `Icon.tsx` — `<Icon name="git-branch" fallback="box" className="size-4" />`, the read
  side. Forwards lucide props; `name` is `Omit`ted from `LucideProps` to avoid the clash.
- `IconPicker.tsx` — search box + paged grid. `pageSize` (default 10) reveals a page at a
  time via "Show more"; `columns` (default 5) adjustable. Bound to Lazybones tokens
  (`fg`/`panel-2`/`accent`, `ring-accent` focus) — matches `combobox.tsx`/`dialog.tsx`.
- `index.ts` — barrel.

## Why lucide, not a new dep
`lucide-react` is already the repo's icon set (20+ files import it). The lib turns it into
a *named, searchable, storable* resource so features/extensions can persist an icon as
opaque data — no new dependency, no per-icon static imports (uses the `icons` record).

## Tests (rule 9 — real lucide, no mock)
`icons.test.ts` — 7 tests, green: resolver (kebab/Pascal/unknown/empty), catalog invariants
(size > 1000, deduped, sorted), search ranking + limit, round-trip resolvability.
`pnpm test src/lib/icons/icons.test.ts` → 7 passed. `tsc --noEmit` clean for the new files.

## Usage
```tsx
import { Icon, IconPicker, resolveIcon } from "@/lib/icons";
<Icon name={cell.icon} fallback="box" className="size-4" />
<IconPicker value={icon} onSelect={setIcon} pageSize={10} />
```

## Not done / open
No Dialog/Popover wrapper — the picker is a body; call sites supply the trigger. Search is
name-token based (no curated synonym aliases yet); lucide's multi-name export aliases could
feed richer search tokens later if needed.
