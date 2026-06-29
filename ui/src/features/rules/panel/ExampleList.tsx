// The examples tab — a list of ready-to-run example rules; one click loads the body into the editor
// buffer (rules-editor-ux scope). The parent (`useRules.loadExample`) guards the dirty indicator, so an
// unsaved edit isn't silently clobbered. Each row teaches: a title + a one-line note. One component per
// file (FILE-LAYOUT).

import { FileCode } from "lucide-react";

import { Button } from "@/components/ui/button";
import { EXAMPLES } from "../examples/examples";

interface ExampleListProps {
  /** Load an example body into the editor buffer (dirty-confirmed by the parent). */
  onLoad: (body: string) => void;
}

/** The clickable list of example rules. */
export function ExampleList({ onLoad }: ExampleListProps) {
  return (
    <div aria-label="examples" className="flex h-full flex-col overflow-auto p-2">
      <p className="px-1 pb-2 text-[11px] text-muted">
        Click an example to load it into the editor, then Run.
      </p>
      <ul className="grid gap-1">
        {EXAMPLES.map((ex) => (
          <li key={ex.id}>
            <Button
              type="button"
              variant="ghost"
              aria-label={`load example ${ex.title}`}
              onClick={() => onLoad(ex.body)}
              className="h-auto w-full flex-col items-start gap-0.5 rounded-md border border-border bg-card px-2.5 py-2 text-left hover:border-accent hover:bg-muted"
            >
              <span className="flex items-center gap-1.5 text-xs font-medium text-fg">
                <FileCode size={13} className="text-muted" />
                {ex.title}
              </span>
              <span className="text-[11px] font-normal leading-snug text-muted">{ex.summary}</span>
            </Button>
          </li>
        ))}
      </ul>
    </div>
  );
}
