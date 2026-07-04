// A staggered-list mount — children fade+rise in sequence — gated by the member's motion preference.
// `off` renders a plain wrapper (children appear at once, no motion nodes); `subtle`/`full` stagger the
// items with a per-child delay and the scaled distance/duration. Wrap a list in <Stagger> and each direct
// item in <StaggerItem>. Motion primitives come from the ONE seam (`@/lib/motion`).
//
// One responsibility: a motion-gated staggered mount for a list.

import type { ReactNode } from "react";

import { motion } from "./motion";
import { useMotionPref } from "./useMotionPref";

interface StaggerProps {
  children: ReactNode;
  className?: string;
  role?: string;
  "aria-label"?: string;
  /** Base per-child stagger step in seconds at `full` (halved for `subtle`). */
  step?: number;
}

export function Stagger({ children, className, step = 0.05, role, "aria-label": ariaLabel }: StaggerProps) {
  const { motion: level, enabled } = useMotionPref();
  if (!enabled) return <div className={className} role={role} aria-label={ariaLabel}>{children}</div>;
  const staggerChildren = level === "subtle" ? step * 0.5 : step;
  return (
    <motion.div
      className={className}
      role={role}
      aria-label={ariaLabel}
      initial="hidden"
      animate="show"
      variants={{ show: { transition: { staggerChildren } } }}
    >
      {children}
    </motion.div>
  );
}

interface ItemProps {
  children: ReactNode;
  className?: string;
  travel?: number;
}

export function StaggerItem({ children, className, travel = 10 }: ItemProps) {
  const { enabled, duration, distance } = useMotionPref();
  if (!enabled) return <div className={className}>{children}</div>;
  const y = distance(travel);
  return (
    <motion.div
      className={className}
      variants={{
        hidden: { opacity: 0, y },
        show: { opacity: 1, y: 0, transition: { duration: duration(0.26), ease: [0.22, 1, 0.36, 1] } },
      }}
    >
      {children}
    </motion.div>
  );
}
