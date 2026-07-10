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

import { useTheme } from "@/lib/theme";
import { Reveal } from "@/lib/motion";
import { AppPageHeader } from "./page-header";
import { HeaderBreadcrumbs } from "./header-breadcrumbs";
import { useEmbeddedPage } from "./page-embedded";

interface AppPageProps {
  /** aria-label for the section, e.g. "dashboard view". Also names the error region. */
  label: string;
  icon: LucideIcon;
  /** Optional accent colour for the header icon chip (any CSS colour) — used by surfaces that let the
   *  user theme a page (e.g. a dashboard's page settings). Omitted ⇒ the shell accent. */
  iconColor?: string;
  title: string;
  description?: string;
  workspace?: string;
  /** Header trailing controls (badges, actions) — rendered in the `AppPageHeader` actions slot. */
  actions?: ReactNode;
  /** Page-level error — rendered as the canonical destructive-token `role="alert"` strip. */
  error?: string | null;
  /** Mounted inside a dock pane: suppress the full-width header (the dock tab is the title bar).
   *  Defaults from the `EmbeddedPageContext` a pane host provides, so routed views need no prop. */
  embedded?: boolean;
  /** The body row: a roster `AppRail` + the main region (or an `AppEmptyState`). */
  children: ReactNode;
}

export function AppPage({
  label,
  icon,
  iconColor,
  title,
  description,
  workspace,
  actions,
  error,
  embedded,
  children,
}: AppPageProps) {
  const ctxEmbedded = useEmbeddedPage();
  const inPane = embedded ?? ctxEmbedded;
  // The header style is a member choice (Settings → Theme → Layout → Header). `band` (default) is
  // today's AppPageHeader icon-chip band; `breadcrumbs` renders the breadcrumb trail. Both carry the
  // same actions slot (workspace chip + Settings gear). Chosen here — the one place the header mounts.
  const { theme } = useTheme();
  const Header = theme.layout.header === "breadcrumbs" ? HeaderBreadcrumbs : AppPageHeader;
  return (
    <section aria-label={label} className="flex h-full min-w-0 flex-col bg-bg text-fg">
      {!inPane && (
        <Header
          icon={icon}
          iconColor={iconColor}
          title={title}
          description={description}
          workspace={workspace}
          actions={actions}
        />
      )}

      {error ? (
        <div
          role="alert"
          aria-label={`${label} error`}
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-sm text-destructive"
        >
          {error}
        </div>
      ) : null}

      {/* Page body eases in on navigation — a keyed Reveal re-fires the fade/slide when the surface (label)
          changes. Gated by the member's motion pref (off = static). Shell page-transition. */}
      <Reveal key={label} className="flex min-h-0 flex-1">
        {children}
      </Reveal>
    </section>
  );
}
