// Render an icon by its stable string name — the read side of the icon lib. Developers
// store a name ("git-branch") and drop <Icon name={...}/> to paint it, with an optional
// `fallback` name when the stored value is unknown (e.g. an ext referenced an icon this
// build doesn't have). Thin wrapper over `resolveIcon`; forwards lucide props (size,
// className, strokeWidth). One responsibility per file (FILE-LAYOUT).

import type { LucideProps } from "lucide-react";

import { resolveIcon } from "./resolve";

export interface IconProps extends Omit<LucideProps, "name"> {
  /** Stable icon name (kebab-case or PascalCase). */
  name: string | null | undefined;
  /** Name to render if `name` doesn't resolve. Defaults to nothing (renders null). */
  fallback?: string;
}

export function Icon({ name, fallback, ...props }: IconProps) {
  const Resolved = resolveIcon(name) ?? resolveIcon(fallback);
  if (!Resolved) return null;
  return <Resolved {...props} />;
}
