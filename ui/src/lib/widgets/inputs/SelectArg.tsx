// The `select` arg widget (channel rich responses scope) — an `x-lb-widget:"select"` arg renders a
// dropdown. Options come from EITHER a static `options:[…]` hint OR a `source:"<tool>"` catalog tool
// whose rows become options (fetched through the gated bridge, mirroring how RuntimeArg reads
// `agent.runtimes`). For reminders the `action_kind` arg is static-options (no source). RENDER + the
// one source fetch only (FILE-LAYOUT). A source row's option value is its `id`/`name`/`value`/first
// string field; the fetch degrades to the static options (or empty) on deny — never a fake list.

import { useEffect, useMemo, useState } from "react";

import { Select } from "@/components/ui/select";
import { makeWidgetBridge } from "@/features/dashboard/builder/widgetBridge";

interface Props {
  /** The picked option value (empty until chosen / the first option lands). */
  value: string;
  onChange: (value: string) => void;
  /** Static option list (used verbatim when present). */
  options?: string[];
  /** A catalog tool whose rows become options (fetched via the bridge, gated). */
  source?: string;
}

/** Pull an option value off a source row — the first of `value`/`id`/`name`, else the first string field. */
function optionOf(row: unknown): string | null {
  if (typeof row === "string") return row;
  if (!row || typeof row !== "object") return null;
  const o = row as Record<string, unknown>;
  for (const k of ["value", "id", "name"]) if (typeof o[k] === "string") return o[k] as string;
  const first = Object.values(o).find((v) => typeof v === "string");
  return typeof first === "string" ? first : null;
}

export function SelectArg({ value, onChange, options, source }: Props) {
  const [fetched, setFetched] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Fetch the source options once (gated through a bridge leashed to just this catalog tool). No source
  // → the static `options` are the whole list; the effect is a no-op.
  useEffect(() => {
    if (!source) return;
    let live = true;
    const bridge = makeWidgetBridge([source]);
    bridge
      .call<unknown>(source, {})
      .then((result) => {
        if (!live) return;
        const rows = Array.isArray(result)
          ? result
          : (result as { items?: unknown[] })?.items ?? [];
        setFetched((rows as unknown[]).map(optionOf).filter((v): v is string => v !== null));
      })
      .catch((e) => live && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      live = false;
    };
  }, [source]);

  // The effective option list — the fetched source rows, or the static options (memoized so it is a
  // stable dep for the preselect effect below).
  const list = useMemo(() => (source ? fetched ?? [] : options ?? []), [source, fetched, options]);
  const selected = value || list[0] || "";

  // Preselect the first option so a required select is never unset (mirrors RuntimeArg's default).
  useEffect(() => {
    if (!value && list.length > 0) onChange(list[0]);
  }, [value, list, onChange]);

  return (
    <div className="border-t border-border bg-panel p-2" aria-label="select picker">
      <Select
        aria-label="select"
        value={selected}
        onChange={(e) => onChange(e.target.value)}
      >
        {list.length === 0 && <option value="">(no options)</option>}
        {list.map((o) => (
          <option key={o} value={o}>
            {o}
          </option>
        ))}
      </Select>
      {error && (
        <div role="alert" className="mt-1 text-xs text-destructive">
          {error}
        </div>
      )}
    </div>
  );
}
