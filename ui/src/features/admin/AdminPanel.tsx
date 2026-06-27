// The shared admin panel frame (admin-console redesign) — one consistent header (icon · title ·
// optional action · workspace badge) + a scrollable body, so every admin tab reads like an admin
// surface and NOT a chat window (no message-composer pinned to the bottom; create lives in the
// header action). Markup only. One responsibility per file (FILE-LAYOUT).

import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

interface Props {
  icon: LucideIcon;
  title: string;
  ws: string;
  /** Optional header-right control (e.g. a "New …" button) — replaces the chat composer. */
  action?: ReactNode;
  error?: string | null;
  children: ReactNode;
}

export function AdminPanel({ icon: Icon, title, ws, action, error, children }: Props) {
  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <Icon size={16} className="text-muted" />
        <h1 className="text-sm font-medium">{title}</h1>
        <div className="ml-auto flex items-center gap-3">
          {action}
          <span className="text-xs text-muted">{ws}</span>
        </div>
      </header>
      {error && (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error}
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
    </section>
  );
}
