// A one-shot reveal — fade + slight upward slide on mount — gated by the member's motion preference.
// `off` renders the child statically (no motion node, no transition); `subtle` uses a short, small-
// distance variant; `full` the designed distance/duration. This is the shell's page/tab/panel entrance
// primitive: page mounts and tab-panel swaps wrap their content in <Reveal>. Motion primitives come from
// the ONE seam (`@/lib/motion`), never `motion` directly (bundle-discipline guard).
//
// One responsibility: a motion-gated mount reveal.

import type { ReactNode } from "react";

import { motion } from "./motion";
import { useMotionPref } from "./useMotionPref";

interface Props {
  children: ReactNode;
  className?: string;
  /** Base upward travel in px at `full` (scaled down for `subtle`, 0 for `off`). */
  travel?: number;
  /** Base duration in seconds at `full`. */
  duration?: number;
}

export function Reveal({ children, className, travel = 12, duration: base = 0.28 }: Props) {
  const { enabled, duration, distance } = useMotionPref();
  // Motion off (or reduced-motion): render the plain node so there is no transition at all.
  if (!enabled) return <div className={className}>{children}</div>;
  const y = distance(travel);
  return (
    <motion.div
      className={className}
      initial={{ opacity: 0, y }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: duration(base), ease: [0.22, 1, 0.36, 1] }}
    >
      {children}
    </motion.div>
  );
}
