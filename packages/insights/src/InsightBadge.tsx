// Tone-tinted chips for a severity or a status — the package's OWN look (scoped `ins-badge` +
// `tone-*`). A host with its own design system ignores these and maps `severityTone`/`statusTone`
// onto its own Badge. One component per file (FILE-LAYOUT).

import type { JSX } from "react";
import type { Severity, Status } from "./types";
import { severityTone, statusTone } from "./model";

/** A severity chip ("CRITICAL" etc.), tinted by the tone key. */
export function SeverityBadge({ severity }: { severity: Severity }): JSX.Element {
  return <span className={`ins-badge tone-${severityTone(severity)}`}>{severity}</span>;
}

/** A status chip ("OPEN" / "ACKED" / "RESOLVED"), tinted by the tone key. */
export function StatusBadge({ status }: { status: Status }): JSX.Element {
  return <span className={`ins-badge tone-${statusTone(status)}`}>{status}</span>;
}
