// The sidebar's identity row — the workspace brand mark + name/tagline that sits at the top of the
// rail and DOUBLES as the collapse toggle. Extracted from NavRail so the interaction lives in one
// file (FILE-LAYOUT) and the motion stays gated behind the ONE seam (`@/lib/motion`), never `motion`
// directly.
//
// The touch the brand box earns: it's the anchor of the shell, the one element a member's eye lands
// on first, so a press should feel physical. Motion off / reduced-motion falls back to the plain
// static markup with the same layout — no motion node, no transition (the `enabled` gate). The
// gradient tile answers a hover with a soft spring-lift and a brightening wash; a press settles it.
//
// One responsibility: render (and animate) the brand identity row.

import { motion, useMotionPref } from "@/lib/motion";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { Branding } from "@/lib/branding";

/** The brand mark — the workspace's identity glyph. Renders the admin's **logo** image when set, else
 *  the **icon** image, else the text `siteAbbr` tile (the historical "lb"). Same gradient tile as the
 *  compiled default so the fallback chain stays coherent; `--accent-foreground` keeps the glyph legible
 *  on the accent in every preset/mode. */
function BrandMark({
  siteAbbr,
  logoDataUri,
  iconDataUri,
}: {
  siteAbbr: string;
  logoDataUri?: string;
  iconDataUri?: string;
}) {
  if (logoDataUri || iconDataUri) {
    return (
      <img
        src={logoDataUri ?? iconDataUri}
        alt=""
        aria-hidden="true"
        className="h-8 w-8 shrink-0 rounded-lg object-contain"
      />
    );
  }
  return (
    <div
      className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-[11px] font-bold shadow-sm"
      style={{
        background: "linear-gradient(135deg, hsl(var(--accent)), hsl(var(--accent-2)))",
        color: "hsl(var(--accent-foreground))",
      }}
    >
      {siteAbbr}
    </div>
  );
}

/** The name + tagline block. Hidden in icon-collapsed mode (the parent's `group-data-[collapsible=icon]`
 *  fence). Kept as its own node so both the interactive and static header render it identically. */
function BrandLabel({ siteName, tagline }: { siteName: string; tagline?: string }) {
  return (
    <div className="grid flex-1 text-left text-sm leading-tight group-data-[collapsible=icon]:hidden">
      <span className="truncate font-semibold tracking-tight">{siteName}</span>
      {tagline && <span className="truncate text-xs text-muted">{tagline}</span>}
    </div>
  );
}

interface Props {
  brand: Branding;
  /** True when the collapsible mode allows toggling — the row becomes an interactive collapse control.
   *  In `none` mode it renders as a plain static brand (no pointer, hover, tooltip, or toggle). */
  canToggle: boolean;
  /** Toggle expand/collapse (the sidebar provider's `toggleSidebar`). */
  onToggle: () => void;
  /** Accessible label + tooltip copy for the current direction ("Collapse sidebar" / "Expand sidebar"). */
  toggleLabel: string;
}

export function BrandHeader({ brand, canToggle, onToggle, toggleLabel }: Props) {
  const { enabled } = useMotionPref();

  const inner = (
    <>
      {/* The tile lifts and brightens on hover, settles on press — a spring so it reads as a physical
          surface, not a CSS step. Only the mark springs; the label stays put so text doesn't jitter.
          Motion off ⇒ `enabled` is false and we render the tile with no wrapper (static). */}
      {enabled ? (
        <motion.span
          className="relative shrink-0"
          initial={false}
          whileHover={{ scale: 1.1, rotate: -3, filter: "brightness(1.18)" }}
          whileTap={{ scale: 0.92, rotate: 0 }}
          transition={{ type: "spring", stiffness: 480, damping: 18, mass: 0.6 }}
        >
          {/* An accent glow that blooms on hover — the tile reads as lit, not just scaled. Behind the
              mark, non-interactive; fades in via the parent's hover (group/brandmark). */}
          <span
            aria-hidden
            className="pointer-events-none absolute -inset-1 rounded-xl opacity-0 blur-md transition-opacity duration-300 group-hover/brandmark:opacity-70"
            style={{ background: "linear-gradient(135deg, hsl(var(--accent) / 0.5), hsl(var(--accent-2) / 0.4))" }}
          />
          <span className="relative block">
            <BrandMark
              siteAbbr={brand.siteAbbr}
              logoDataUri={brand.logoDataUri}
              iconDataUri={brand.iconDataUri}
            />
          </span>
        </motion.span>
      ) : (
        <BrandMark
          siteAbbr={brand.siteAbbr}
          logoDataUri={brand.logoDataUri}
          iconDataUri={brand.iconDataUri}
        />
      )}
      <BrandLabel siteName={brand.siteName} tagline={brand.tagline} />
    </>
  );

  if (!canToggle) {
    return <div className="group/brandmark flex w-full items-center gap-2 p-2">{inner}</div>;
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={onToggle}
          aria-label={toggleLabel}
          className="group/brandmark flex w-full items-center gap-2 rounded-md p-2 text-left outline-none transition-colors hover:bg-fg/[0.04] focus-visible:ring-2 focus-visible:ring-ring group-data-[collapsible=icon]:justify-center"
        >
          {inner}
        </button>
      </TooltipTrigger>
      <TooltipContent side="right" align="center">
        {toggleLabel}
      </TooltipContent>
    </Tooltip>
  );
}
