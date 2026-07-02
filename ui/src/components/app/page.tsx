// AppPage — the canonical full-screen surface shell (ui-standards-scope / shadcn-migration-scope).
// It fixes the ONE shape every page must share, so surfaces can't drift out of alignment: a
// `flex h-full min-w-0 flex-col` section, a FULL-WIDTH `AppPageHeader` at the top, an optional
// inline error strip, then the `min-h-0 flex-1` body row (a roster `AppRail` + the main region).
//
// This is why Flows reads right and the older pages didn't: the header spans the whole width and the
// rail sits BELOW it — not a full-height rail beside a body-only header. Pages that adopt `AppPage`
// get that structure for free. One component per file (FILE-LAYOUT).

import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

import { AppPageHeader } from "./page-header";

interface AppPageProps {
  /** aria-label for the section, e.g. "dashboard view". Also names the error region. */
  label: string;
  icon: LucideIcon;
  title: string;
  description?: string;
  workspace?: string;
  /** Header trailing controls (badges, actions) — rendered in the `AppPageHeader` actions slot. */
  actions?: ReactNode;
  /** Page-level error — rendered as the canonical destructive-token `role="alert"` strip. */
  error?: string | null;
  /** The body row: a roster `AppRail` + the main region (or an `AppEmptyState`). */
  children: ReactNode;
}

export function AppPage({
  label,
  icon,
  title,
  description,
  workspace,
  actions,
  error,
  children,
}: AppPageProps) {
  return (
    <section aria-label={label} className="flex h-full min-w-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={icon}
        title={title}
        description={description}
        workspace={workspace}
        actions={actions}
      />

      {error ? (
        <div
          role="alert"
          aria-label={`${label} error`}
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-sm text-destructive"
        >
          {error}
        </div>
      ) : null}

      <div className="flex min-h-0 flex-1">{children}</div>
    </section>
  );
}
