// Unit tests for the shell-side VarScope resolution (widget-config-vars Slice 3). The scope = URL
// selection (falling back to definition defaults) + built-ins from the session + time range. Pure
// resolution; the session is stubbed via the real session store (no mock backend — a real Session).

import { describe, expect, it, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";

import { useVarScope } from "./useVarScope";
import { setSession } from "@/lib/session/session.store";
import type { Variable } from "@/lib/vars";
import type { DashboardSearch } from "@/features/routing/search";

const search = (over: Partial<DashboardSearch>): DashboardSearch => ({
  from: "2026-01-01",
  to: "2026-01-31",
  ...over,
});

beforeEach(() => setSession(null));

describe("useVarScope", () => {
  it("resolves a URL selection into values + built-ins from the session/range", () => {
    setSession({ token: "t", principal: "user:ada", workspace: "acme" });
    const vars: Variable[] = [{ name: "host", type: "query", query: { tool: "store.query" }, multi: true }];
    const { result } = renderHook(() =>
      useVarScope(vars, search({ "var-host": ["web01", "web02"] }), "ops", "acme"),
    );
    expect(result.current.values.host).toEqual(["web01", "web02"]);
    // Built-ins from the verified session + range (un-spoofable — from the token, not a cell).
    expect(result.current.builtins["__user.login"]).toBe("ada");
    expect(result.current.builtins["__workspace"]).toBe("acme");
    expect(result.current.builtins["__dashboard"]).toBe("ops");
    expect(result.current.builtins["__from"]).toBe(String(Date.parse("2026-01-01T00:00:00.000Z")));
  });

  it("falls back to a const/text/interval default when no selection", () => {
    setSession({ token: "t", principal: "user:bob", workspace: "acme" });
    const vars: Variable[] = [
      { name: "k", type: "const", const: "fixed" },
      { name: "q", type: "text", text: "hello" },
      { name: "step", type: "interval", interval: ["1m", "5m"] },
    ];
    const { result } = renderHook(() => useVarScope(vars, search({}), "ops", "acme"));
    expect(result.current.values.k).toBe("fixed");
    expect(result.current.values.q).toBe("hello");
    expect(result.current.values.step).toBe("1m");
    // The interval built-in tracks the first interval variable's resolved value.
    expect(result.current.builtins["__interval"]).toBe("1m");
  });

  it("leaves an unselected query/custom variable out of values (interpolate keeps it literal)", () => {
    setSession({ token: "t", principal: "user:ada", workspace: "acme" });
    const vars: Variable[] = [{ name: "host", type: "query", query: { tool: "store.query" } }];
    const { result } = renderHook(() => useVarScope(vars, search({}), "ops", "acme"));
    expect(result.current.values.host).toBeUndefined();
  });
});
