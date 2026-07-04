// A height collapse/expand — animates content in and out on `open` change, gated by the member's motion
// preference. `off` renders the child instantly (mounted when open, gone when closed — no animation);
// `subtle`/`full` animate height + opacity with the scaled duration. Used by the accordion (Brand colors)
// so a disclosure eases open instead of popping. Motion primitives come from the ONE seam (`@/lib/motion`).
//
// One responsibility: a motion-gated height collapse.

import type { ReactNode } from "react";

import { AnimatePresence, motion } from "./motion";
import { useMotionPref } from "./useMotionPref";

interface Props {
  open: boolean;
  children: ReactNode;
  className?: string;
}

export function Collapse({ open, children, className }: Props) {
  const { enabled, duration } = useMotionPref();

  // Motion off / reduced: no animation — present when open, absent when closed.
  if (!enabled) return open ? <div className={className}>{children}</div> : null;

  return (
    <AnimatePresence initial={false}>
      {open && (
        <motion.div
          className={className}
          style={{ overflow: "hidden" }}
          initial={{ height: 0, opacity: 0 }}
          animate={{ height: "auto", opacity: 1 }}
          exit={{ height: 0, opacity: 0 }}
          transition={{ duration: duration(0.26), ease: [0.22, 1, 0.36, 1] }}
        >
          {children}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
