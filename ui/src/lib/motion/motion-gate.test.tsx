// The motion seam's gate: every shell animation primitive (Reveal/Stagger/Collapse) must render its
// content STATICALLY when the effective motion level is `off` — no motion node, no inline animation —
// so the off switch (and `prefers-reduced-motion`) is trustworthy. We force `off` the belt-and-braces
// way the resolver honors it: stub `matchMedia` to report `prefers-reduced-motion: reduce`, which
// `resolveMotion` folds the default `subtle` level down to `off`. When motion is on we assert the
// content is still present (the animation itself is a real-browser concern the live-verify shots cover).
//
// One responsibility: prove the primitives honor the off switch and stay accessible when on.

import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ThemeProvider } from "@/lib/theme";
import { Collapse, Reveal, Stagger, StaggerItem } from "./index";

/** Force `prefers-reduced-motion: reduce` (or not) for the render. `resolveMotion` reads
 *  `document.defaultView.matchMedia`, so stub the jsdom `window.matchMedia`; it folds subtle→off. */
const realMatchMedia = window.matchMedia;
function setReducedMotion(reduce: boolean) {
  window.matchMedia = ((q: string) =>
    ({
      matches: reduce && q.includes("prefers-reduced-motion"),
      media: q,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      onchange: null,
      dispatchEvent: () => false,
    }) as unknown as MediaQueryList) as typeof window.matchMedia;
}

afterEach(() => {
  window.matchMedia = realMatchMedia;
  vi.unstubAllGlobals();
});

const wrap = (ui: React.ReactNode) => render(<ThemeProvider>{ui}</ThemeProvider>);

describe("motion primitives honor the off switch", () => {
  it("Reveal renders a PLAIN div (no inline motion style) under reduced motion", () => {
    setReducedMotion(true);
    wrap(<Reveal><span>content</span></Reveal>);
    const el = screen.getByText("content").parentElement!;
    expect(el.getAttribute("style") ?? "").not.toMatch(/opacity|transform/);
    expect(screen.getByText("content")).toBeInTheDocument();
  });

  it("Stagger + StaggerItem still render every child under reduced motion", () => {
    setReducedMotion(true);
    wrap(
      <Stagger aria-label="list">
        <StaggerItem><span>a</span></StaggerItem>
        <StaggerItem><span>b</span></StaggerItem>
      </Stagger>,
    );
    expect(screen.getByText("a")).toBeInTheDocument();
    expect(screen.getByText("b")).toBeInTheDocument();
  });

  it("Collapse shows content when open and hides it when closed (reduced motion = instant)", () => {
    setReducedMotion(true);
    const { rerender } = wrap(<Collapse open><span>panel</span></Collapse>);
    expect(screen.getByText("panel")).toBeInTheDocument();
    rerender(<ThemeProvider><Collapse open={false}><span>panel</span></Collapse></ThemeProvider>);
    expect(screen.queryByText("panel")).not.toBeInTheDocument();
  });

  it("content is present when motion is on (no reduced-motion, default subtle)", () => {
    setReducedMotion(false);
    wrap(<Reveal><span>shown</span></Reveal>);
    expect(screen.getByText("shown")).toBeInTheDocument();
  });
});
