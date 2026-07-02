// The design tokens — the look's single source of truth (look-scope.md §visual-language).
// Nothing outside theme/ hardcodes a color; the framework binds these to shell tokens.

export const tokens = {
  color: {
    canvas: "#0a0e14", // deep-space dark
    steel: "#3a4150", // equipment bodies (desaturated)
    grid: "#141a24", // ground grid, fades with zoom
    accent: "#22d3ee", // live data + selection ONLY (look-scope anti-goals)
    /** per-medium accents (look-scope open q → decided: per-medium, capped at 3) */
    medium: {
      air: "#22d3ee", // supply/return air — shares the accent family
      chw: "#60a5fa", // chilled water
      hw: "#fb923c", // hot water
    },
    duct: "#161d29", // duct body — slightly lighter than canvas (look-scope §recipe)
    text: {
      label: "#94a3b8", // secondary labels
      value: "#e2e8f0", // live values (tabular numerals)
    },
    status: {
      running: "#2dd4bf",
      stopped: "#4b5563",
      fault: "#f59e0b", // pulses toward #ef4444
      override: "#a78bfa",
    },
  },
  motion: {
    feedbackMs: 150,
    cameraSpringMs: 600,
    faultPulseHz: 0.5,
  },
  grid: {
    step: 8, // snap grid, world units
  },
} as const;
