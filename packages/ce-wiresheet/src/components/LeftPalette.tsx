// Left-drawer component palette for the ported wiresheet — styled with rbx's
// Tailwind tokens to match the existing rbx port's Palette. Behaviour: DOUBLE-CLICK
// or DRAG a type onto the canvas to add it; ↑/↓ navigate (auto-expanding the
// selected item's section) and Enter adds; Esc closes. Sections collapsed by
// default. Services are filtered out upstream (where `palette` is built).

import { useEffect, useMemo, useRef, useState } from 'react'
import { Package, Search, ChevronRight, Plus, X } from 'lucide-react'

const DND_TYPE = 'application/x-ce-component-type'

interface PalComp { name: string; type: string }
interface PalExt { id: string; components: PalComp[] }

export function LeftPalette({
  open,
  extensions,
  onAdd,
  onClose,
}: {
  open: boolean
  extensions: PalExt[]
  onAdd: (type: string) => void
  onClose: () => void
}) {
  const [q, setQ] = useState('')
  const filtered = useMemo(() => filterExts(extensions, q), [extensions, q])
  // Flat list of all visible types (across sections) for ↑/↓ + Enter; index is
  // data-derived (type → position) so it stays consistent regardless of which
  // sections are expanded.
  const flat = useMemo(() => filtered.flatMap((g) => g.components), [filtered])
  const flatIndex = useMemo(() => new Map(flat.map((c, i) => [c.type, i])), [flat])
  const [sel, setSel] = useState(0)
  const [openIds, setOpenIds] = useState<Set<string>>(new Set()) // expanded sections (collapsed by default)

  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  useEffect(() => { if (open) inputRef.current?.focus() }, [open])
  useEffect(() => { setSel((s) => Math.min(s, Math.max(0, flat.length - 1))); }, [flat.length])
  useEffect(() => {
    listRef.current?.querySelector<HTMLElement>(`[data-pal-idx="${sel}"]`)?.scrollIntoView({ block: 'nearest' })
  }, [sel])

  // Keep the selected row's section expanded so the highlight is visible.
  const selType = flat[sel]?.type
  const selExtId = selType ? filtered.find((g) => g.components.some((c) => c.type === selType))?.id : undefined

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') { e.preventDefault(); onClose(); return }
    if (flat.length === 0) return
    if (e.key === 'ArrowDown') { e.preventDefault(); setSel((s) => Math.min(flat.length - 1, s + 1)); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); setSel((s) => Math.max(0, s - 1)); }
    else if (e.key === 'Enter') { e.preventDefault(); const c = flat[sel]; if (c) onAdd(c.type); }
  }
  const toggle = (id: string) =>
    setOpenIds((s) => { const n = new Set(s); if (n.has(id)) n.delete(id); else n.add(id); return n })

  return (
    <div className="flex h-full w-60 shrink-0 flex-col border-r border-border bg-card shadow-xl" onKeyDown={onKeyDown}>
      <div className="border-b border-border p-2.5">
        <div className="mb-2 flex items-center gap-1.5 text-[12px] font-semibold text-foreground">
          <Package size={14} className="text-r1" /> Palette
          <button
            type="button"
            onClick={onClose}
            title="Close palette (Esc)"
            className="ml-auto rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
          >
            <X size={14} />
          </button>
        </div>
        <div className="relative">
          <Search size={13} className="absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground" />
          {/* Self-contained search input (was rbx's shadcn <Input> — inlined to drop
              the @/ coupling). */}
          <input
            ref={inputRef}
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search types…"
            className="flex w-full rounded-md border border-input bg-transparent px-3 py-1 text-foreground shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 h-8 pl-7 text-[12px]"
          />
        </div>
      </div>

      <div ref={listRef} className="flex-1 overflow-y-auto p-1.5">
        {filtered.length === 0 ? (
          <p className="px-2 py-6 text-center text-[11.5px] text-muted-foreground">
            {extensions.length === 0 ? 'No component types yet.' : `Nothing matches “${q}”.`}
          </p>
        ) : (
          filtered.map((g) => {
            const expanded = q.trim() !== '' || openIds.has(g.id) || g.id === selExtId
            return (
              <div key={g.id} className="mb-1">
                <button
                  type="button"
                  onClick={() => toggle(g.id)}
                  className="flex w-full items-center gap-1.5 rounded-md px-2 py-1.5 text-left hover:bg-muted"
                >
                  <ChevronRight
                    size={13}
                    className={`shrink-0 text-muted-foreground transition-transform ${expanded ? 'rotate-90' : ''}`}
                  />
                  <span className="flex-1 truncate text-[11.5px] font-semibold text-foreground">{g.id}</span>
                  <span className="text-[10px] text-muted-foreground">{g.components.length}</span>
                </button>
                {expanded && (
                  <div className="ml-2 border-l border-border pl-1.5">
                    {g.components.map((c) => {
                      const i = flatIndex.get(c.type) ?? -1
                      return (
                        <Row key={c.type} comp={c} onAdd={onAdd} index={i} selected={i === sel} onHover={() => setSel(i)} />
                      )
                    })}
                  </div>
                )}
              </div>
            )
          })
        )}
      </div>
    </div>
  )
}

function Row({
  comp,
  onAdd,
  index,
  selected,
  onHover,
}: {
  comp: PalComp
  onAdd: (t: string) => void
  index: number
  selected: boolean
  onHover: () => void
}) {
  return (
    <div
      data-pal-idx={index}
      draggable
      onDragStart={(e) => {
        e.dataTransfer.effectAllowed = 'copy'
        e.dataTransfer.setData(DND_TYPE, comp.type)
      }}
      onDoubleClick={() => onAdd(comp.type)}
      onMouseEnter={onHover}
      title={`${comp.type} — double-click or Enter to add, or drag onto the canvas`}
      className={`group flex w-full cursor-grab items-center justify-between gap-2 rounded-md px-2 py-1.5 text-left active:cursor-grabbing ${
        selected ? 'bg-muted' : 'hover:bg-muted'
      }`}
    >
      <span className="min-w-0 flex-1 truncate text-[12px] text-foreground">{comp.name}</span>
      <Plus size={13} className={`shrink-0 text-muted-foreground ${selected ? '' : 'opacity-0 group-hover:opacity-100'}`} />
    </div>
  )
}

function filterExts(exts: PalExt[], q: string): PalExt[] {
  const n = q.trim().toLowerCase()
  if (!n) return exts
  const out: PalExt[] = []
  for (const g of exts) {
    if (g.id.toLowerCase().includes(n)) {
      out.push(g)
      continue
    }
    const components = g.components.filter(
      (c) => c.name.toLowerCase().includes(n) || c.type.toLowerCase().includes(n),
    )
    if (components.length) out.push({ ...g, components })
  }
  return out
}
