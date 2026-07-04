// Barrel for the agent-dock feature (re-exports only — FILE-LAYOUT). The shell mounts <AgentDock> in a
// flex slot beside <Outlet/>, wraps the routed tree in <PageContextProvider>, drives chrome via
// useDockChrome, and renders the launcher in the StatusBar.

export { AgentDock } from "./AgentDock";
export { PageContextProvider, usePageContext } from "./PageContextProvider";
export { useDockChrome, type DockChrome } from "./useDockChrome";
export { DockLauncher } from "./DockLauncher";
export { useDockHotkey } from "./useDockHotkey";
