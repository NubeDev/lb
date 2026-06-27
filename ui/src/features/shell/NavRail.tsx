// The nav rail — the vertical icon strip selecting which surface is open (collaboration scope). One
// button per surface; markup + wiring only. Kept out of App.tsx so the shell file stays small.

import {
  Activity,
  Boxes,
  Database,
  Hash,
  Inbox,
  LogOut,
  Puzzle,
  Send,
  Shield,
  Users,
} from "lucide-react";

/** The fixed core surfaces the shell ships. */
export type CoreSurface =
  | "channels"
  | "members"
  | "ingest"
  | "data"
  | "inbox"
  | "outbox"
  | "admin"
  | "extensions";

/** A selected surface: a core one, or an **extension page** keyed `ext:<id>` (ui-federation scope). */
export type Surface = CoreSurface | `ext:${string}`;

/** An extension-contributed sidebar page slot (ui-federation scope). */
export interface ExtSlot {
  ext: string;
  label: string;
}

interface Props {
  active: Surface;
  onSelect: (surface: Surface) => void;
  onSignOut: () => void;
  /** Core surfaces the session is allowed to SEE (cap-gated by the caller). Admin/extensions appear
   *  only for an admin session; the gateway re-checks every verb regardless (admin-console scope). */
  allowed: CoreSurface[];
  /** Installed extension pages contributed to the sidebar (ui-federation scope). */
  extSlots?: ExtSlot[];
}

const SURFACES: { key: CoreSurface; icon: typeof Hash; label: string }[] = [
  { key: "channels", icon: Hash, label: "Channels" },
  { key: "members", icon: Users, label: "Members" },
  { key: "ingest", icon: Activity, label: "Ingest" },
  { key: "data", icon: Database, label: "Data" },
  { key: "inbox", icon: Inbox, label: "Inbox" },
  { key: "outbox", icon: Send, label: "Outbox" },
  { key: "admin", icon: Shield, label: "Admin" },
  { key: "extensions", icon: Boxes, label: "Extensions" },
];

export function NavRail({ active, onSelect, onSignOut, allowed, extSlots = [] }: Props) {
  const item = (key: Surface, label: string, Icon: typeof Hash) => (
    <button
      key={key}
      aria-label={label}
      title={label}
      className={`rounded-md p-2 ${
        active === key ? "bg-accent/15 text-accent" : "text-muted hover:bg-bg"
      }`}
      onClick={() => onSelect(key)}
    >
      <Icon size={18} />
    </button>
  );

  return (
    <nav className="flex w-12 flex-col items-center gap-3 border-r border-border bg-panel py-3">
      {SURFACES.filter((s) => allowed.includes(s.key)).map(({ key, icon, label }) =>
        item(key, label, icon),
      )}
      {extSlots.length > 0 && <div className="my-1 h-px w-6 bg-border" aria-hidden />}
      {extSlots.map((s) => item(`ext:${s.ext}`, s.label, Puzzle))}
      <button
        aria-label="Sign out"
        title="Sign out"
        className="mt-auto rounded-md p-2 text-muted hover:bg-bg"
        onClick={onSignOut}
      >
        <LogOut size={18} />
      </button>
    </nav>
  );
}
