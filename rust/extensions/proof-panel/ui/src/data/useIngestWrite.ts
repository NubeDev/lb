import { useCallback, useState } from "react";

import { useBridge } from "@/app/useBridge";

/** The status of a one-shot write action — idle until the user clicks, then writing → ok / error. */
export type WriteState =
  | { status: "idle" }
  | { status: "writing" }
  | { status: "ok"; accepted: number }
  | { status: "error"; error: string };

/** One sample to write, matching the host `Sample` wire shape `ingest.write` accepts. */
export interface SampleInput {
  series: string;
  ts: number;
  seq: number;
  value: unknown;
}

/** Write samples through the granted `ingest.write` verb — the demo's headline action: the page CREATES
 *  the data it then reads back. Returns the count accepted on success so the caller can chain a read.
 *  A rejected call (out of scope / denied) surfaces honestly as an error state, never a fabricated ok. */
export function useIngestWrite() {
  const bridge = useBridge();
  const [state, setState] = useState<WriteState>({ status: "idle" });

  const write = useCallback(
    async (samples: SampleInput[]): Promise<number | null> => {
      setState({ status: "writing" });
      try {
        const res = await bridge.call<{ accepted: number }>("ingest.write", {
          // The host maps these into real `Sample`s; producer is forced to the authenticated principal.
          samples: samples.map((s) => ({
            series: s.series,
            producer: "",
            ts: s.ts,
            seq: s.seq,
            payload: s.value,
            labels: null,
            qos: "best-effort",
          })),
        });
        const accepted = res?.accepted ?? 0;
        setState({ status: "ok", accepted });
        return accepted;
      } catch (e: unknown) {
        setState({ status: "error", error: e instanceof Error ? e.message : String(e) });
        return null;
      }
    },
    [bridge],
  );

  return { state, write };
}
