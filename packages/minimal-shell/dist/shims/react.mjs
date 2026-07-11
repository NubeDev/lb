// React singleton shim — re-exports from globalThis.__lbReact (published by the shell before
// any remote loads). This is the federation contract: the shell owns ONE React instance; an
// extension's bare `import "react"` resolves here via the import map, NOT a second copy.
const R = (globalThis).__lbReact;
if (!R) throw new Error("minimal-shell: globalThis.__lbReact not set — load /src/singletons.ts first");
export default R.default ?? R;
export const { Children, cloneElement, createContext, createElement, createRef, forwardRef, Fragment, isValidElement, lazy, memo, startTransition, useEffect, useId, useImperativeHandle, useInsertionEffect, useLayoutEffect, useMemo, useReducer, useRef, useState, useSyncExternalStore, useTransition, version } = R;
