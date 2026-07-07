// The save-query dialog (datasources-ux + query scope) — wraps a small id/name/description form in
// the shadcn `Dialog` primitive so saving the SQL editor's current text as a reusable
// `query:{ws}:{id}` record is a focused pop-out (focus trapping + Escape/overlay dismissal for
// free). Mirrors the `AddDatasourceDialog` action-in-header pattern. The text + target are implicit
// (the editor's SQL + this datasource); the author supplies only the slug + an optional label +
// description. Closes on submit. One responsibility, one file (FILE-LAYOUT).

import { useEffect, useState } from "react";
import { Save } from "lucide-react";

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
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";

interface Props {
  /** The datasource name — baked into the saved target (`datasource:<source>`), shown in the copy. */
  source: string;
  /** The SQL the editor will save (the dialog disables submit when empty). */
  sql: string;
  /** Disabled when there's nothing to save (mirrors the Run button's empty-SQL gate). */
  disabled?: boolean;
  /** Save the query; resolves to the saved id. The dialog closes on resolve, surfaces an error on throw. */
  onSave: (args: { id: string; name?: string; description?: string; sql: string }) => Promise<string>;
}

/** Slugify a candidate id — kebab-case, lowercased, non-alnum → `-`. Mirrors the rule/query slug
 *  convention so an authored "Daily Revenue" becomes `daily-revenue`. */
function slugify(s: string): string {
  return s
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function SaveQueryDialog({ source, sql, disabled, onSave }: Props) {
  const [open, setOpen] = useState(false);
  const [id, setId] = useState("");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reset the form each time the dialog opens (stale state from a prior save is jarring).
  useEffect(() => {
    if (open) {
      setId("");
      setName("");
      setDescription("");
      setError(null);
    }
  }, [open]);

  const canSubmit = !busy && id.trim().length > 0 && !disabled;

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canSubmit) return;
    setBusy(true);
    setError(null);
    try {
      await onSave({
        id: slugify(id),
        name: name.trim() || undefined,
        description: description.trim() || undefined,
        sql,
      });
      setOpen(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <Button
        aria-label="save query"
        size="sm"
        variant="ghost"
        className="gap-1.5"
        disabled={disabled}
        onClick={() => setOpen(true)}
      >
        <Save size={13} /> Save
      </Button>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Save query</DialogTitle>
          <DialogDescription>
            Save the current SQL as a reusable query against <code className="font-mono">{source}</code>.
            Saved queries appear in the source picker (Data Studio, dashboards, rules).
          </DialogDescription>
        </DialogHeader>
        <form aria-label="save query form" className="space-y-3" onSubmit={submit}>
          <div className="space-y-1.5">
            <Label htmlFor="query-id">Id</Label>
            <Input
              id="query-id"
              aria-label="query id"
              placeholder="daily-revenue"
              value={id}
              onChange={(e) => setId(e.target.value)}
              autoFocus
            />
            <p className="text-xs text-muted">
              Kebab-case slug, unique per workspace. Saving an existing id overwrites it.
            </p>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="query-name">Name</Label>
            <Input
              id="query-name"
              aria-label="query name"
              placeholder="Daily revenue (defaults to id)"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="query-description">Description</Label>
            <Textarea
              id="query-description"
              aria-label="query description"
              placeholder="What this query is for (optional)"
              rows={2}
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>
          {error && (
            <p role="alert" className="text-xs text-destructive">
              {error}
            </p>
          )}
          <DialogFooter>
            <Button aria-label="submit query" size="sm" type="submit" disabled={!canSubmit}>
              {busy ? "Saving…" : "Save query"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
