// A test-only stateful wrapper mirroring what the router does for `SettingsView` in production: hold
// the active tab in state and pass `tab`/`onTabChange`, so a test can click a tab and see it switch
// exactly as `/settings/<tab>` would. Keeps the gateway tests decoupled from the router while still
// exercising the real tab UI. One component per file (FILE-LAYOUT).

import * as React from "react";

import { SettingsView, coerceSettingsTab, type SettingsTab } from "./SettingsView";

interface Props {
  ws: string;
  caps: string[] | undefined;
  initialTab?: SettingsTab;
}

export function SettingsHarness({ ws, caps, initialTab = "preferences" }: Props) {
  const [tab, setTab] = React.useState<SettingsTab>(initialTab);
  return (
    <SettingsView ws={ws} caps={caps} tab={tab} onTabChange={(next) => setTab(coerceSettingsTab(next))} />
  );
}
