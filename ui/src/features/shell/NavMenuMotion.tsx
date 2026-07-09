// The rail's menu-item motion wrapper — a one-shot staggered entrance for the sidebar nav items so
// the rail *assembles* on mount/login instead of snapping in whole. Each entry wraps its
// `SidebarMenuButton` in `NavMenuMotionItem`, which renders the shadcn `SidebarMenuItem` (`<li>`) as
// a `motion.li` that fades + rises with a delay derived from its position — a self-contained stagger
// that needs no motion node on the parent `<ul>`, so the exact `SidebarMenu` list markup, its flex
// layout, and its `data-sidebar` attributes are untouched.
//
// Why not the shared `<Stagger>`: it emits `<div>`s, which would break the `<ul>`/`<li>` semantics
// and the sidebar's flex-column + gap layout. Motion primitives come from the ONE seam (`@/lib/motion`),
// gated by the member's preference: off / reduced-motion renders the plain `SidebarMenuItem` — no
// motion node, no transition.
//
// One responsibility: a motion-gated staggered entrance for one rail menu entry.

import type { ReactNode } from "react";

import { motion, useMotionPref } from "@/lib/motion";
import { SidebarMenuItem } from "@/components/ui/sidebar";
import { cn } from "@/lib/utils";

interface ItemProps {
  children: ReactNode;
  className?: string;
  /** Zero-based position in its group — drives the entrance delay (capped so long lists still settle
   *  quickly). */
  index: number;
  /** Upward travel (px) at `full` (scaled for `subtle`, 0 for `off`). */
  travel?: number;
  /** Per-item delay step (s) at `full` (halved for `subtle`). */
  step?: number;
}

/** The sliding active-selection pill — a shared-element indicator that physically GLIDES from the
 *  previously-selected rail item to the newly-selected one on every navigation (motion `layoutId`),
 *  instead of the accent fill snapping on/off per button. Rendered absolutely behind the button's
 *  content (the button drops its own active background; this pill is the fill). One pill is mounted at
 *  a time — only under the active item — and because every active item shares the same `layoutId`,
 *  motion animates the single element between their positions.
 *
 *  Motion off / reduced-motion: render a plain static pill (same look, no layout animation) so the
 *  active item still reads as filled. */
export function NavActivePill() {
  const { enabled, duration } = useMotionPref();
  const className =
    "pointer-events-none absolute inset-0 rounded-md bg-accent/10 shadow-[inset_0_0_0_1px_hsl(var(--accent)/0.22)]";
  if (!enabled) return <span aria-hidden className={className} />;
  return (
    <motion.span
      aria-hidden
      layoutId="nav-active-pill"
      className={className}
      transition={{ type: "spring", stiffness: 380, damping: 32, mass: 1, duration: duration(0.42) }}
    />
  );
}

/** One rail entry. Renders the plain `SidebarMenuItem` when motion is off; else a `motion.li` carrying
 *  the primitive's exact markup (`data-sidebar` + `group/menu-item relative`), fading + rising into
 *  place after `index * step`, capped at 10 steps so a long rail doesn't crawl. `SidebarMenuItem` is a
 *  bare `<li>` (no Radix Slot / `asChild`), so we mirror its markup rather than wrap it. */
export function NavMenuMotionItem({ children, className, index, travel = 8, step = 0.035 }: ItemProps) {
  const { motion: level, enabled, duration, distance } = useMotionPref();
  if (!enabled) return <SidebarMenuItem className={className}>{children}</SidebarMenuItem>;
  const y = distance(travel);
  const stepScaled = level === "subtle" ? step * 0.5 : step;
  const delay = Math.min(index, 10) * stepScaled;
  return (
    <motion.li
      data-sidebar="menu-item"
      className={cn("group/menu-item relative", className)}
      initial={{ opacity: 0, y }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: duration(0.24), ease: [0.22, 1, 0.36, 1], delay }}
    >
      {children}
    </motion.li>
  );
}
