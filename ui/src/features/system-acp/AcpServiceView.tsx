// The ACP service page (tool-catalog scope) — the read-only detail page for the ACP (Agent Client
// Protocol) adapter, drilled from the System page's ACP card. ACP lets Zed/Cursor/any ACP host drive
// the central agent over stdio (agent-run Part 4). It is a per-session adapter, NOT a polled network
// server, so this page is honest: it shows the adapter's *static capabilities* (protocol version,
// handled methods, advertised capabilities, error codes, auth notes), not a live connection count.
// Obeys the UI standard (shadcn-first, `AppPageHeader`, responsive). Data lives in `useAcpService`.

import { ArrowLeft, Plug2, RefreshCw } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { AcpInfo } from "@/lib/system/system.types";
import { useAcpService } from "./useAcpService";

interface Props {
  ws: string;
  /** Back to the System page (the card this drilled from). Omitted in isolated tests. */
  onBack?: () => void;
}

export function AcpServiceView({ ws, onBack }: Props) {
  const { info, error, loading, refresh } = useAcpService();

  return (
    <section className="flex h-full min-w-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={Plug2}
        title="ACP Service"
        description="Agent Client Protocol — lets an editor (Zed/Cursor) drive the central agent over stdio."
        workspace={ws}
        actions={
          <div className="flex items-center gap-2">
            {info && (
              <Badge variant="outline" className="hidden gap-1.5 sm:inline-flex">
                <span className="text-muted">protocol</span>
                <span className="font-medium tabular-nums text-fg">v{info.protocol_version}</span>
              </Badge>
            )}
            <Button
              variant="outline"
              size="sm"
              aria-label="refresh"
              disabled={loading}
              onClick={() => void refresh()}
            >
              <RefreshCw size={14} className={loading ? "animate-spin" : undefined} />
              Refresh
            </Button>
            {onBack && (
              <Button variant="ghost" size="sm" aria-label="back to system" onClick={onBack}>
                <ArrowLeft size={14} />
                System
              </Button>
            )}
          </div>
        }
      />

      {error && (
        <div
          role="alert"
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-sm text-destructive"
        >
          {error}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto p-4">
        {!info ? (
          <p className="text-sm text-muted">{loading ? "Reading adapter…" : "No ACP info."}</p>
        ) : (
          <div className="mx-auto max-w-3xl space-y-6">
            <AdapterNote
              text={`A per-stdio-session adapter, not a persistent network server — this page reports its capabilities, not a live connection count. Protocol version ${info.protocol_version}.`}
            />

            <Section title="Methods" caption="The ACP requests the adapter handles.">
              <div className="flex flex-wrap gap-2">
                {info.methods.map((m) => (
                  <code
                    key={m}
                    className="rounded-md border border-border bg-bg px-2 py-1 font-mono text-xs text-fg"
                  >
                    {m}
                  </code>
                ))}
              </div>
            </Section>

            <Section title="Capabilities" caption="What the adapter advertises in its handshake.">
              <KeyValueGrid pairs={info.capabilities} />
            </Section>

            <Section title="Error codes" caption="The JSON-RPC errors the adapter can return.">
              <ul className="space-y-2">
                {info.error_codes.map((e) => (
                  <li key={e.label} className="flex flex-col gap-0.5 sm:flex-row sm:gap-3">
                    <code className="shrink-0 font-mono text-xs font-medium text-fg">{e.label}</code>
                    <span className="text-xs text-muted">{e.value}</span>
                  </li>
                ))}
              </ul>
            </Section>

            <Section title="Notes" caption="Auth model + the client-servers decision.">
              <ul className="list-disc space-y-1.5 pl-5">
                {info.notes.map((n, i) => (
                  <li key={i} className="text-xs leading-relaxed text-muted">
                    {n}
                  </li>
                ))}
              </ul>
            </Section>
          </div>
        )}
      </div>
    </section>
  );
}

function Section({
  title,
  caption,
  children,
}: {
  title: string;
  caption: string;
  children: React.ReactNode;
}) {
  return (
    <section aria-label={title} className="space-y-2">
      <div>
        <h3 className="text-sm font-medium text-fg">{title}</h3>
        <p className="text-xs text-muted">{caption}</p>
      </div>
      {children}
    </section>
  );
}

/** A flat label→value grid for the advertised capability flags. */
function KeyValueGrid({ pairs }: { pairs: AcpInfo["capabilities"] }) {
  return (
    <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
      {pairs.map((p) => (
        <span
          key={p.label}
          className="inline-flex items-baseline justify-between gap-2 rounded-md border border-border bg-bg px-2 py-1.5 text-xs"
          aria-label={p.label}
        >
          <span className="font-mono text-muted">{p.label}</span>
          <span className="font-medium tabular-nums text-fg">{p.value}</span>
        </span>
      ))}
    </div>
  );
}

function AdapterNote({ text }: { text: string }) {
  return (
    <p className="rounded-md border border-border bg-accent/5 px-3 py-2 text-xs leading-relaxed text-muted">
      {text}
    </p>
  );
}
