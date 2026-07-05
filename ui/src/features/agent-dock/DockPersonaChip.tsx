// The dock PERSONA CHIP (persona-session #5) — the header control that ALWAYS shows exactly what the
// next invoke will send as `persona`, and why. The chip + the run must never disagree (one gateway test
// pins this), so the chip receives the RESOLVED focus from `usePersonaFocus` and the dock sends exactly
// `current.id` (or undefined when null). Presentation + wiring only (FILE-LAYOUT); the focus derivation
// lives in the hook, the pin storage in `personaPin.ts`.
//
// The chip's three states mirror the precedence layers:
//   - "Pinned"        — the user set the pin in this tab (sessionStorage); sticky until cleared.
//   - "From this page" — the page surface matched an enabled persona's `surfaces` (zero user action).
//   - "Workspace default" — no pin, no match; the server's prefs fold decides (may land on none).
// The switcher lists ENABLED personas (disabled ones are hidden — curation working as intended) and
// pins one on click; "Clear pin" returns to the context match (or default). Persona ids are OPAQUE
// (rule 10) — no branch on a specific id, only the resolved `current` and the roster list.

import { useState } from "react";
import { Check, ChevronsUpDown, Pin } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { PersonaFocus } from "./usePersonaFocus";

interface Props {
  focus: PersonaFocus;
}

export function DockPersonaChip({ focus }: Props) {
  const [open, setOpen] = useState(false);

  const { current, options, roster } = focus;
  const hasRoster = roster.length > 0;

  // The chip's label + caption — exactly what the next invoke will send. The chip is NOT a lie: if
  // `current` is null we say "Workspace default" (no `persona` arg ⇒ the server's prefs fold).
  const label = current?.label ?? "Workspace default";
  const caption =
    current?.reason === "pinned"
      ? "pinned"
      : current?.reason === "context"
        ? "from this page"
        : "workspace default";

  // No roster available (denied list / loading on first paint) — render nothing so the header stays
  // clean. The dock still sends invokes (with no persona arg); the server folds prefs as usual.
  if (!hasRoster && !focus.loading) return null;
  if (focus.loading && !hasRoster) return null;

  const onPick = (id: string) => {
    focus.pin(id);
    setOpen(false);
  };
  const onClear = () => {
    focus.clearPin();
    setOpen(false);
  };

  return (
    <div className="relative shrink-0">
      <Button
        type="button"
        variant="outline"
        size="sm"
        aria-label="persona focus"
        data-persona-id={current?.id ?? ""}
        data-focus-reason={current?.reason ?? "default"}
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
        className="h-8 gap-1.5 px-2 text-xs"
        title={`Focus: ${label} — ${caption}`}
      >
        {current?.reason === "pinned" ? (
          <Pin size={12} className="shrink-0 text-accent" />
        ) : (
          <span className="shrink-0 text-muted">●</span>
        )}
        <span className="max-w-[7rem] truncate font-medium">{label}</span>
        <ChevronsUpDown size={11} className="shrink-0 text-muted" />
      </Button>

      {open && (
        <>
          {/* click-away cloak — closes the popover without a focus trap (the dock is non-modal). */}
          <button
            type="button"
            aria-hidden
            tabIndex={-1}
            className="fixed inset-0 z-30 cursor-default"
            onClick={() => setOpen(false)}
          />
          <div
            role="listbox"
            aria-label="persona switcher"
            className="absolute right-0 top-full z-40 mt-1 min-w-[14rem] max-w-[20rem] rounded-md border border-border bg-panel shadow-lg"
          >
            <div className="border-b border-border px-2 py-1.5 text-[10px] uppercase tracking-wide text-muted">
              Pin a focus for this tab
            </div>
            <ul className="max-h-64 overflow-auto py-1">
              {options.length === 0 ? (
                <li className="px-3 py-2 text-xs text-muted">No personas enabled.</li>
              ) : (
                options.map((p) => {
                  const selected = current?.id === p.id && current?.reason === "pinned";
                  return (
                    <li key={p.id}>
                      <button
                        type="button"
                        role="option"
                        aria-selected={selected}
                        aria-label={`pin ${p.id}`}
                        onClick={() => onPick(p.id)}
                        className="flex w-full items-center gap-2 px-2.5 py-1.5 text-left text-xs text-fg hover:bg-panel-2"
                      >
                        <span className="min-w-0 flex-1 truncate">
                          {p.label}
                          <span className="ml-1 text-[10px] text-muted">{p.id}</span>
                        </span>
                        {selected && <Check size={12} className="shrink-0 text-accent" />}
                      </button>
                    </li>
                  );
                })
              )}
            </ul>
            {focus.pinId && (
              <button
                type="button"
                onClick={onClear}
                aria-label="clear persona pin"
                className="block w-full border-t border-border px-2.5 py-1.5 text-left text-xs text-muted hover:bg-panel-2"
              >
                Clear pin
              </button>
            )}
            <p className="border-t border-border px-2.5 py-1.5 text-[10px] leading-snug text-muted">
              A pin lives in this tab only — other tabs and members are unaffected.
            </p>
          </div>
        </>
      )}
    </div>
  );
}
