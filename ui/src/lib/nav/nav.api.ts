// The nav API client — one call per export, mirroring the gateway's `nav.*` routes and the host verbs
// 1:1 (nav scope). The UI never calls `invoke` directly; it goes through these named verbs
// (FILE-LAYOUT frontend rules). Each is capability-gated server-side; the workspace + owner come from
// the session token (the hard wall, §7), never an argument. The nav grants NOTHING — resolving is a
// pure lens over the caps the session already holds.

import type { Nav, NavHidden, NavItem, NavPref, NavSummary, ResolvedNav, Visibility } from "./nav.types";
import { invoke } from "@/lib/ipc/invoke";

/** The roster the caller can reach (own + team-shared + workspace), summaries only. Mirrors
 *  `nav.list`. */
export function listNavs(): Promise<NavSummary[]> {
  return invoke<{ navs: NavSummary[] }>("nav_list", {}).then((r) => r.navs);
}

/** Read one nav (the three-gate read, full items). Mirrors `nav.get`. */
export function getNav(id: string): Promise<Nav> {
  return invoke<Nav>("nav_get", { id });
}

/** Create or update a nav (idempotent UPSERT on `id`; owner-only update). Mirrors `nav.save`. */
export function saveNav(id: string, title: string, items: NavItem[]): Promise<Nav> {
  return invoke<Nav>("nav_save", { id, title, items });
}

/** Soft-delete a nav (idempotent tombstone; owner-only). Mirrors `nav.delete`. */
export function deleteNav(id: string): Promise<void> {
  return invoke<void>("nav_delete", { id });
}

/** Set a nav's visibility / write the S4 share edge (owner-only). Mirrors `nav.share`. */
export function shareNav(id: string, visibility: Visibility, team?: string): Promise<Nav> {
  return invoke<Nav>("nav_share", { id, visibility, team });
}

/** Revoke one team share edge (owner-only; idempotent). Mirrors `nav.unshare`. Same `nav.share`
 *  cap as `share` — the inverse write under one grant. */
export function unshareNav(id: string, team: string): Promise<Nav> {
  return invoke<Nav>("nav_unshare", { id, team });
}

/** List the teams a nav is currently shared to (owner-only). Mirrors `nav.list_shares` — the
 *  builder's share-roster read, the exact set the resolver walks. */
export function listNavShares(id: string): Promise<string[]> {
  return invoke<{ teams: string[] }>("nav_list_shares", { id }).then((r) => r.teams);
}

/** Set the one workspace-default nav pointer (admin-ish; empty `id` clears it). Mirrors
 *  `nav.set_default`. */
export function setDefaultNav(id: string): Promise<void> {
  return invoke<void>("nav_set_default", { id });
}

/** THE composite read — the caller's effective menu (picked, tag-expanded, cap-stripped). Mirrors
 *  `nav.resolve`. NavRail renders this one payload directly. */
export function resolveNav(): Promise<ResolvedNav> {
  return invoke<ResolvedNav>("nav_resolve", {});
}

/** Read the caller's OWN active-nav pick (member-owned). Mirrors `nav.pref.get`. */
export function getNavPref(): Promise<NavPref> {
  return invoke<NavPref>("nav_pref_get", {});
}

/** Set the caller's OWN active-nav pick (empty `id` clears it; keyed to the token sub — a caller
 *  cannot set another user's pick). Mirrors `nav.pref.set`. Pins are untouched. */
export function setNavPref(id: string): Promise<NavPref> {
  return invoke<NavPref>("nav_pref_set", { id });
}

/** Replace the caller's OWN pinned favorites (hide-and-pins scope) — ordered refs in the shared
 *  grammar (bare surface key | `ext:<id>` | `dashboard:<id>`). The active pick is untouched (the
 *  partial-write `nav.pref.set`). Member-owned; bounded server-side. */
export function setNavPins(pinned: string[]): Promise<NavPref> {
  return invoke<NavPref>("nav_pref_set", { pinned });
}

/** Read the workspace sidebar hidden-set (hide-and-pins scope). Member-level — the same set the
 *  resolver echoes. Mirrors `nav.hidden.get`. */
export function getNavHidden(): Promise<NavHidden> {
  return invoke<NavHidden>("nav_hidden_get", {});
}

/** Replace the workspace sidebar hidden-set (admin — rides `nav.save`; full-set LWW, empty clears).
 *  Hiding never blocks a route: declutter only, every page verb re-checked on click. Mirrors
 *  `nav.hidden.set`. */
export function setNavHidden(hidden: string[]): Promise<NavHidden> {
  return invoke<NavHidden>("nav_hidden_set", { hidden });
}
