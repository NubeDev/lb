import { Check, Moon, Sun } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { THEME_ACCENT_OPTIONS, THEME_MODE_OPTIONS, type ThemeAccent, type ThemeMode, useTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";

const ACCENT_SWATCH_CLASS: Record<ThemeAccent, string> = {
  amber: "bg-[hsl(var(--theme-swatch-amber))]",
  teal: "bg-[hsl(var(--theme-swatch-teal))]",
  blue: "bg-[hsl(var(--theme-swatch-blue))]",
};

function modeIcon(mode: ThemeMode) {
  return mode === "dark" ? Moon : Sun;
}

export function ThemeSwitcher() {
  const { theme, setMode, setAccent } = useTheme();
  const nextMode = theme.mode === "dark" ? "light" : "dark";
  const CollapsedModeIcon = modeIcon(theme.mode);

  return (
    <TooltipProvider>
      <div className="w-full px-1 group-data-[collapsible=icon]:px-0">
      <div className="space-y-1.5 rounded-md border border-border bg-bg/60 p-1.5 shadow-sm shadow-black/5 group-data-[collapsible=icon]:hidden">
        <div className="grid grid-cols-2 gap-1">
          {THEME_MODE_OPTIONS.map((option) => {
            const Icon = modeIcon(option.value);
            const selected = theme.mode === option.value;
            return (
              <Tooltip key={option.value}>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    size="sm"
                    variant={selected ? "default" : "ghost"}
                    aria-label={`Use ${option.label.toLowerCase()} mode`}
                    aria-pressed={selected}
                    className={cn("h-8 px-2 text-xs", !selected && "text-muted hover:bg-panel")}
                    onClick={() => setMode(option.value)}
                  >
                    <Icon className="h-3.5 w-3.5" />
                    <span>{option.label}</span>
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="right">{option.label}</TooltipContent>
              </Tooltip>
            );
          })}
        </div>

        <div className="grid grid-cols-3 gap-1">
          {THEME_ACCENT_OPTIONS.map((option) => {
            const selected = theme.accent === option.value;
            return (
              <Tooltip key={option.value}>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    aria-label={`Use ${option.label.toLowerCase()} accent`}
                    aria-pressed={selected}
                    className={cn(
                      "h-8 w-full hover:bg-panel",
                      selected && "bg-panel ring-1 ring-accent/45 hover:bg-panel",
                    )}
                    onClick={() => setAccent(option.value)}
                  >
                    <span
                      className={cn(
                        "flex h-4 w-4 items-center justify-center rounded-full border border-black/10 shadow-sm shadow-black/10",
                        ACCENT_SWATCH_CLASS[option.value],
                      )}
                    >
                      {selected && <Check className="h-3 w-3 text-bg" />}
                    </span>
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="right">{option.label}</TooltipContent>
              </Tooltip>
            );
          })}
        </div>
      </div>

      <div className="hidden flex-col items-center gap-1 group-data-[collapsible=icon]:flex">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              size="icon"
              variant="ghost"
              aria-label={`Switch to ${nextMode} mode`}
              className="h-8 w-8 text-muted hover:bg-bg hover:text-fg"
              onClick={() => setMode(nextMode)}
            >
              <CollapsedModeIcon className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">{theme.mode === "dark" ? "Dark" : "Light"}</TooltipContent>
        </Tooltip>

        {THEME_ACCENT_OPTIONS.map((option) => {
          const selected = theme.accent === option.value;
          return (
            <Tooltip key={option.value}>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  size="icon"
                  variant="ghost"
                  aria-label={`Use ${option.label.toLowerCase()} accent`}
                  aria-pressed={selected}
                  className={cn("h-8 w-8 hover:bg-bg", selected && "bg-bg ring-1 ring-accent/45")}
                  onClick={() => setAccent(option.value)}
                >
                  <span
                    className={cn(
                      "flex h-4 w-4 items-center justify-center rounded-full border border-black/10 shadow-sm shadow-black/10",
                      ACCENT_SWATCH_CLASS[option.value],
                    )}
                  >
                    {selected && <Check className="h-3 w-3 text-bg" />}
                  </span>
                </Button>
              </TooltipTrigger>
              <TooltipContent side="right">{option.label}</TooltipContent>
            </Tooltip>
          );
        })}
      </div>
      </div>
    </TooltipProvider>
  );
}
