// The shape an app extension's Module Federation container exposes to the shell.

import type * as React from "react";
import type { Bridge, MountCtx } from "./mount";
import type { WidgetHandleProps } from "./widget";

/**
 * An app remote exposes React components, not DOM mounts (RN has no DOM).
 * `Page` renders the extension's `[app]` nav entry; `Widget` renders one of its
 * `[[widget]]` tiles. Both optional — an extension may ship either or both.
 */
export interface AppRemote {
  Page?: React.ComponentType<{ ctx: MountCtx; bridge: Bridge }>;
  Widget?: React.ComponentType<WidgetHandleProps>;
}
