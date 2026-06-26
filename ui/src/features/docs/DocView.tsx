// The doc view — opens one shared doc asset for the current user. Layout + wiring only; data
// lives in useDoc (FILE-LAYOUT). This is the UI face of the S4 sharing story: a team member or
// channel-linked reader sees the content; a non-member sees the node's "denied" — the same gate
// the Rust `assets_doc_test` proves, surfaced to the user.

import { FileText } from "lucide-react";

import { useDoc } from "./useDoc";

interface Props {
  ws: string;
  id: string;
  /** The current user's principal (the demo session identity until real login lands). */
  author: string;
}

export function DocView({ ws, id, author }: Props) {
  const { doc, loading, error } = useDoc(ws, id, author);

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <FileText size={16} className="text-muted" />
        <h1 className="text-sm font-medium">{doc?.title ?? id}</h1>
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      {loading ? (
        <div className="flex flex-1 items-center justify-center text-sm text-muted">Loading…</div>
      ) : error ? (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error === "denied" ? "You don't have access to this document." : error}
        </div>
      ) : (
        <article className="flex-1 whitespace-pre-wrap px-4 py-3 text-sm">{doc?.content}</article>
      )}
    </section>
  );
}
