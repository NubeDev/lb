// Widget contract — byte-compatible with the shipped web v3 "frames-in" contract
// (ui/src/features/dashboard/builder/federationWidget.ts). This file is the authored
// source; the web host copy, each extension's contract mirror, and the devkit
// template are checked against it. Change them together or not at all.

/** A shell-resolved data frame. v3 data widgets render frames; they never fetch. */
export type WidgetFrame = Record<string, unknown>;

export interface WidgetCtx {
  /** Contract version. 3 = frames-in. */
  v: number;
  workspace: string;
  binding: Record<string, unknown>;
  options: Record<string, unknown>;
  vars?: Record<string, unknown>;
  builtins?: Record<string, unknown>;
  timeRange?: { from: number; to: number };
  /** v3 data tiles: frames resolved by the shell from the cell's `sources[]`. */
  data?: WidgetFrame[];
  /** v3: Field-tab options (Grafana-compatible fieldConfig). */
  fieldConfig?: unknown;
}

/** Widget bridge: `call` as in `Bridge`, plus `watch` mapped onto gateway SSE. */
export interface WidgetBridge {
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
  watch: (
    tool: string,
    args: Record<string, unknown>,
    onEvent: (e: unknown) => void,
  ) => () => void;
}

/** Props the app shell passes to an extension Widget component. */
export interface WidgetHandleProps {
  ctx: WidgetCtx;
  bridge: WidgetBridge;
  /** Which of the extension's `[[widget]]` tiles to render (label slug). */
  widgetId: string;
}
