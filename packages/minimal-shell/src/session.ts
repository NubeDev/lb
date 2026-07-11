// Minimal session store + hook — useSyncExternalStore over localStorage.
import { useSyncExternalStore } from "react";
import type { Session } from "./ipc";
import { login as apiLogin, acceptInvite as apiAccept } from "./ipc";

const KEY = "lb.session";

// useSyncExternalStore compares snapshots with Object.is: returning a fresh JSON.parse object
// every call makes React loop ("getSnapshot should be cached") once a session exists. Cache the
// parsed value keyed by the raw string so the snapshot is stable between changes.
let cachedRaw: string | null = null;
let cachedSession: Session | null = null;

function getSession(): Session | null {
  const raw = localStorage.getItem(KEY);
  if (raw === cachedRaw) return cachedSession;
  cachedRaw = raw;
  try {
    cachedSession = raw ? JSON.parse(raw) : null;
  } catch {
    cachedSession = null;
  }
  return cachedSession;
}

function setSession(s: Session | null) {
  if (s) localStorage.setItem(KEY, JSON.stringify(s));
  else localStorage.removeItem(KEY);
  emit();
}

const listeners = new Set<() => void>();
function emit() {
  listeners.forEach((l) => l());
}
function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

// ipc.ts clears the stored session on a 401 and fires this event (it can't import us — cycle);
// re-emit so useSession subscribers drop to the login view immediately.
if (typeof window !== "undefined") {
  window.addEventListener("lb.session.cleared", emit);
}

export function useSession(): Session | null {
  return useSyncExternalStore(subscribe, getSession, getSession);
}

export async function signIn(user: string, workspace: string, secret?: string): Promise<void> {
  const s = await apiLogin(user, workspace, secret);
  setSession(s);
}

export async function acceptInvite(
  workspace: string,
  token: string,
  secret: string,
  currentSecret?: string,
): Promise<void> {
  const s = await apiAccept(workspace, token, secret, currentSecret);
  setSession(s);
}

export function signOut(): void {
  setSession(null);
}
