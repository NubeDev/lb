// Minimal session store + hook — useSyncExternalStore over localStorage.
import { useSyncExternalStore } from "react";
import type { Session } from "./ipc";
import { login as apiLogin, acceptInvite as apiAccept } from "./ipc";

const KEY = "lb.session";

function getSession(): Session | null {
  try {
    return JSON.parse(localStorage.getItem(KEY) || "null");
  } catch {
    return null;
  }
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
