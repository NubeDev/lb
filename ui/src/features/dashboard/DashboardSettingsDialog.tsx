// The dashboard page-settings dialog (dashboard page-settings). A modal over the three page-presentation
// fields the record now carries: the one-line DESCRIPTION shown under the title, the page ICON (a stable
// icon-lib name, picked from the shared `@/lib/icons` catalog), and its COLOUR (a preset swatch or a
// custom CSS colour). Editing is local; Save writes the three fields back through `onSave` → `saveMeta`
// (→ `dashboard.save` with only the meta keys, so cells/variables are preserved). Owner-only is enforced
// host-side; the opener gates on the edit cap. One responsibility: the page-settings interaction.

import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Icon, IconPicker } from "@/lib/icons";
import { cn } from "@/lib/utils";
import type { Dashboard, DashboardMeta, Toolbar } from "@/lib/dashboard";

interface Props {
  dashboard: Dashboard;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Persist the edited page settings (description/icon/color) → `saveMeta`. */
  onSave: (meta: DashboardMeta) => void;
}

/** The default page blurb — kept in sync with `DashboardView`'s header fallback so an unset description
 *  reads the same in the dialog placeholder and on the page. */
export const DEFAULT_DASHBOARD_DESCRIPTION =
  "Live workspace dashboards and series widgets.";

/** A small preset palette (shell-token-adjacent hues) — the common picks in one click; the custom input
 *  covers anything else. Values are plain CSS colours (opaque to the host). */
const SWATCHES = [
  "#3b82f6", // blue
  "#22c55e", // green
  "#f59e0b", // amber
  "#ef4444", // red
  "#a855f7", // purple
  "#ec4899", // pink
  "#14b8a6", // teal
  "#64748b", // slate
];

/** The three opt-in header controls (dashboard toolbar-settings), each hidden by default. Keyed by the
 *  `Toolbar` field so the toggle maps 1:1 to what the header reads. */
const TOOLBAR_CONTROLS: {
  key: keyof Toolbar;
  label: string;
  hint: string;
}[] = [
  {
    key: "dateSelect",
    label: "Date range",
    hint: "From / to date pickers for the board's time window.",
  },
  {
    key: "refreshRate",
    label: "Refresh rate",
    hint: "Auto-refresh interval control.",
  },
  {
    key: "share",
    label: "Share & visibility",
    hint: "Share button and the private / team / workspace selector.",
  },
];

