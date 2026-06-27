// The nav rail — the vertical icon strip selecting which surface is open (collaboration scope). One
// button per surface; markup + wiring only. Kept out of App.tsx so the shell file stays small.

import { Hash, Inbox, LogOut, Send, Users } from "lucide-react";

export type Surface = "channels" | "members" | "inbox" | "outbox";

interface Props {
  active: Surface;
  onSelect: (surface: Surface) => void;
  onSignOut: () => void;
}

const SURFACES: { key: Surface; icon: typeof Hash; label: string }[] = [
  { key: "channels", icon: Hash, label: "Channels" },
  { key: "members", icon: Users, label: "Members" },
  { key: "inbox", icon: Inbox, label: "Inbox" },
  { key: "outbox", icon: Send, label: "Outbox" },
];

export function NavRail({ active, onSelect, onSignOut }: Props) {
  return (
    <nav className="flex w-12 flex-col items-center gap-3 border-r border-border bg-panel py-3">
      {SURFACES.map(({ key, icon: Icon, label }) => (
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
      ))}
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
