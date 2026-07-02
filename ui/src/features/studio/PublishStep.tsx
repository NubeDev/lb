// Step 3 — the terminal action. Before publishing: a plain-language confirmation of exactly what's
// about to happen and where. After: a success state that closes the loop and offers the one obvious
// next move — build another. No dangling controls; the flow has an end.

import { CircleCheck, PackageCheck, Rocket } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import type { StudioWizard } from "./studio.wizard";

export function PublishStep({ w }: { w: StudioWizard }) {
  if (w.published) {
    return (
      <div className="flex flex-1 items-center justify-center py-6">
        <div className="flex max-w-sm flex-col items-center text-center">
          <div className="mb-4 flex h-12 w-12 items-center justify-center rounded-full border border-accent/30 bg-accent/10 text-accent">
            <CircleCheck size={24} />
          </div>
          <p className="text-sm font-medium text-fg">
            Published to the workspace
          </p>
          <p className="mt-1 text-xs leading-5 text-muted">
            <span className="font-mono text-fg">{w.inspect?.id}</span> is
            registered and its tools are callable over MCP. Reload the extension
            list to see it.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="rounded-lg border border-border bg-bg/60 p-4">
        <div className="flex items-center gap-2 text-sm font-medium text-fg">
          <Rocket size={15} className="text-accent" />
          Ready to publish
        </div>
        <p className="mt-1.5 text-xs leading-5 text-muted">
          This registers the built artifact into the current workspace's signed
          registry. Its tools become callable over MCP by the UI, agents, and
          other extensions.
        </p>
        <dl className="mt-3 grid gap-2 text-xs">
          <div className="grid grid-cols-[4rem_1fr] items-baseline gap-2">
            <dt className="text-muted">Extension</dt>
            <dd className="font-mono text-[11px] text-fg">{w.inspect?.id}</dd>
          </div>
          <div className="grid grid-cols-[4rem_1fr] items-baseline gap-2">
            <dt className="text-muted">Tools</dt>
            <dd className="font-mono text-[11px] text-fg">
              {w.inspect?.tools.join(", ") || "none declared"}
            </dd>
          </div>
        </dl>
      </div>

      <div className="flex items-center gap-2 text-xs text-muted">
        <Badge variant="default" className="gap-1.5">
          <PackageCheck size={12} />
          artifact built
        </Badge>
        <span>
          from <span className="font-mono text-fg">{w.path}</span>
        </span>
      </div>
    </div>
  );
}
