import { useEffect } from "react";
import type { WidgetBridge, WidgetCtx } from "@nube/ext-ui-sdk";
import { usePoint } from "@/app/usePoint";
import { isNotFound } from "@/app/types";

interface Props {
  ctx: WidgetCtx;
  bridge: WidgetBridge;
}

/** The `[[widget]] label = "ROS Point Value"` tile — reads one point via `point.get` and renders its
 *  live value. Panel config (`ctx.options`), set by the cascading Connection→Network→Device→Point
 *  select chain (`extension.toml`'s declared `options[]`, resolved client-side by rubix-ai's
 *  `select-async` control): `connectionUuid`, `networkUuid`/`deviceUuid` (narrow the Point picker,
 *  not read here), `pointUuid`. */
export function PointValueWidget({ ctx, bridge }: Props) {
  const connectionUuid = ctx.options.connectionUuid as string | undefined;
  const pointUuid = ctx.options.pointUuid as string | undefined;
  const point = usePoint(bridge);

  useEffect(() => {
    if (connectionUuid && pointUuid) point.run(connectionUuid, pointUuid);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connectionUuid, pointUuid]);

  if (!connectionUuid || !pointUuid) {
    return <p className="text-sm text-muted">Configure a connection + point for this tile.</p>;
  }
  if (point.loading && !point.data) return <p className="text-sm text-muted">Loading…</p>;
  if (point.error) return <p className="text-sm text-red-400">{point.error}</p>;
  if (!point.data || isNotFound(point.data)) return <p className="text-sm text-muted">Point not found.</p>;

  return (
    <div className="flex h-full flex-col items-center justify-center gap-1">
      <span className="text-2xl font-semibold tabular-nums">{point.data.present_value ?? "—"}</span>
      <span className="text-sm text-muted">{point.data.name}</span>
    </div>
  );
}
