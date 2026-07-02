// The React face of the ValueSource seam: context + subscribe hooks. Shape
// components never see this — ShapeNode resolves `shape.bind` here and passes
// plain values down (thecrew-scope.md §reuse #2). One file, one responsibility:
// turning the seam's subscriptions into React state.

import { createContext, useContext, useEffect, useState } from "react";
import type { ValueRef } from "../scene/scene.types";
import type { ValueSource } from "./value-source";
import { createEmptySource } from "./empty-source";

/** Default = the inert null source (the simulator is deleted in the lift — CLAUDE §9). The mount
 *  shell provides the real bridge-backed source; tests provide a seeded/stub one. */
export const ValueSourceContext = createContext<ValueSource>(createEmptySource());

export function useValueSource(): ValueSource {
  return useContext(ValueSourceContext);
}

/** Resolve a shape's bind map → { propName: liveValue }. */
export function useValues(bind?: Record<string, ValueRef>): Record<string, unknown> {
  const source = useValueSource();
  const [values, setValues] = useState<Record<string, unknown>>({});
  // re-subscribe only when the channel set actually changes
  const key = bind ? JSON.stringify(bind) : "";

  useEffect(() => {
    if (!bind || Object.keys(bind).length === 0) {
      setValues({});
      return;
    }
    const unsubs = Object.entries(bind).map(([prop, ref]) =>
      source.subscribe(ref.channel, (value) =>
        setValues((prev) => (Object.is(prev[prop], value) ? prev : { ...prev, [prop]: value })),
      ),
    );
    return () => unsubs.forEach((u) => u());
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, source]);

  return values;
}

/** Single-channel variant — the PropertyRail's live value beside each binding. */
export function useChannelValue(channel: string | null | undefined): unknown {
  const source = useValueSource();
  const [value, setValue] = useState<unknown>(() => (channel ? source.get(channel) : null));
  useEffect(() => {
    if (!channel) {
      setValue(null);
      return;
    }
    return source.subscribe(channel, setValue);
  }, [channel, source]);
  return channel ? value : null;
}