export function DashboardSettingsDialog({ dashboard, open, onOpenChange, onSave }: Props) {
  const [description, setDescription] = useState(dashboard.description ?? "");
  const [icon, setIcon] = useState(dashboard.icon ?? "");
  const [color, setColor] = useState(dashboard.color ?? "");
  // Header-chrome flags (dashboard toolbar-settings) — each control is HIDDEN by default; the author
  // opts it in here. Seeded from the stored `toolbar` (absent ⇒ all off).
  const [toolbar, setToolbar] = useState<Toolbar>(dashboard.toolbar ?? {});

  // Re-seed from the record ONLY on the open→ transition (a fresh edit starts from the stored value).
  // Depending on `dashboard` here would re-seed on every parent re-render while the dialog is open,
  // silently wiping a pending edit (e.g. a toggle flipped but not yet saved). `open` is the only trigger.
  useEffect(() => {
    if (open) {
      setDescription(dashboard.description ?? "");
      setIcon(dashboard.icon ?? "");
      setColor(dashboard.color ?? "");
      setToolbar(dashboard.toolbar ?? {});
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const save = () => {
    onSave({
      description: description.trim(),
      icon,
      color,
      // Send explicit booleans (not an undefined-riddled partial) so the stored flags mirror the
      // toggles exactly — the host preserves an OMITTED `toolbar`, but here we mean to set it.
      toolbar: {
        dateSelect: !!toolbar.dateSelect,
        refreshRate: !!toolbar.refreshRate,
        share: !!toolbar.share,
      },
    });
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Page settings — “{dashboard.title}”</DialogTitle>
          <DialogDescription>
            How this dashboard page presents itself — its subtitle, icon and accent colour. Applies on
            save.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-5">
          {/* A live preview of the header icon chip in the chosen colour. */}
          <div className="flex items-center gap-3 rounded-md border border-border/60 bg-panel-2/40 px-3 py-2.5">
            <span
              className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border"
              style={{
                color: color || "hsl(var(--accent))",
                borderColor: color ? `${color}40` : "hsl(var(--accent) / 0.25)",
                background: color ? `${color}1a` : "hsl(var(--accent) / 0.12)",
              }}
            >
              <Icon name={icon} fallback="layout-dashboard" className="size-4" />
            </span>
            <span className="min-w-0">
              <span className="block truncate text-sm font-medium text-fg">{dashboard.title}</span>
              <span className="block truncate text-xs text-muted">
                {description.trim() || DEFAULT_DASHBOARD_DESCRIPTION}
              </span>
            </span>
          </div>

          {/* Description */}
          <label className="flex flex-col gap-1.5">
            <span className="text-sm font-medium text-fg">Description</span>
            <Input
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={DEFAULT_DASHBOARD_DESCRIPTION}
              aria-label="Dashboard description"
              maxLength={160}
            />
            <span className="text-xs text-muted">
              A one-line subtitle shown under the page title.
            </span>
          </label>

          {/* Toolbar chrome — each control is hidden by default; opt one in here (toolbar-settings). */}
          <div className="flex flex-col gap-1.5">
            <span className="text-sm font-medium text-fg">Toolbar</span>
            <div className="flex flex-col divide-y divide-border/60 rounded-md border border-border/60 bg-panel-2/40">
              {TOOLBAR_CONTROLS.map(({ key, label, hint }) => (
                // A plain row, NOT a <label> — the Switch is a <button>, and a <button> nested in a
                // <label> double-fires (the label forwards its click to the control), toggling on then
                // off. The Switch carries its own aria-label for the accessible name.
                <div
                  key={key}
                  className="flex items-center justify-between gap-3 px-3 py-2.5"
                >
                  <span className="min-w-0">
                    <span className="block text-sm text-fg">{label}</span>
                    <span className="block text-xs text-muted">{hint}</span>
                  </span>
                  <Switch
                    aria-label={label}
                    checked={!!toolbar[key]}
                    onCheckedChange={(on) =>
                      setToolbar((t) => ({ ...t, [key]: on }))
                    }
                  />
                </div>
              ))}
            </div>
            <span className="text-xs text-muted">
              All hidden by default — turn on only what this page needs.
            </span>
          </div>

          {/* Colour — presets + a custom picker */}
          <div className="flex flex-col gap-1.5">
            <span className="text-sm font-medium text-fg">Icon colour</span>
            <div className="flex flex-wrap items-center gap-2">
              {SWATCHES.map((c) => (
                <button
                  key={c}
                  type="button"
                  aria-label={`Colour ${c}`}
                  aria-pressed={color === c}
                  title={c}
                  onClick={() => setColor(c)}
                  className={cn(
                    "h-6 w-6 rounded-full border border-border/60 transition-transform hover:scale-110",
                    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent",
                    color === c && "ring-2 ring-accent ring-offset-1 ring-offset-bg",
                  )}
                  style={{ background: c }}
                />
              ))}
              {/* Custom colour (native picker) */}
              <input
                type="color"
                aria-label="Custom icon colour"
                title="Custom colour"
                value={/^#[0-9a-fA-F]{6}$/.test(color) ? color : "#3b82f6"}
                onChange={(e) => setColor(e.target.value)}
                className="h-6 w-6 cursor-pointer rounded-full border border-border/60 bg-transparent p-0"
              />
              {color && (
                <button
                  type="button"
                  onClick={() => setColor("")}
                  className="text-xs text-muted underline-offset-2 hover:text-fg hover:underline"
                >
                  Reset
                </button>
              )}
            </div>
            <span className="text-xs text-muted">
              Empty uses the shell accent colour.
            </span>
          </div>

          {/* Icon */}
          <div className="flex flex-col gap-1.5">
            <span className="text-sm font-medium text-fg">Icon</span>
            <IconPicker value={icon} onSelect={setIcon} pageSize={15} columns={8} />
          </div>
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={save}>Save</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
