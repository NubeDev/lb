// Regression: `useMotionPref` (and so every motion-wrapped surface, e.g. <Reveal>) must render
// OUTSIDE a ThemeProvider — embedded/test mounts render pages bare, and the old throwing `useTheme`
// crashed the whole page ("useTheme must be used within ThemeProvider", see
// docs/debugging/frontend/reveal-crashes-outside-theme-provider.md). Falls back to the default
// preference via the optional context read.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { Reveal } from "./Reveal";
import { useMotionPref } from "./useMotionPref";

function Probe() {
  const pref = useMotionPref();
  return <span data-testid="motion">{pref.motion}</span>;
}

describe("useMotionPref outside ThemeProvider", () => {
  it("falls back to the default preference instead of throwing", () => {
    render(<Probe />);
    // The default look's motion resolves to a real level (never a crash); exact level depends on the
    // environment's reduced-motion fold, so assert it is one of the valid values.
    expect(["off", "subtle", "full"]).toContain(screen.getByTestId("motion").textContent);
  });

  it("<Reveal> renders its child without a provider", () => {
    render(<Reveal>hello</Reveal>);
    expect(screen.getByText("hello")).toBeInTheDocument();
  });
});
