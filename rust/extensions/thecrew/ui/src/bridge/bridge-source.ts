// The bridge-backed ValueSource — the seam swap the playground was built for
// (thecrew-extension-scope.md §Goals): `data/value-source.ts` stops being fed by the simulator and
// is fed by the HOST BRIDGE under the viewer's grant. One MULTIPLEXER, not per-shape subscriptions
// (parent scope's "binding fan-out" risk): collect + dedupe every bound channel, one backfill +
// one live subscription per series, fan out to all subscribers. A 200-prop page opens N-series
// subscriptions, never N-prop.
//
// A channel === a series name (the playground's ValueRef.channel is the framework's series id).
// Backfill: `series.latest` → the newest sample's payload. Live: `bridge.watch("series.watch")`
// (the shipped SSE) WHEN the bridge offers watch (the widget tier); otherwise POLL `series.latest`
// (the frozen PAGE bridge is call-only — parent scope Open question 2: 2s polling is fine for
// phases 1–2). A denied series (viewer lacks `series.read`/`watch`) resolves to `null` — the bound
// shape renders its no-access state, never a crash (deny path, testing plan).

import type { ValueSource, Unsubscribe } from "../data/value-source";
import type { Bridge, WidgetBridge } from "./contract";

/** A `series.latest` reply envelope: `{ sample: { payload } | null }`. */
interface LatestReply {
  sample?: { payload?: unknown } | null;
}

const POLL_MS = 2000; // parent scope Open question 2: polling cadence when the bridge has no watch.

/**
 * Build a ValueSource over the bridge. `channels` is the deduped set the scene binds (collected by
 * the caller from the doc's `bind` maps); `channels()` returns it for the PropertyRail picker. The
 * multiplexer keeps one upstream per series regardless of how many props/shapes bind it.
 */
export function createBridgeSource(
  bridge: Bridge | WidgetBridge,
  channels: string[],
): ValueSource {
  const known = [...new Set(channels)];
  // Per-series fan-out state: subscribers, last value, and the teardown for the live/poll upstream.
  interface Entry {
    subs: Set<(v: unknown) => void>;
    value: unknown;
    stop?: () => void;
  }
  const entries = new Map<string, Entry>();

  const watch = "watch" in bridge ? bridge.watch : undefined;

  function extract(reply: LatestReply | null | undefined): unknown {
    const sample = reply?.sample;
    return sample && typeof sample === "object" ? (sample.payload ?? null) : null;
  }

  function push(series: string, value: unknown) {
    const e = entries.get(series);
    if (!e || Object.is(e.value, value)) return;
    e.value = value;
    for (const fn of e.subs) fn(value);
  }

  /** Open ONE upstream for `series`: backfill via series.latest, then watch-or-poll. Deny → null. */
  function open(series: string, e: Entry) {
    let stopped = false;
    // Backfill once with the latest committed sample.
    bridge
      .call<LatestReply>("series.latest", { series })
      .then((r) => {
        if (!stopped) push(series, extract(r));
      })
      .catch(() => {
        // Denied or no value — the shape shows its no-access/empty state, never a crash.
        if (!stopped) push(series, null);
      });

    if (watch) {
      // Live via the shipped SSE. `series.watch` events are BARE samples (no envelope).
      const unwatch = watch("series.watch", { series }, (event) => {
        if (stopped) return;
        const sample = event as { payload?: unknown } | null;
        if (sample && typeof sample === "object" && "payload" in sample) {
          push(series, sample.payload ?? null);
        }
      });
      e.stop = () => {
        stopped = true;
        unwatch();
      };
    } else {
      // Poll fallback (call-only page bridge). One timer per series; cleared on teardown.
      const timer = setInterval(() => {
        bridge
          .call<LatestReply>("series.latest", { series })
          .then((r) => !stopped && push(series, extract(r)))
          .catch(() => !stopped && push(series, null));
      }, POLL_MS);
      e.stop = () => {
        stopped = true;
        clearInterval(timer);
      };
    }
  }

  return {
    get(channel: string): unknown {
      return entries.get(channel)?.value ?? null;
    },

    subscribe(channel: string, onValue: (value: unknown) => void): Unsubscribe {
      let e = entries.get(channel);
      if (!e) {
        e = { subs: new Set(), value: null };
        entries.set(channel, e);
        open(channel, e); // first subscriber opens the single upstream
      }
      e.subs.add(onValue);
      onValue(e.value); // fire immediately with the current value (seam contract)
      return () => {
        const entry = entries.get(channel);
        if (!entry) return;
        entry.subs.delete(onValue);
        if (entry.subs.size === 0) {
          entry.stop?.(); // last subscriber gone → close the upstream (stateless eviction)
          entries.delete(channel);
        }
      };
    },

    channels(): string[] {
      return [...known];
    },
  };
}

/** Collect + dedupe every channel a scene doc binds — the multiplexer's input. */
export function collectChannels(doc: {
  shapes: Record<string, { bind?: Record<string, { channel: string }> }>;
}): string[] {
  const set = new Set<string>();
  for (const shape of Object.values(doc.shapes)) {
    if (!shape.bind) continue;
    for (const ref of Object.values(shape.bind)) {
      if (ref?.channel) set.add(ref.channel);
    }
  }
  return [...set];
}
