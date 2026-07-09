// The cron-builder authoring component — a thin wrapper around `react-js-cron` (reminders scope's
// pinned React cron builder). Most users do not read cron, so the UI authoring surface is a visual
// builder that reads/writes a standard 5-field cron string (lossless round-trip). It is antd-based,
// so the wrapper scopes antd's ConfigProvider to THIS subtree (the shell's global theme is Tailwind
// + shadcn — antd is NOT pulled into the global theme, per the scope decision). One component, one
// concern (FILE-LAYOUT): a labeled cron field that round-trips a string.
//
// Theming: antd needs concrete color strings, but the app's palette lives in CSS custom properties
// (`--bg`/`--fg`/`--accent`/… as HSL triples) that light/dark/custom themes rewrite at runtime. So we
// READ the live tokens off `:root` via `getComputedStyle` and feed them to antd's token + component
// tokens — the builder (field AND its portalled dropdowns) then tracks whatever theme is active
// instead of a hardcoded amber-on-dark. `useThemeOptional()` re-renders us on a theme change (and
// lets us degrade outside a `ThemeProvider`, like the other broad widgets).

import { useMemo, useRef } from "react";
import { ConfigProvider, theme as antdTheme } from "antd";
import { Cron } from "react-js-cron";
import "react-js-cron/dist/styles.css";

import { useThemeOptional } from "@/lib/theme";

interface Props {
  /** The current 5-field cron string (e.g. `0 8 * * 0,1`). */
  value: string;
  /** Called with the new cron string on every edit (lossless round-trip). */
  onChange: (value: string) => void;
}

/** Resolve one CSS custom property (an HSL triple like `178 72% 27%`) into a usable `hsl(...)` color.
 *  Reads from `:root` computed styles so it reflects the active theme (mode + accent + custom). */
function token(name: string, fallback: string): string {
  if (typeof window === "undefined") return fallback;
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return raw ? `hsl(${raw})` : fallback;
}

/** Is the app currently in dark mode? The shell toggles a `.dark` class on the root. */
function isDark(): boolean {
  return typeof document !== "undefined" && document.documentElement.classList.contains("dark");
}

/** A labeled visual cron authoring field. Renders the antd `Cron` builder scoped under a local
 *  ConfigProvider bound to the app's live theme tokens so antd never touches the global theme yet
 *  reads as one app (field, selects, and the portalled dropdown menus alike). */
export function CronBuilder({ value, onChange }: Props) {
  // Re-render whenever the theme preference changes so the tokens below are re-read from `:root`.
  const themeCtx = useThemeOptional();
  const containerRef = useRef<HTMLDivElement>(null);

  const antdConfig = useMemo(() => {
    const dark = isDark();
    const bg = token("--panel", dark ? "#1a1a1a" : "#ffffff");
    const bgElevated = token("--panel-2", bg);
    const fg = token("--fg", dark ? "#e5e5e5" : "#111111");
    const muted = token("--muted", dark ? "#9ca3af" : "#6b7280");
    const border = token("--border", dark ? "#3a3a3a" : "#d4d4d8");
    const accent = token("--accent", "#0d9488");
    const accentFg = token("--accent-foreground", dark ? "#0a0a0a" : "#ffffff");
    // A faint tint for hover/active option rows — replaces antd's default (and the old amber hover).
    const controlHover = token("--muted-bg", dark ? "#262626" : "#f1f5f9");

    return {
      // Track the app's light/dark base so text/borders invert correctly with the shell.
      algorithm: dark ? antdTheme.darkAlgorithm : antdTheme.defaultAlgorithm,
      token: {
        colorPrimary: accent,
        colorBorder: border,
        colorText: fg,
        colorTextPlaceholder: muted,
        colorBgContainer: bg,
        colorBgElevated: bgElevated,
        borderRadius: 6,
        controlItemBgHover: controlHover,
        controlItemBgActive: accent,
      },
      components: {
        Select: {
          // The dropdown menu surfaces (portalled) — bind them to the app palette + accent selection
          // so the mustard/brown default selected-row is gone and hover reads as the app's accent.
          optionSelectedBg: accent,
          optionSelectedColor: accentFg,
          optionActiveBg: controlHover,
          colorPrimary: accent,
        },
      },
    };
    // Recompute on any theme change: `themeCtx.theme` isn't read inside the memo (the colors come
    // from `getComputedStyle` off `:root`), it's the re-read TRIGGER — so exhaustive-deps can't see
    // why it belongs here. It does: mode/accent/custom all flow through the preference object.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [themeCtx?.theme]);

  return (
    <ConfigProvider
      theme={antdConfig}
      // Portal the dropdowns INTO this subtree so they sit under the same scoped ConfigProvider (and
      // inside dialogs, above the overlay) rather than at the document root with default antd chrome.
      getPopupContainer={() => containerRef.current ?? document.body}
    >
      <div ref={containerRef} className="cron-builder">
        <Cron value={value} setValue={onChange} clockFormat="12-hour-clock" />
      </div>
    </ConfigProvider>
  );
}
