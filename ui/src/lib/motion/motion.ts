// The ONE import site of `motion` (motion.dev) in the whole app — the single seam the scope mandates so
// the off switch is trustworthy and the engine tree-shakes to one place. Every springy shell animation
// (sheet slide, accordion height, nav-rail collapse, page fade/slide, staggered mounts) imports its
// primitives from HERE, never from `motion` directly. A lint/review rule keeps it that way; a repo grep
// for `from "motion"` outside this file is the check.
//
// Rejected: framer-motion (the same library's legacy React-only packaging); `motion` is its successor
// with a smaller hybrid engine.
//
// One responsibility: re-export the motion primitives the shell uses (and nothing more).

export { motion, AnimatePresence } from "motion/react";
export type { Variants, Transition } from "motion/react";
