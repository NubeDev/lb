import { useEffect, useState } from "react";
import type { WidgetBridge, WidgetCtx } from "@nube/ext-ui-sdk";
import { usePoint } from "@/app/usePoint";
import { useWritePoint } from "@/app/useWritePoint";
import { useSchedule } from "@/app/useSchedule";
import { useWriteSchedule } from "@/app/useWriteSchedule";
import { isNotFound } from "@/app/types";

interface Props {
  ctx: WidgetCtx;
  bridge: WidgetBridge;
}

/** The `[[widget]] label = "ROS Point Write"` tile — the control half of the poll/read pair. Panel
 *  config (`ctx.options`), set by the cascading select chain (`extension.toml`'s declared `options[]`):
 *  `connectionUuid`, `subscriptionType` (`"point"` | `"schedule"`), `networkUuid`/`deviceUuid` (narrow
 *  the Point picker, not read here), `pointUuid`, `priority`/`slot` (1-16, point mode), `scheduleUuid`
 *  (schedule mode). Write-capable callers only (`isAdmin`) — a non-admin sees the value, read-only. */
export function PointWriteWidget({ ctx, bridge }: Props) {
  const connectionUuid = ctx.options.connectionUuid as string | undefined;
  const subscriptionType = (ctx.options.subscriptionType as string | undefined) ?? "point";
  const isAdmin = ctx.isAdmin ?? false;

  if (subscriptionType === "schedule") {
    return <ScheduleWrite ctx={ctx} bridge={bridge} connectionUuid={connectionUuid} isAdmin={isAdmin} />;
  }
  return <PointWrite ctx={ctx} bridge={bridge} connectionUuid={connectionUuid} isAdmin={isAdmin} />;
}

function PointWrite({
  ctx,
  bridge,
  connectionUuid,
  isAdmin,
}: {
  ctx: WidgetCtx;
  bridge: WidgetBridge;
  connectionUuid: string | undefined;
  isAdmin: boolean;
}) {
  const pointUuid = ctx.options.pointUuid as string | undefined;
  const slot = Number(ctx.options.priority ?? ctx.options.slot ?? 8);

  const point = usePoint(bridge);
  const write = useWritePoint(bridge);
  const [value, setValue] = useState("");

  useEffect(() => {
    if (connectionUuid && pointUuid) point.run(connectionUuid, pointUuid);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connectionUuid, pointUuid]);

  if (!connectionUuid || !pointUuid) {
    return <p className="text-sm text-muted">Configure a connection + point for this tile.</p>;
  }
  if (point.loading && !point.data) return <p className="text-sm text-muted">Loading…</p>;
  if (point.data && isNotFound(point.data)) return <p className="text-sm text-muted">Point not found.</p>;

  const currentValue = point.data && !isNotFound(point.data) ? point.data.present_value : null;

  const submit = async () => {
    await write.run({ ros_uuid: connectionUuid, point_uuid: pointUuid, slot, value: value === "" ? null : Number(value) });
    point.run(connectionUuid, pointUuid);
  };

  return (
    <div className="flex h-full flex-col items-center justify-center gap-2">
      <span className="text-2xl font-semibold tabular-nums">{currentValue ?? "—"}</span>
      {isAdmin ? (
        <div className="flex items-center gap-1">
          <input
            className="w-20 rounded border border-border bg-panel px-1 py-0.5 text-sm"
            placeholder="null releases"
            value={value}
            onChange={(e) => setValue(e.target.value)}
          />
          <button
            className="rounded bg-accent px-2 py-0.5 text-sm text-white"
            onClick={submit}
            disabled={write.loading}
          >
            Set
          </button>
        </div>
      ) : (
        <span className="text-sm text-muted">{point.data && !isNotFound(point.data) ? point.data.name : ""}</span>
      )}
      {write.error && <span className="text-xs text-red-400">{write.error}</span>}
    </div>
  );
}

/** Schedule mode — a flat pick (no network/device nesting, matching the appliance's own schedule
 *  model). The write control is deliberately minimal (enable/disable): the schedule's full weekly/
 *  exception/event payload is an appliance-side editing surface, out of scope for a dashboard tile. */
function ScheduleWrite({
  ctx,
  bridge,
  connectionUuid,
  isAdmin,
}: {
  ctx: WidgetCtx;
  bridge: WidgetBridge;
  connectionUuid: string | undefined;
  isAdmin: boolean;
}) {
  const scheduleUuid = ctx.options.scheduleUuid as string | undefined;

  const schedule = useSchedule(bridge);
  const write = useWriteSchedule(bridge);

  useEffect(() => {
    if (connectionUuid && scheduleUuid) schedule.run(connectionUuid, scheduleUuid);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connectionUuid, scheduleUuid]);

  if (!connectionUuid || !scheduleUuid) {
    return <p className="text-sm text-muted">Configure a connection + schedule for this tile.</p>;
  }
  if (schedule.loading && !schedule.data) return <p className="text-sm text-muted">Loading…</p>;
  if (schedule.data && isNotFound(schedule.data)) return <p className="text-sm text-muted">Schedule not found.</p>;

  const current = schedule.data && !isNotFound(schedule.data) ? schedule.data : null;

  const toggle = async (enable: boolean) => {
    await write.run({ ros_uuid: connectionUuid, schedule_uuid: scheduleUuid, schedule: { enable } });
    schedule.run(connectionUuid, scheduleUuid);
  };

  return (
    <div className="flex h-full flex-col items-center justify-center gap-2">
      <span className="text-lg font-semibold">{current?.name ?? "—"}</span>
      <span className="text-sm text-muted">{current?.is_active ? "active" : "inactive"}</span>
      {isAdmin && (
        <button
          className="rounded bg-accent px-2 py-0.5 text-sm text-white"
          onClick={() => toggle(!current?.enable)}
          disabled={write.loading}
        >
          {current?.enable ? "Disable" : "Enable"}
        </button>
      )}
      {write.error && <span className="text-xs text-red-400">{write.error}</span>}
    </div>
  );
}
