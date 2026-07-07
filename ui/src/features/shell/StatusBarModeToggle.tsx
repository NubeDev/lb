// The StatusBar's quick mode flip — a single compact icon button that toggles dark/light, sized to
// ride the 7-unit ops strip alongside the dock launcher. Presentation only (FILE-LAYOUT): the mode
// itself comes from the shell's ThemeProvider, and the same change flows through the same DOM/token
// path the Settings → Theme ModeToggle uses, so this is purely a discoverable pointer affordance.

import { Moon, Sun } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useTheme, type ThemeMode } from "@/lib/theme";

const NEXT: Record<ThemeMode, { icon: typeof Sun; to: ThemeMode; label: string }> = {
  dark: { icon: Sun, to: "light", label: "Switch to light mode" },
  light: { icon: Moon, to: "dark", label: "Switch to dark mode" },
};

export function StatusBarModeToggle() {
  const { theme, setMode } = useTheme();
  const next = NEXT[theme.mode];
  const Icon = next.icon;
  return (
    <Button
      type="button"
      variant="ghost"
      aria-label={next.label}
      aria-pressed={theme.mode === "dark"}
      title={next.label}
      onClick={() => setMode(next.to)}
      className="h-5 w-6 px-0 text-muted hover:text-fg"
    >
      <Icon className="h-3 w-3" />
    </Button>
  );
}
