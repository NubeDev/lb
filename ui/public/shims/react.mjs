// Importmap shim — re-exports the host's bundled React via the global the shell publishes
// (`ui/src/features/ext-host/singletons.ts`) before any extension `remoteEntry.js` is dynamic-imported.
// Extension bundles externalise `react`; their bare `import "react"` resolves through the host
// importmap (index.html <head>) to this module, so they bind to the host's SINGLE React instance —
// required for hooks/context to work across the host↔extension boundary (two React copies break them).
const R = /** @type {any} */ (globalThis).__lbReact;
if (!R) {
  throw new Error(
    "lb react-shim: globalThis.__lbReact is unset. The shell did not publish React before the extension bundle was imported."
  );
}
export default R.default ?? R;
export const {
  Children,
  Component,
  Fragment,
  Profiler,
  PureComponent,
  StrictMode,
  Suspense,
  cloneElement,
  createContext,
  createElement,
  createRef,
  forwardRef,
  isValidElement,
  lazy,
  memo,
  startTransition,
  use,
  useActionState,
  useCallback,
  useContext,
  useDebugValue,
  useDeferredValue,
  useEffect,
  useId,
  useImperativeHandle,
  useInsertionEffect,
  useLayoutEffect,
  useMemo,
  useOptimistic,
  useReducer,
  useRef,
  useState,
  useSyncExternalStore,
  useTransition,
  version,
} = R;
