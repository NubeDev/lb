// Whether the viewer asked for reduced motion — gates the chart draw-in animation (product-register
// rule: every animation needs a reduced-motion alternative; here that's "no animation"). Mirrors the
// `use-mobile` matchMedia pattern so the whole app reads motion preference the same way.

import * as React from "react";

export function useReducedMotion(): boolean {
  const [reduced, setReduced] = React.useState(false);
  React.useEffect(() => {
    if (!window.matchMedia) return;
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setReduced(mq.matches);
    mq.addEventListener("change", update);
    update();
    return () => mq.removeEventListener("change", update);
  }, []);
  return reduced;
}
