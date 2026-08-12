// remoteEntry.tsx — GENERATED boilerplate. The SDK owns the mount contract (docs/extensions/README.md
// §3a): no hand-written `mount`/`mountWidget`, no `createRoot`, no `document.head` injection.
import styles from "@/styles/tokens.css?inline";
import { defineRemote } from "@nube/ext-ui-sdk";
import { App } from "@/App";
import { PointValueWidget } from "@/widgets/PointValueWidget";
import { PointWriteWidget } from "@/widgets/PointWriteWidget";

export const { mount, mountWidget } = defineRemote({
  id: "ros",
  styles,
  page: (ctx, bridge) => <App ctx={ctx} bridge={bridge} />,
  // Keyed by the [[widget]] label slug (widgetIdOf): "ROS Point Value" → "ros-point-value".
  widgets: {
    "ros-point-value": (ctx, bridge) => <PointValueWidget ctx={ctx} bridge={bridge} />,
    "ros-point-write": (ctx, bridge) => <PointWriteWidget ctx={ctx} bridge={bridge} />,
  },
});
