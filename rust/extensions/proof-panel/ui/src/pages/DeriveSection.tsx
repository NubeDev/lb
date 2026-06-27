import { Sparkles } from "lucide-react";

import { Button, Card, CardContent, CardHeader, CardTitle } from "@/components/ui";
import { useDerive } from "@/data/useDerive";
import { useSeriesLatest } from "@/data/useSeriesLatest";

/** The series the guest's `proof.derive` reads from and writes to (must mirror the wasm guest). */
const SOURCE_SERIES = "proof.demo";
const DERIVED_SERIES = "proof.derived";

/** The host-callback proof, made visible (host-callback scope). "Run derive" invokes the extension's
 *  OWN backend tool `proof-panel.proof.derive`: the wasm guest reads the latest `proof.demo` and writes
 *  `proof.derived = value*2` — entirely through the host-mediated `host.call-tool` callback, under the
 *  guest's delegated `caller ∩ grant` authority. The card then reads `proof.derived` back over the bridge
 *  to prove the guest's WRITE really committed (the page → guest → host → store → page full loop). A
 *  denied callback shows the honest error, never a fabricated value. */
export function DeriveSection() {
  const deriver = useDerive();
  const derived = useSeriesLatest(DERIVED_SERIES);

  async function run() {
    const value = await deriver.derive();
    if (value !== null) derived.refresh(); // re-read the committed derived series
  }

  const busy = deriver.state.status === "deriving";

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle>Host-callback derive</CardTitle>
        <Button size="sm" aria-label="run derive" onClick={run} disabled={busy}>
          <Sparkles className="h-3.5 w-3.5" aria-hidden />
          {busy ? "Deriving…" : "Run derive"}
        </Button>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <p className="text-xs text-muted">
          Calls the guest tool <code>proof.derive</code>, which reads{" "}
          <span className="text-fg">{SOURCE_SERIES}</span> and writes{" "}
          <span className="text-fg">{DERIVED_SERIES}</span> = value×2 through the host callback — a
          wasm guest doing real platform work.
        </p>

        {deriver.state.status === "error" && (
          <p className="text-accent">Could not derive: {deriver.state.error}</p>
        )}
        {deriver.state.status === "ok" && (
          <p className="text-fg" data-testid="derive-result">
            Derived {deriver.state.derived} from seq {deriver.state.sourceSeq}
          </p>
        )}

        <div className="rounded-md border border-border p-3">
          <p className="mb-1 text-xs text-muted">
            Latest committed · <span className="text-fg">{DERIVED_SERIES}</span>
          </p>
          {derived.state.status === "loading" && <p>Reading derived…</p>}
          {derived.state.status === "error" && (
            <p className="text-accent">Could not read derived: {derived.state.error}</p>
          )}
          {derived.state.status === "ready" && derived.state.data === null && (
            <p>No derived sample yet — click Run derive.</p>
          )}
          {derived.state.status === "ready" && derived.state.data !== null && (
            <p className="text-fg" data-testid="derived-latest">
              seq {String(derived.state.data.seq)} · value {String(derived.state.data.payload)}
            </p>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
