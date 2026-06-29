// The options search box (viz panel-editor scope, Resolved decision: "Options search filters option
// labels across all tabs"). Phase 1: a controlled input that surfaces a query string; the PanelEditor
// uses it to highlight/scroll matching option groups. Fuzzy/synonym search is a named follow-up. One
// responsibility: the search input.

import { Search } from "lucide-react";

interface Props {
  value: string;
  onChange: (q: string) => void;
}

export function OptionsSearch({ value, onChange }: Props) {
  return (
    <div className="relative">
      <Search size={13} className="absolute left-2 top-1/2 -translate-y-1/2 text-muted" aria-hidden />
      {/* eslint-disable-next-line no-restricted-syntax -- styled native input (no shadcn Input search variant) */}
      <input
        aria-label="search options"
        className="h-8 w-full rounded-md border border-border bg-bg pl-7 pr-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20"
        placeholder="Search options"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}
