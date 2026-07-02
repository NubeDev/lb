import { readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import type { Plugin } from 'vite'

// A Vite lib-build plugin that SCOPES the emitted `ce-wiresheet.css` under `.ce-wiresheet`,
// closing the slice-9 federated-CSS leaks that survive the source-level fix. The editor is
// a LIBRARY injected into a HOST document (the LB shell), so its stylesheet must not write
// anything global. Two leak shapes remain after `wiresheet.css`/`wiresheet-theme.css` are
// scoped at source, both from code we do NOT own the CSS of:
//
//   1. Tailwind v4's `@theme` emits its design tokens onto `:root,:host{…}` (--color-*,
//      --spacing, --radius-md, --font-*, …). Injected into the host, these overwrite the
//      shell's OWN `:root` tokens of the same name (verified: --radius-md, --font-sans differ).
//   2. `@xyflow/react/dist/style.css` (imported by CeEditor) ships ~150 BARE `.react-flow*`
//      rules (some nested in `@media` blocks). The host renders React Flow in its system/
//      data/flows views with its OWN `.react-flow` theming — the vendored copy collides.
//
// The fix rewrites those two shapes to be `.ce-wiresheet`-scoped in the FINAL bundle (the
// bytes that ship), leaving safe/necessary globals alone: `@property --tw-*`, `@keyframes`
// STEPS (`0%`/`to` — must NOT be scoped), and the `*,:before,:after,::backdrop{--tw-*}`
// polyfill (custom-property fallbacks — no reset, inert on the host).
//
// Why a hand tokenizer and not PostCSS: postcss isn't resolvable from this package (vite
// bundles its own copy, not re-exported). We walk brace depth AND track the enclosing
// at-rule so we scope rule selectors (incl. those nested in `@media`/`@supports`/`@layer`)
// but never keyframe steps or at-rule preludes. The slice-9 regression guard (`preflight-
// audit` + `wiresheet-scope` tests) fails the build if a NEW global leak shape ever ships.
// NOTE: with `@tailwindcss/vite` + `cssCodeSplit:false`, the CSS is NOT a rollup asset in
// `generateBundle` (it's written by Tailwind's own emit), so we transform it on DISK in
// `writeBundle` (runs once per format after files land). Idempotent: re-scoping an
// already-scoped file is a no-op (each rewrite skips selectors already containing the scope),
// so the second (CJS) pass over the same `ce-wiresheet.css` doesn't double-prefix.
export function scopeWiresheetCss(scope = '.ce-wiresheet'): Plugin {
  return {
    name: 'scope-wiresheet-css',
    apply: 'build',
    writeBundle(opts) {
      // The CSS isn't a rollup-tracked asset here, so target the known emitted file directly
      // (`assetFileNames: 'ce-wiresheet.[ext]'`). Idempotent, so multiple output passes are safe.
      const abs = path.resolve(opts.dir ?? 'dist', 'ce-wiresheet.css')
      try {
        writeFileSync(abs, scopeCss(readFileSync(abs, 'utf8'), scope))
      } catch (e) {
        if ((e as NodeJS.ErrnoException).code !== 'ENOENT') throw e // no CSS in a JS-only pass
      }
    },
  }
}

// At-rules whose direct children are STYLE RULES (their selectors must be scoped), as opposed
// to `@keyframes` (children are steps like `0%`/`to`) or declaration-only at-rules (`@theme`,
// `@property`, `@font-face` — their `{…}` is declarations, no nested selectors).
const CONDITIONAL_GROUP = /^@(media|supports|layer|container|scope)\b/i

export function scopeCss(css: string, scope: string): string {
  let out = ''
  let seg = '' // chars since the last `{`/`}`/`;`
  // Stack of `true` when the enclosing block CONTAINS selectors that should be scoped
  // (top level, or inside a conditional-group at-rule), `false` when it does NOT
  // (a declaration body, or a `@keyframes`/`@property`/`@theme`/`@font-face` block).
  const scopable: boolean[] = []

  const inSelectorContext = () => scopable.length === 0 || scopable[scopable.length - 1]

  for (let i = 0; i < css.length; i++) {
    const ch = css[i]
    if (ch === '{') {
      const prelude = seg
      const isAt = prelude.trimStart().startsWith('@')
      if (inSelectorContext() && !isAt) {
        // `prelude` is a selector list of a style rule → rewrite it. Its body is declarations.
        out += rewriteSelectorList(prelude, scope) + '{'
        scopable.push(false)
      } else {
        // an at-rule (keep prelude as-is), OR a nested block inside a declaration context.
        out += prelude + '{'
        // Only a conditional-group at-rule opens another selector context; everything else
        // (keyframes/property/theme/font-face, or a declaration block) does not.
        scopable.push(isAt ? CONDITIONAL_GROUP.test(prelude.trimStart()) : false)
      }
      seg = ''
    } else if (ch === '}') {
      out += seg + '}'
      seg = ''
      scopable.pop()
    } else if (ch === ';' && scopable.length === 0) {
      out += seg + ';' // top-level `@charset`/`@import` statement
      seg = ''
    } else {
      seg += ch
    }
  }
  return out + seg
}

function rewriteSelectorList(list: string, scope: string): string {
  const rewritten = list.split(',').map((sel) => rewriteSelector(sel, scope))
  // Dedupe exact repeats — `:root,:host` both map to the scope, which would emit
  // `.ce-wiresheet,.ce-wiresheet`. Compare trimmed; keep first occurrence's formatting.
  const seen = new Set<string>()
  const kept = rewritten.filter((sel) => {
    const key = sel.trim()
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
  return kept.join(',')
}

function rewriteSelector(sel: string, scope: string): string {
  const trimmed = sel.trim()
  if (!trimmed) return sel
  if (trimmed.includes(scope)) return sel // already scoped — avoid `.ce-wiresheet .ce-wiresheet …`

  // `:root`/`:host` (the Tailwind @theme token block + any stray) → the scope itself.
  if (/^:root$|^:host$/.test(trimmed)) {
    const lead = leadingWs(sel)
    return `${lead}${scope}`
  }

  // Any selector referencing `.react-flow` (leading, element-prefixed `svg.react-flow…`, or
  // descendant) → prefix the whole selector with the scope so it only matches in the subtree.
  if (trimmed.includes('.react-flow')) {
    return `${leadingWs(sel)}${scope} ${trimmed}`
  }

  return sel
}

const leadingWs = (s: string) => s.slice(0, s.length - s.trimStart().length)
