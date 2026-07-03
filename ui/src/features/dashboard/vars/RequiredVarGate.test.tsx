// Unit: the required-variable gate (reusable-pages scope). Proves the pure `unboundRequiredVars`
// predicate (which decides whether the grid fires) across the binding-precedence cases, and that the
// gate renders an honest "select a <label>" prompt. The end-to-end "cells do NOT fire while unbound"
// invariant is asserted against the real gateway in the gateway test — here we lock the predicate.

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import type { Variable, VarScope } from "@/lib/vars";
import { RequiredVarGate, unboundRequiredVars } from "./RequiredVarGate";

const required = (name: string): Variable => ({ name, type: "query", required: true });
const optional = (name: string): Variable => ({ name, type: "query" });

function scope(values: Record<string, string | string[]>): VarScope {
  return { values, builtins: {} };
}

describe("unboundRequiredVars (binding precedence)", () => {
  it("flags a required var with no value bound (→ gate)", () => {
    const out = unboundRequiredVars([required("site")], scope({}));
    expect(out.map((v) => v.name)).toEqual(["site"]);
  });

  it("clears once the URL/default supplies a value (→ grid fires)", () => {
    const out = unboundRequiredVars([required("site")], scope({ site: "plant-1" }));
    expect(out).toEqual([]);
  });

  it("never gates on a non-required variable, bound or not", () => {
    const out = unboundRequiredVars([optional("env")], scope({}));
    expect(out).toEqual([]);
  });

  it("treats an empty string / empty multi-list as still unbound", () => {
    expect(unboundRequiredVars([required("site")], scope({ site: "" })).length).toBe(1);
    expect(unboundRequiredVars([required("site")], scope({ site: [] })).length).toBe(1);
    expect(unboundRequiredVars([required("site")], scope({ site: ["plant-1"] })).length).toBe(0);
  });

  it("reports every unbound required parameter", () => {
    const out = unboundRequiredVars(
      [required("site"), required("floor"), optional("env")],
      scope({ site: "plant-1" }),
    );
    expect(out.map((v) => v.name)).toEqual(["floor"]);
  });
});

describe("RequiredVarGate render", () => {
  it("names the parameter to pick (using its label)", () => {
    render(<RequiredVarGate unbound={[{ name: "site", label: "Site", type: "query", required: true }]} />);
    expect(screen.getByTestId("required-var-gate")).toBeInTheDocument();
    expect(screen.getByText(/Select a Site to load this page\./)).toBeInTheDocument();
  });
});
