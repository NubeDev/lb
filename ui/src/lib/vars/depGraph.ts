// Chained / dependent variables: derive the resolution ORDER from `$var` references (advanced-variables
// scope). A variable whose resolver `query.args` / `query.tool` (or a static form) references `$other`
// must resolve AFTER `$other`, so `$other`'s selection can be interpolated into its resolver call. Grafana
// derives this graph the same way — from the query string, not an explicit `dependsOn` edge. Pure TS.
//
// We reuse `extractVarNamesDeep` (the shipped deep parser) over each variable's resolver + static forms,
// keep only edges to OTHER user variables in the set (built-ins and unknown names are ignored), and
// Kahn-topologically order. A CYCLE fails honestly: `orderVariables` throws a `VarCycleError` naming the
// members — never a hang, never a silent partial order.

import type { Variable } from "./types";
import { extractVarNamesDeep, isBuiltinName } from "./parse";

/** Thrown when the dependency graph has a cycle. `cycle` names the variables still unresolved. */
export class VarCycleError extends Error {
  cycle: string[];
  constructor(cycle: string[]) {
    super(`variable dependency cycle: ${cycle.join(" → ")}`);
    this.name = "VarCycleError";
    this.cycle = cycle;
  }
}

/** Every `$var` reference in a variable's resolver + static forms (built-ins excluded). */
export function variableDeps(v: Variable): string[] {
  const forms: unknown[] = [v.query?.tool, v.query?.args, v.custom, v.text, v.const, v.interval, v.regex];
  const names = extractVarNamesDeep(forms);
  return names.filter((n) => !isBuiltinName(n));
}

/** The dependency edges of `variables`: `name → [deps]`, restricted to names present in the set. */
export function buildDepGraph(variables: Variable[]): Map<string, string[]> {
  const present = new Set(variables.map((v) => v.name));
  const graph = new Map<string, string[]>();
  for (const v of variables) {
    graph.set(
      v.name,
      variableDeps(v).filter((d) => present.has(d) && d !== v.name),
    );
  }
  return graph;
}

/** Topologically order `variables` so each resolves after its dependencies. Stable on the input order for
 *  independent variables. Throws `VarCycleError` on a cycle (fails honestly, never hangs). */
export function orderVariables(variables: Variable[]): Variable[] {
  const graph = buildDepGraph(variables);
  const byName = new Map(variables.map((v) => [v.name, v]));
  const indegree = new Map<string, number>();
  const dependents = new Map<string, string[]>(); // dep → [names that depend on it]
  for (const v of variables) indegree.set(v.name, 0);
  for (const [name, deps] of graph) {
    indegree.set(name, deps.length);
    for (const d of deps) dependents.set(d, [...(dependents.get(d) ?? []), name]);
  }
  // Seed the queue with zero-indegree nodes IN INPUT ORDER (stable for independents).
  const queue = variables.filter((v) => (indegree.get(v.name) ?? 0) === 0).map((v) => v.name);
  const ordered: Variable[] = [];
  while (queue.length) {
    const name = queue.shift()!;
    ordered.push(byName.get(name)!);
    for (const dep of dependents.get(name) ?? []) {
      const n = (indegree.get(dep) ?? 0) - 1;
      indegree.set(dep, n);
      if (n === 0) queue.push(dep);
    }
  }
  if (ordered.length !== variables.length) {
    const stuck = variables.map((v) => v.name).filter((n) => !ordered.some((o) => o.name === n));
    throw new VarCycleError(stuck);
  }
  return ordered;
}

/** The transitive set of variable names that depend (directly or not) on `changed` — the ONLY variables
 *  that must re-resolve when `changed`'s selection changes (bounded fan-out, not the whole graph). */
export function dependentsOf(variables: Variable[], changed: string): Set<string> {
  const graph = buildDepGraph(variables);
  const rev = new Map<string, string[]>();
  for (const [name, deps] of graph) for (const d of deps) rev.set(d, [...(rev.get(d) ?? []), name]);
  const out = new Set<string>();
  const stack = [changed];
  while (stack.length) {
    const cur = stack.pop()!;
    for (const dep of rev.get(cur) ?? []) {
      if (!out.has(dep)) {
        out.add(dep);
        stack.push(dep);
      }
    }
  }
  return out;
}
