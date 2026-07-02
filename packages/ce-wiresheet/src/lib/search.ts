import type { Component } from "./engine-types";
import { ROLE_NORMAL } from "./engine-types";
import { parseFacet, rawFacet } from "./facet";

// A single search result: a component, or one of its props matched by its facet
// label / value aliases.
export interface SearchHit {
  compUid: number; // navigation target
  compName: string;
  type: string;
  path: string; // stripped of leading "root/"
  here: boolean; // in the folder currently being viewed
  propName?: string;
  label?: string; // facet label
  aliasText?: string; // space-joined alias labels
}

// Flatten a (nested) component tree into a searchable index: one entry per
// component, plus one per user-prop that carries a facet label or aliases (a
// bare prop name is already reachable via its component, so it's skipped).
export function buildSearchIndex(nodes: Component[], currentParentUid: number): SearchHit[] {
  const flat: SearchHit[] = [];
  const walk = (c: Component) => {
    if (c.uid !== 0) {
      const path = c.path.startsWith("root/") ? c.path.slice(5) : c.path;
      const here = c.parent === currentParentUid;
      const compName = c.name || c.type;
      flat.push({ compUid: c.uid, compName, type: c.type, path, here });
      const facet = parseFacet(rawFacet(c.properties) ?? "");
      for (const [propName, p] of Object.entries(c.properties)) {
        if ((p.systemRole ?? ROLE_NORMAL) !== ROLE_NORMAL) continue;
        const fc = facet.get(p.uid);
        const aliasText = fc?.aliases?.map((a) => a.label).join(" ") ?? "";
        if (!fc?.label && !aliasText) continue;
        flat.push({
          compUid: c.uid,
          compName,
          type: c.type,
          path,
          here,
          propName,
          label: fc?.label,
          aliasText,
        });
      }
    }
    c.children?.forEach(walk);
  };
  nodes.forEach(walk);
  return flat;
}

// Rank an index against a query. Empty query → component rows only (first 60).
// Otherwise score by match quality (exact > prefix > contains), float
// current-folder hits to the top, cap at 80.
export function rankSearchHits(all: SearchHit[], query: string): SearchHit[] {
  const f = query.trim().toLowerCase();
  if (!f) return all.filter((h) => !h.propName).slice(0, 60);
  return all
    .map((h) => {
      let score = -1;
      if (h.propName) {
        const label = (h.label ?? "").toLowerCase();
        const al = (h.aliasText ?? "").toLowerCase();
        const pn = h.propName.toLowerCase();
        if (label === f || al.split(" ").includes(f)) score = 1;
        else if (label.startsWith(f) || pn.startsWith(f)) score = 2;
        else if (label.includes(f) || al.includes(f) || pn.includes(f)) score = 3;
      } else {
        const name = h.compName.toLowerCase();
        if (name === f) score = 0;
        else if (name.startsWith(f)) score = 1;
        else if (name.includes(f)) score = 2;
        else if (h.path.toLowerCase().includes(f) || h.type.toLowerCase().includes(f)) score = 3;
      }
      return { h, score };
    })
    .filter((x) => x.score >= 0)
    .sort(
      (a, b) =>
        Number(b.h.here) - Number(a.h.here) ||
        a.score - b.score ||
        a.h.compName.localeCompare(b.h.compName),
    )
    .slice(0, 80)
    .map((x) => x.h);
}
