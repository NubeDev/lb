// The build's streaming output as a proper terminal panel — the focus of the Build step, not a
// side note. Auto-scrolls to the newest line; shows a live "streaming" pulse while a build is running
// and a quiet resting hint before one starts.

import { useEffect, useRef } from "react";
import { Terminal } from "lucide-react";

interface Props {
  logs: string[];
  streaming: boolean;
}

export function BuildLog({ logs, streaming }: Props) {
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [logs]);

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border border-border bg-[#0d0b09] text-[#e7e0d6]">
      <div className="flex items-center gap-2 border-b border-white/10 px-3 py-2 text-xs">
        <Terminal size={13} className="text-accent" />
        <span className="font-medium">Build output</span>
        {streaming && (
          <span className="ml-auto flex items-center gap-1.5 text-[11px] text-accent">
            <span className="relative flex h-1.5 w-1.5">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-accent/70" />
              <span className="relative inline-flex h-1.5 w-1.5 rounded-full bg-accent" />
            </span>
            streaming
          </span>
        )}
      </div>
      <div className="min-h-0 flex-1 overflow-auto p-3">
        {logs.length ? (
          <pre className="whitespace-pre-wrap break-words font-mono text-[11px] leading-5">
            {logs.join("\n")}
            <div ref={endRef} />
          </pre>
        ) : (
          <div className="flex h-full items-center justify-center text-[11px] text-white/35">
            Output will stream here once the build starts.
          </div>
        )}
      </div>
    </div>
  );
}
