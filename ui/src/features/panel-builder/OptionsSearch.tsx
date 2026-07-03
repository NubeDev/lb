// The options search box (viz panel-editor scope, Resolved decision: "Options search filters option
// labels across all tabs"). Phase 1: a controlled input that surfaces a query string; the PanelEditor
// uses it to highlight/scroll matching option groups. Fuzzy/synonym search is a named follow-up. One
// responsibility: the search input.

import { Search } from "lucide-react";

import { Input } from "@/components/ui/input";

interface Props {
  value: string;
  onChange: (q: string) => void;
}

export function OptionsSearch({ value, onChange }: Props) {
  return (
    <div className="relative">
      <Search size={13} className="absolute left-2 top-1/2 -translate-y-1/2 text-muted" aria-hidden />
      <Input
        aria-label="search options"
        className="h-8 pl-7 text-xs"
        placeholder="Search options"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}
