// The motion seam barrel — the public entry for shell chrome animations. Consumers import motion
// primitives + the preference hook from here; `motion` (motion.dev) is imported ONLY in `motion.ts`.
export { motion, AnimatePresence, type Variants, type Transition } from "./motion";
export { useMotionPref, type MotionPref } from "./useMotionPref";
export { Reveal } from "./Reveal";
export { Stagger, StaggerItem } from "./Stagger";
export { Collapse } from "./Collapse";
