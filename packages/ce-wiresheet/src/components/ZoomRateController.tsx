import { useEffect, useRef } from "react";
import { useStoreApi } from "@xyflow/react";

import { metrics } from "../lib/instrumentation";

// Auto-adjusts the WS push rate from TWO signals:
//   1. Zoom — sets the CEILING. Zoomed out you can't read individual values, so
//      a high tick rate just burns bandwidth + re-renders; zoomed in you want
//      the full rate.
//   2. Render performance — scales the effective rate DOWN from that ceiling
//      when the render loop can't keep up, even at full zoom. The value stream
//      drives re-renders, so backing it off is the most direct way to recover
//      FPS. Recovers gradually once frames are cheap again (AIMD: back off fast,
//      ramp up slow — avoids oscillation).
//
// Renders nothing. Mounted inside the ReactFlow provider so it can read the
// live transform (via the store API, without re-rendering App).

// Bucket thresholds: [minZoom, hz]. The chosen rate is the hz of the highest
// threshold whose minZoom <= current zoom. Tuned so a normal "see the graph"
// zoom already reaches the ceiling rather than topping out low. No bucket above
// ~15 Hz: the engine's event-poll floor caps effective cadence there anyway.
const BUCKETS: Array<{ minZoom: number; hz: number }> = [
  { minZoom: 0, hz: 1 }, // far out — can't read anything
  { minZoom: 0.3, hz: 4 }, // shapes legible, values not really
  { minZoom: 0.55, hz: 10 }, // readable → full (ceiling) rate
  { minZoom: 1.3, hz: 15 }, // deep zoom on a few nodes
];

function rateForZoom(zoom: number): number {
  let hz = BUCKETS[0].hz;
  for (const b of BUCKETS) {
    if (zoom >= b.minZoom) hz = b.hz;
  }
  return hz;
}

// Feedback loop tuning.
const POLL_MS = 1000; // re-evaluate (zoom + perf) once per second
const LOW_FPS = 30; // below this → back off the rate
const GOOD_FPS = 50; // above this → ramp the rate back up
const MIN_HZ = 1; // the engine clamps tickHz to ≥ 1, so this is the floor
const BACKOFF = 0.5; // multiplicative decrease (halve) when struggling
const RECOVER = 0.25; // additive increase (of the 0..1 scale) when healthy

export function ZoomRateController({
  enabled,
  setRate,
}: {
  enabled: boolean;
  setRate: (hz: number) => void;
}) {
  const store = useStoreApi();
  // Scale factor in (0, 1] applied to the zoom ceiling. Held in a ref so the
  // feedback loop carries state across polls without re-rendering.
  const scale = useRef(1);
  const lastSent = useRef<number | null>(null);

  useEffect(() => {
    if (!enabled) {
      lastSent.current = null;
      scale.current = 1;
      return;
    }
    const evaluate = () => {
      const zoom = store.getState().transform[2];
      const ceiling = rateForZoom(zoom);
      const fps = metrics.fps;

      // AIMD on the scale. `fps === 0` means instrumentation hasn't produced a
      // sample yet — hold rather than guess.
      if (fps > 0) {
        if (fps < LOW_FPS) scale.current = Math.max(0.02, scale.current * BACKOFF);
        else if (fps > GOOD_FPS) scale.current = Math.min(1, scale.current + RECOVER);
      }

      const want = Math.max(MIN_HZ, Math.min(ceiling, Math.round(ceiling * scale.current)));
      if (want !== lastSent.current) {
        lastSent.current = want;
        setRate(want);
      }
    };
    evaluate(); // apply immediately on enable
    const id = window.setInterval(evaluate, POLL_MS);
    return () => window.clearInterval(id);
  }, [enabled, setRate, store]);

  return null;
}
