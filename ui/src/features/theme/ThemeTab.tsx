// The Customizer's Theme tab — composes the preset picker, radius, mode, import, and brand colors, plus
// Reset and the admin-only "Set as workspace default". Ported UX from the shadcn-store Customizer, but
// every control drives the shell's BASE tokens (not shadcn tokens) via the theme layer. The Layout tab
// is a deliberate non-goal (the shell is NavRail + StudioShell, not a shadcn Sidebar). One component
// per file (FILE-LAYOUT); the controls are their own files.

import * as React from "react";

import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { persistWorkspaceDefaultTheme, useTheme } from "@/lib/theme";
import { CAP, hasCap, useSession } from "@/lib/session";

import { BrandColors } from "./BrandColors";
import { FontPicker } from "./FontPicker";
import { ImportField } from "./ImportField";
import { LookPicker } from "./LookPicker";
import { ModeToggle } from "./ModeToggle";
import { MotionPicker } from "./MotionPicker";
import { PresetPicker } from "./PresetPicker";
import { RadiusPicker } from "./RadiusPicker";
import { SurfacePicker } from "./SurfacePicker";

export function ThemeTab() {
  const { theme, reset } = useTheme();
  const { session } = useSession();
  const canSetDefault = hasCap(session?.caps, CAP.prefsSetDefault);
  const [defaultState, setDefaultState] = React.useState<"idle" | "saving" | "saved" | "denied">("idle");

  const setWorkspaceDefault = async () => {
    setDefaultState("saving");
    try {
      await persistWorkspaceDefaultTheme(theme);
      setDefaultState("saved");
    } catch {
      // Opaque deny (or transient failure) — the control was shown but the write was refused server-side.
      setDefaultState("denied");
    }
  };

  return (
    <div className="space-y-5 p-4">
      <LookPicker />
      <Separator />
      <PresetPicker />
      <Separator />
      <FontPicker />
      <Separator />
      <SurfacePicker />
      <Separator />
      <MotionPicker />
      <Separator />
      <RadiusPicker />
      <Separator />
      <ModeToggle />
      <Separator />
      <ImportField />
      <Separator />
      <BrandColors />
      <Separator />

      <div className="space-y-2">
        <Button type="button" variant="outline" size="sm" className="w-full" onClick={reset}>
          Reset to default
        </Button>

        {canSetDefault && (
          <div className="space-y-1">
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="w-full"
              onClick={setWorkspaceDefault}
              disabled={defaultState === "saving"}
            >
              {defaultState === "saving" ? "Saving…" : "Set as workspace default"}
            </Button>
            {defaultState === "saved" && (
              <p className="text-xs text-muted">Saved as the workspace default theme.</p>
            )}
            {defaultState === "denied" && (
              <p role="alert" className="text-xs text-red-500">
                Could not set the workspace default.
              </p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
