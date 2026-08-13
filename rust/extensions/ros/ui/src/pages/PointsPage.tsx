import { useEffect, useState } from "react";
import { Gauge } from "lucide-react";
import type { PageBridge } from "@nube/ext-ui-sdk";
import { ExtPage } from "@nube/ext-ui-sdk";
import { usePoints } from "@/app/usePoints";
import { useWritePoint } from "@/app/useWritePoint";

interface Props {
  bridge: PageBridge;
  rosUuid: string;
  deviceUuid: string;
  networkName: string;
  deviceName: string;
  isAdmin: boolean;
  onBack: () => void;
}

export function PointsPage({
  bridge,
  rosUuid,
  deviceUuid,
  networkName,
  deviceName,
  isAdmin,
  onBack,
}: Props) {
  const points = usePoints(bridge);
  const write = useWritePoint(bridge);
  const [writingUuid, setWritingUuid] = useState<string | null>(null);
  const [slot, setSlot] = useState("8");
  const [value, setValue] = useState("");

  useEffect(() => {
    points.run(rosUuid, deviceUuid);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rosUuid, deviceUuid]);

  const submitWrite = async (pointUuid: string) => {
    await write.run({
      ros_uuid: rosUuid,
      point_uuid: pointUuid,
      slot: Number(slot),
      value: value === "" ? null : Number(value),
    });
    setWritingUuid(null);
    points.run(rosUuid, deviceUuid);
  };

  return (
    <ExtPage icon={<Gauge size={16} />} crumbs={[{ label: networkName, onClick: onBack }, { label: deviceName }]}>
      {points.error && <p className="text-red-400">{points.error}</p>}
      {points.loading && !points.data && <p className="text-muted">Loading…</p>}
      {points.data && points.data.items.length === 0 && (
        <p className="text-muted">No points on this device.</p>
      )}
      <ul className="flex flex-col gap-2">
        {points.data?.items.map((p) => (
          <li key={p.uuid} className="rounded border border-border bg-panel px-3 py-2">
            <div className="flex items-center justify-between">
              <div>
                <span className="font-medium">{p.name}</span>
                {!p.enable && <span className="ml-2 text-sm text-muted">(disabled)</span>}
              </div>
              <div className="flex items-center gap-2">
                <span className="tabular-nums">{p.present_value ?? "—"}</span>
                {isAdmin && (
                  <button
                    className="rounded border border-border px-2 py-1 text-sm"
                    onClick={() => setWritingUuid(writingUuid === p.uuid ? null : p.uuid)}
                  >
                    Write
                  </button>
                )}
              </div>
            </div>
            {writingUuid === p.uuid && (
              <div className="mt-2 flex items-center gap-2">
                <label className="flex items-center gap-1 text-sm">
                  Slot
                  <input
                    className="w-14 rounded border border-border bg-bg px-1 py-0.5"
                    value={slot}
                    onChange={(e) => setSlot(e.target.value)}
                  />
                </label>
                <label className="flex items-center gap-1 text-sm">
                  Value
                  <input
                    className="w-24 rounded border border-border bg-bg px-1 py-0.5"
                    placeholder="null releases"
                    value={value}
                    onChange={(e) => setValue(e.target.value)}
                  />
                </label>
                <button
                  className="rounded bg-accent px-2 py-1 text-sm text-white"
                  onClick={() => submitWrite(p.uuid)}
                  disabled={write.loading}
                >
                  Send
                </button>
                {write.error && <span className="text-sm text-red-400">{write.error}</span>}
              </div>
            )}
          </li>
        ))}
      </ul>
    </ExtPage>
  );
}
