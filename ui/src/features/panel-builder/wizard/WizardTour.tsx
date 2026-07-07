// The wizard's one-time concepts tour (panel-wizard scope, step 7). react-joyride, with ONE dismissible
// pass fired on first entry into OptionsStep. The tour carries the *naming* ("a threshold colors values
// above a limit"); the live preview carries the *showing*. If an option needs more than a one-line tour
// stop it is too complex for the simple track and belongs in an "Advanced" drawer (scope decision).
//
// Persistence: a `localStorage` flag keyed per-user so the tour fires ONCE, then never again — a heavy
// tour is its own overwhelm (scope Risks). Reset by clearing the flag (no UI for that in v1; the live
// preview is the primary teacher). One responsibility: the concepts tour.
//
// Co-located in the wizard folder — it is wizard-only, NOT a shared engine piece (it carries no
// per-option semantics; it names the surface, never the option).

import { useState } from "react";
import { Joyride, type Step, type Status, STATUS } from "react-joyride";

interface Props {
  /** The user id (for per-user dismissal persistence). Absent ⇒ the tour is disabled. */
  userId?: string;
}

/** The fixed steps — the surface's NAMING layer. The live preview IS the showing layer; this tour
 *  complements it, never duplicates it. */
const STEPS: Step[] = [
  {
    target: '[aria-label="wizard options step"]',
    title: "Edit an option, watch the preview",
    content:
      "Hover or edit any option and the preview on the right highlights the part of the panel it affects. An option marked 'renderer pending' honestly does nothing yet.",
  },
  {
    target: '[aria-label="panel preview"]',
    title: "One live preview",
    content:
      "This is the real panel, exactly as it will save — your option edits land here live, and the region the focused option affects is emphasized.",
  },
];

const DISMISS_KEY = "lb.panel-wizard.tour.dismissed";

/** Has this user already dismissed the tour? Reads once on mount. */
function isDismissed(userId?: string): boolean {
  if (!userId) return true;
  try {
    return localStorage.getItem(`${DISMISS_KEY}.${userId}`) === "1";
  } catch {
    return true;
  }
}

/** Persist dismissal for `userId`. */
function dismiss(userId?: string) {
  if (!userId) return;
  try {
    localStorage.setItem(`${DISMISS_KEY}.${userId}`, "1");
  } catch {
    // localStorage unavailable — accept the loss; the tour fires again next entry. Not load-bearing.
  }
}

/** The one-time concepts tour. Fires on first entry into OptionsStep; a dismissible pass; never again. */
export function WizardTour({ userId }: Props) {
  const [run] = useState(() => !isDismissed(userId));
  if (!run) return null;
  return (
    <Joyride
      steps={STEPS}
      continuous
      locale={{ close: "Got it", last: "Got it" }}
      onEvent={(data) => {
        // Dismiss on finish OR skip — either is "I've seen it, don't show again".
        const status = (data as { status: Status }).status;
        if (status === STATUS.FINISHED || status === STATUS.SKIPPED) dismiss(userId);
      }}
    />
  );
}
