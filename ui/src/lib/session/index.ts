// Barrel for the session module (re-exports only — FILE-LAYOUT).

export { useSession } from "./useSession";
export type { SessionState } from "./useSession";
export { getSession, sessionToken, setSession, subscribeSession } from "./session.store";
export { login } from "./session.api";
export type { Session } from "./session.types";
export { CAP, ADMIN_CAPS, ADMIN_SECTION_CAPS, hasCap, isAdmin } from "./admin-caps";
