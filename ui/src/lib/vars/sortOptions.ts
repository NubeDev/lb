// Sort a variable's option list (advanced-variables scope). Grafana's sort modes over `{text, value}` —
// sorting is by the DISPLAY TEXT (what the user reads in the dropdown), matching Grafana. `none` keeps
// insertion order. Pure TS, no React.

import type { VariableOption, VariableSort } from "./types";

/** Natural-ish numeric compare: parse leading numbers, else fall back to string. */
function numCompare(a: string, b: string): number {
  const na = parseFloat(a);
  const nb = parseFloat(b);
  const aNum = !Number.isNaN(na);
  const bNum = !Number.isNaN(nb);
  if (aNum && bNum && na !== nb) return na - nb;
  if (aNum && !bNum) return -1;
  if (!aNum && bNum) return 1;
  return a.localeCompare(b);
}

/** Return a NEW sorted option list per `sort` (default `none` = unchanged). Stable for equal keys. */
export function sortOptions(options: VariableOption[], sort: VariableSort | undefined): VariableOption[] {
  if (!sort || sort === "none") return options;
  const copy = [...options];
  const key = (o: VariableOption) => o.text;
  switch (sort) {
    case "alphaAsc":
      copy.sort((a, b) => key(a).localeCompare(key(b)));
      break;
    case "alphaDesc":
      copy.sort((a, b) => key(b).localeCompare(key(a)));
      break;
    case "numAsc":
      copy.sort((a, b) => numCompare(key(a), key(b)));
      break;
    case "numDesc":
      copy.sort((a, b) => numCompare(key(b), key(a)));
      break;
    case "alphaCiAsc":
      copy.sort((a, b) => key(a).toLowerCase().localeCompare(key(b).toLowerCase()));
      break;
    case "alphaCiDesc":
      copy.sort((a, b) => key(b).toLowerCase().localeCompare(key(a).toLowerCase()));
      break;
  }
  return copy;
}
