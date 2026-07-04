// Design tokens for the shell — one dark theme, mint accent. The look is the smart-home reference
// set: near-black canvas, softly-raised cards, a single saturated mint used sparingly for the live/
// active state, hairline borders instead of drop shadows, generous radii. Everything downstream
// (components, screens) reads from here so a re-skin is a one-file change.

const palette = {
  ink: '#0C0E0D', // app canvas — near-black with a green undertone
  surface: '#161918', // raised card
  surfaceHi: '#1E2221', // hovered/pressed card, inputs
  line: 'rgba(255,255,255,0.07)', // hairline border (replaces shadows)
  mint: '#39DD9B', // the one accent — live/active/CTA
  mintDim: 'rgba(57,221,155,0.14)', // mint wash behind an active tile
  text: '#F3F6F4',
  textDim: '#9BA5A1',
  textFaint: '#5E6764',
  danger: '#FF6B6B',
} as const;

export const darkTheme = {
  colors: palette,
  radius: { sm: 12, md: 18, lg: 26, pill: 999 },
  space: (n: number) => n * 4, // 4pt grid — space(4) = 16
  font: {
    // Space Grotesk / General Sans read as the reference display face; Inter for body. Fall back to
    // the platform sans until the fonts are bundled (a follow-up asset step, not a blocker).
    display: 'System',
    body: 'System',
    weightBold: '700' as const,
    weightMed: '600' as const,
  },
} as const;

export type AppTheme = typeof darkTheme;
