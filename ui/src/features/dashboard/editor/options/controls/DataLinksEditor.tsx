// The data-links editor (editor-parity scope, step 2) — authors `fieldConfig.defaults.links`
// (Grafana's `DataLink[]`: title + url + open-in-new-tab). One responsibility: edit the data-link list.

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import type { DataLink } from "@/lib/dashboard";

interface Props {
  value: DataLink[] | undefined;
  onChange: (next: DataLink[] | undefined) => void;
}

export function DataLinksEditor({ value, onChange }: Props) {
  const links = value ?? [];
  const write = (next: DataLink[]) => onChange(next.length ? next : undefined);
  const setLink = (idx: number, next: Partial<DataLink>) =>
    write(links.map((l, i) => (i === idx ? { ...l, ...next } : l)));
  const remove = (idx: number) => write(links.filter((_, i) => i !== idx));
  const add = () => write([...links, { title: "", url: "" }]);

  return (
    <div className="grid gap-2" aria-label="data links editor">
      {links.length === 0 && <p className="text-xs text-muted">No data links.</p>}
      {links.map((l, idx) => (
        <div key={idx} className="grid gap-1.5 rounded-md border border-border bg-bg p-2" aria-label={`data link ${idx}`}>
          <div className="flex items-center gap-1.5">
            <Input aria-label={`data link ${idx} title`} className="h-7 flex-1 text-xs" placeholder="title" value={l.title} onChange={(e) => setLink(idx, { title: e.target.value })} />
            <Button variant="ghost" aria-label={`remove data link ${idx}`} className="h-auto px-1.5 text-muted hover:text-red-500" onClick={() => remove(idx)}>
              ×
            </Button>
          </div>
          <Input aria-label={`data link ${idx} url`} className="h-7 w-full text-xs" placeholder="https://… (supports ${__value.text})" value={l.url} onChange={(e) => setLink(idx, { url: e.target.value })} />
          <label className="flex items-center gap-2 text-xs text-muted">
            <Checkbox aria-label={`data link ${idx} new tab`} checked={!!l.targetBlank} onChange={(e) => setLink(idx, { targetBlank: e.target.checked || undefined })} />
            Open in new tab
          </label>
        </div>
      ))}
      <Button variant="outline" size="sm" aria-label="add data link" className="h-auto justify-self-start px-2 py-0.5 text-[11px] text-muted hover:text-fg" onClick={add}>
        + Add data link
      </Button>
    </div>
  );
}
