import { useCallback, useState } from "react";

/** Shared async-call plumbing every per-verb hook composes: loading/error/data around one bridge
 *  call. Not itself a verb — `useConnections.ts`/`useCreateConnection.ts`/etc. each own their own
 *  verb and args shape; this only avoids re-writing the same three `useState`s in every one of them. */
export function useAsyncAction<Args extends unknown[], T>(fn: (...args: Args) => Promise<T>) {
  const [data, setData] = useState<T | undefined>(undefined);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>(undefined);

  const run = useCallback(
    async (...args: Args) => {
      setLoading(true);
      setError(undefined);
      try {
        const result = await fn(...args);
        setData(result);
        return result;
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        throw e;
      } finally {
        setLoading(false);
      }
    },
    [fn],
  );

  return { data, loading, error, run, setData };
}
