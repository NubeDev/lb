// The Ingest page — the series explorer (data-console scope). A series list/search on the left; a
// detail pane showing the selected series' latest value + a recent-samples table; and a small manual
// "write sample" form (`ingest.write`) for testing/hand entry. Snapshot reads + manual refresh — no
// live charts (that is the dashboard). Layout + wiring only; data lives in `useIngest`.

import { useState } from "react";
import { Activity } from "lucide-react";

import { useIngest } from "./useIngest";
import type { Sample } from "@/lib/ingest/ingest.types";

interface Props {
  ws: string;
}

/** Render a sample payload by its JSON type — a scalar verbatim, a structure as compact JSON. */
function renderPayload(p: unknown): string {
  if (p === null || p === undefined) return "—";
  if (typeof p === "object") return JSON.stringify(p);
  return String(p);
}

export function IngestView({ ws }: Props) {
  const { series, selected, latest, recent, error, search, select, write } = useIngest();
  const [query, setQuery] = useState("");

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <Activity size={16} className="text-muted" />
        <h1 className="text-sm font-medium">Ingest</h1>
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      {error && (
        <div role="alert" className="border-b border-border bg-red-500/10 px-4 py-2 text-sm text-red-400">
          {error}
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        {/* Series list / search */}
        <aside className="flex w-64 flex-col border-r border-border">
          <form
            className="border-b border-border p-2"
            onSubmit={(e) => {
              e.preventDefault();
              void search(query);
            }}
          >
            <input
              aria-label="search series"
              placeholder="prefix, or kind:temperature"
              className="w-full rounded border border-border bg-panel px-2 py-1 text-sm"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </form>
          <ul className="flex-1 overflow-auto">
            {series.length === 0 && (
              <li className="px-3 py-2 text-xs text-muted">no series yet</li>
            )}
            {series.map((s) => (
              <li key={s}>
                <button
                  aria-label={`select ${s}`}
                  className={`w-full px-3 py-1.5 text-left text-sm ${
                    selected === s ? "bg-accent/15 text-accent" : "hover:bg-panel"
                  }`}
                  onClick={() => void select(s)}
                >
                  {s}
                </button>
              </li>
            ))}
          </ul>
        </aside>

        {/* Detail: latest + recent samples + manual write */}
        <div className="flex min-w-0 flex-1 flex-col overflow-auto">
          {selected ? (
            <div className="flex flex-col gap-4 p-4">
              <div>
                <div className="text-xs text-muted">latest</div>
                <div className="text-2xl font-semibold" aria-label="latest value">
                  {latest ? renderPayload(latest.payload) : "—"}
                </div>
                {latest && (
                  <div className="text-xs text-muted">seq {latest.seq} · {latest.producer}</div>
                )}
              </div>

              <div>
                <div className="mb-1 text-xs text-muted">recent samples</div>
                <table className="w-full text-left text-sm">
                  <thead>
                    <tr className="text-xs text-muted">
                      <th className="py-1 pr-4 font-medium">seq</th>
                      <th className="py-1 pr-4 font-medium">payload</th>
                      <th className="py-1 font-medium">ts</th>
                    </tr>
                  </thead>
                  <tbody>
                    {recent.map((s) => (
                      <tr key={s.seq} className="border-t border-border/50">
                        <td className="py-1 pr-4 tabular-nums">{s.seq}</td>
                        <td className="py-1 pr-4">{renderPayload(s.payload)}</td>
                        <td className="py-1 tabular-nums text-muted">{s.ts}</td>
                      </tr>
                    ))}
                    {recent.length === 0 && (
                      <tr>
                        <td colSpan={3} className="py-2 text-xs text-muted">
                          no samples yet
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>

              <WriteForm onWrite={write} defaultSeries={selected} />
            </div>
          ) : (
            <div className="p-4 text-sm text-muted">Select a series to explore, or write a sample.</div>
          )}
        </div>
      </div>
    </section>
  );
}

/** The manual "write sample" form — series, payload (typed), seq, optional label. Pushes one sample
 *  via `ingest.write`; the producer is the token's principal server-side. */
function WriteForm({
  onWrite,
  defaultSeries,
}: {
  onWrite: (s: Sample) => Promise<void>;
  defaultSeries: string;
}) {
  const [series, setSeries] = useState(defaultSeries);
  const [payload, setPayload] = useState("");
  const [seq, setSeq] = useState("1");
  const [label, setLabel] = useState("");

  return (
    <form
      aria-label="write sample"
      className="rounded border border-border bg-panel p-3"
      onSubmit={(e) => {
        e.preventDefault();
        // Parse the payload as JSON if it parses (number/bool/object), else keep it a string.
        let parsed: unknown = payload;
        try {
          parsed = JSON.parse(payload);
        } catch {
          /* a bare string stays a string */
        }
        // Parse an optional `key:value` label into a labels object.
        const labels: Record<string, unknown> = {};
        if (label.includes(":")) {
          const [k, ...rest] = label.split(":");
          labels[k] = rest.join(":");
        }
        void onWrite({
          series,
          producer: "",
          ts: Number(seq) || 0,
          seq: Number(seq) || 0,
          payload: parsed,
          labels,
        });
      }}
    >
      <div className="mb-2 text-xs font-medium text-muted">Write sample</div>
      <div className="flex flex-wrap gap-2">
        <input
          aria-label="series"
          placeholder="series"
          className="rounded border border-border bg-bg px-2 py-1 text-sm"
          value={series}
          onChange={(e) => setSeries(e.target.value)}
        />
        <input
          aria-label="payload"
          placeholder="payload (e.g. 61.4)"
          className="rounded border border-border bg-bg px-2 py-1 text-sm"
          value={payload}
          onChange={(e) => setPayload(e.target.value)}
        />
        <input
          aria-label="seq"
          placeholder="seq"
          className="w-16 rounded border border-border bg-bg px-2 py-1 text-sm"
          value={seq}
          onChange={(e) => setSeq(e.target.value)}
        />
        <input
          aria-label="label"
          placeholder="host:pi-7"
          className="rounded border border-border bg-bg px-2 py-1 text-sm"
          value={label}
          onChange={(e) => setLabel(e.target.value)}
        />
        <button
          type="submit"
          aria-label="submit sample"
          className="rounded bg-accent px-3 py-1 text-sm text-white"
        >
          Push
        </button>
      </div>
    </form>
  );
}
