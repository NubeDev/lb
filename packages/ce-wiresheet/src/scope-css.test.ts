import { describe, expect, it } from 'vitest'
import { scopeCss } from '../scope-css'

// The lib build's CSS-scoping transform (slice-9.1). It rewrites the FINAL emitted stylesheet so the
// injected library writes nothing global into a host document: `:root`/`:host` token blocks → the
// `.ce-wiresheet` scope, and bare `.react-flow*` rules (incl. those nested in `@media`) → scoped. It must
// NOT touch keyframe steps, `@property`/`@theme`/`@font-face` at-rules, the `*,:before` polyfill, or
// already-scoped rules. See scope-css.ts and the debugging entry.
const S = '.ce-wiresheet'

describe('scopeCss', () => {
  it('rewrites :root/:host token blocks to the scope (deduped, not `.ce-wiresheet,.ce-wiresheet`)', () => {
    expect(scopeCss(':root,:host{--color-card:hsl(var(--card));--spacing:.25rem}', S)).toBe(
      '.ce-wiresheet{--color-card:hsl(var(--card));--spacing:.25rem}',
    )
    expect(scopeCss(':root{--x:1}', S)).toBe('.ce-wiresheet{--x:1}')
    expect(scopeCss(':host,:root{--y:2}', S)).toBe('.ce-wiresheet{--y:2}')
  })

  it('scopes bare .react-flow rules — leading, element-prefixed, and descendant', () => {
    expect(scopeCss('.react-flow{direction:ltr}', S)).toBe('.ce-wiresheet .react-flow{direction:ltr}')
    expect(scopeCss('.react-flow.dark{background:#000}', S)).toBe(
      '.ce-wiresheet .react-flow.dark{background:#000}',
    )
    expect(scopeCss('svg.react-flow__connectionline{z-index:1}', S)).toBe(
      '.ce-wiresheet svg.react-flow__connectionline{z-index:1}',
    )
    expect(scopeCss('.react-flow .react-flow__edges{pointer-events:none}', S)).toBe(
      '.ce-wiresheet .react-flow .react-flow__edges{pointer-events:none}',
    )
    expect(scopeCss('.react-flow__edge.selected .react-flow__edge-path{stroke:red}', S)).toBe(
      '.ce-wiresheet .react-flow__edge.selected .react-flow__edge-path{stroke:red}',
    )
  })

  it('scopes .react-flow rules NESTED inside @media/@supports/@layer', () => {
    expect(scopeCss('@media (min-width:40rem){.react-flow__panel{top:0}}', S)).toBe(
      '@media (min-width:40rem){.ce-wiresheet .react-flow__panel{top:0}}',
    )
    expect(scopeCss('@layer base{:root{--y:2}.react-flow{x:1}}', S)).toBe(
      '@layer base{.ce-wiresheet{--y:2}.ce-wiresheet .react-flow{x:1}}',
    )
  })

  it('leaves keyframe STEPS, @property, and the *,:before polyfill untouched', () => {
    expect(scopeCss('@keyframes dashdraw{0%{stroke-dashoffset:10}to{opacity:1}}', S)).toBe(
      '@keyframes dashdraw{0%{stroke-dashoffset:10}to{opacity:1}}',
    )
    expect(scopeCss('@property --tw-blur{syntax:"*";inherits:false}', S)).toBe(
      '@property --tw-blur{syntax:"*";inherits:false}',
    )
    expect(scopeCss('@supports (x:1){*,:before,:after,::backdrop{--tw-x:0}}', S)).toBe(
      '@supports (x:1){*,:before,:after,::backdrop{--tw-x:0}}',
    )
  })

  it('is idempotent — an already-scoped rule is left alone (no double prefix)', () => {
    const once = scopeCss('.react-flow{x:1}', S)
    expect(scopeCss(once, S)).toBe(once)
    expect(scopeCss('.ce-wiresheet .flex{display:flex}', S)).toBe('.ce-wiresheet .flex{display:flex}')
    expect(scopeCss('.ce-page .flex{display:flex}', S)).toBe('.ce-page .flex{display:flex}')
  })

  it('preserves brace balance on a mixed document', () => {
    const doc =
      ':root{--a:1}.react-flow{b:2}@media(x){.react-flow__c{d:3}}@keyframes k{0%{e:4}}.ce-wiresheet .flex{f:5}'
    const out = scopeCss(doc, S)
    let depth = 0
    for (const ch of out) {
      if (ch === '{') depth++
      if (ch === '}') depth--
    }
    expect(depth).toBe(0)
  })
})
