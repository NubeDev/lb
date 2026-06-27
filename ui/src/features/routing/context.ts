// Context passed from the authenticated App shell into TanStack route components.

import type { CoreSurface } from "@/features/shell";
import type { ExtPage } from "@/features/ext-host";

export interface RoutingContext {
  workspace: string;
  principal: string;
  caps: string[] | undefined;
  allowed: CoreSurface[];
  extPages: ExtPage[];
  extPagesLoading: boolean;
  onSignOut: () => void;
  switchWorkspace: (ws: string) => void;
}
