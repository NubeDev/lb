// A labeled settings row (label + help on the left, control on the right) — the one layout primitive
// the Settings tabs share so every axis/control lines up identically. Presentation only.

import type { ReactNode } from "react";

interface FieldProps {
  label: string;
  htmlFor?: string;
  help?: string;
  children: ReactNode;
}

export function Field({ label, htmlFor, help, children }: FieldProps) {
  return (
    <div className="grid grid-cols-1 items-start gap-1.5 border-b border-border/60 py-3 sm:grid-cols-[minmax(0,14rem)_minmax(0,1fr)] sm:gap-4">
      <div className="min-w-0">
        <label htmlFor={htmlFor} className="text-xs font-medium text-fg">
          {label}
        </label>
        {help && <p className="mt-0.5 text-[11px] leading-snug text-muted">{help}</p>}
      </div>
      <div className="min-w-0">{children}</div>
    </div>
  );
}

/** A titled group of fields within a tab. */
export function FieldGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mb-6">
      <h2 className="mb-1 text-xs font-semibold uppercase tracking-wide text-muted">{title}</h2>
      <div>{children}</div>
    </section>
  );
}
