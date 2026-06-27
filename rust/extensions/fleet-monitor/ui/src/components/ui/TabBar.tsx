import { NavLink } from "react-router-dom";

import { cn } from "@/lib/cn";

export interface TabItem {
  to: string;
  label: string;
  end?: boolean;
}

interface Props {
  items: TabItem[];
}

/** A NavLink tab bar — drives the nested sub-routes. The active child gets the amber accent underline. */
export function TabBar({ items }: Props) {
  return (
    <nav className="flex gap-1 border-b border-border" role="tablist">
      {items.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={item.end}
          role="tab"
          className={({ isActive }) =>
            cn(
              "border-b-2 px-3 py-2 text-sm font-medium transition-colors",
              isActive
                ? "border-accent text-fg"
                : "border-transparent text-muted hover:text-fg",
            )
          }
        >
          {item.label}
        </NavLink>
      ))}
    </nav>
  );
}
