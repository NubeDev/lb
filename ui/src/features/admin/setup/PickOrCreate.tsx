// A reusable "pick an existing one, or create a new one" control (setup scope). Three of the wizard's
// steps need the same shape — choose from a real roster (users / teams) or type a new id and create it
// on the spot. Rather than repeat the toggle+form in each step, it lives here once (FILE-LAYOUT: one
// responsibility, folder-of-verbs). Presentational + a create callback; the roster + verb are the
// caller's (they come from the real host lists).

import { useState } from "react";
import { Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { cn } from "@/lib/utils";

export interface PickOption {
  value: string;
  label: string;
}

interface Props {
  /** What we're choosing (for labels/placeholders): e.g. "user", "team". */
  noun: string;
  options: PickOption[];
  /** The current selection (a value from `options`, or a freshly-created id). */
  value: string;
  onSelect: (value: string) => void;
  /** Create a new one; resolves to its id so we can auto-select it. Errors surface via the caller. */
  onCreate: (id: string) => Promise<string>;
  /** Optional slug-ify for the typed id (defaults to lowercase-dashed). */
  slugify?: (raw: string) => string;
}

const defaultSlug = (raw: string) =>
  raw.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

export function PickOrCreate({ noun, options, value, onSelect, onCreate, slugify }: Props) {
  const [mode, setMode] = useState<"pick" | "new">(options.length ? "pick" : "new");
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);

  const create = async () => {
    const id = (slugify ?? defaultSlug)(draft);
    if (!id) return;
    setBusy(true);
    try {
      const made = await onCreate(id);
      onSelect(made);
      setDraft("");
      setMode("pick");
    } catch {
      /* the caller surfaces the error banner */
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-2">
      {/* The pick / new toggle — a two-segment control (matches the shell's segmented feel). */}
      <div className="inline-flex rounded-md border border-border bg-panel p-0.5 text-xs">
        {(["pick", "new"] as const).map((m) => (
          // eslint-disable-next-line no-restricted-syntax -- a segmented toggle track, not a shadcn Button shape
          <button
            key={m}
            type="button"
            disabled={m === "pick" && options.length === 0}
            onClick={() => setMode(m)}
            className={cn(
              "rounded px-3 py-1 font-medium transition-colors disabled:opacity-40",
              mode === m ? "bg-primary text-primary-foreground" : "text-muted hover:text-fg",
            )}
          >
            {m === "pick" ? `Existing ${noun}` : `New ${noun}`}
          </button>
        ))}
      </div>

      {mode === "pick" ? (
        <Select
          aria-label={`Choose a ${noun}`}
          value={value}
          onChange={(e) => onSelect(e.target.value)}
        >
          <option value="">
            {options.length === 0 ? `No ${noun}s yet — create one` : `Choose a ${noun}…`}
          </option>
          {options.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </Select>
      ) : (
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            void create();
          }}
        >
          <Input
            autoFocus
            aria-label={`New ${noun} id`}
            placeholder={`New ${noun} id (e.g. ${noun === "user" ? "ada" : "ops"})`}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
          />
          <Button type="submit" size="sm" disabled={busy || !draft.trim()} aria-label={`Create ${noun}`}>
            <Plus size={13} /> Create
          </Button>
        </form>
      )}
    </div>
  );
}
