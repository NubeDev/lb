import { useEffect } from "react";
import { HardDrive } from "lucide-react";
import type { PageBridge } from "@nube/ext-ui-sdk";
import { ExtPage } from "@nube/ext-ui-sdk";
import { useDevices } from "@/app/useDevices";

interface Props {
  bridge: PageBridge;
  rosUuid: string;
  networkUuid: string;
  connectionName: string;
  networkName: string;
  onBack: () => void;
  onOpen: (deviceUuid: string) => void;
}

export function DevicesPage({
  bridge,
  rosUuid,
  networkUuid,
  connectionName,
  networkName,
  onBack,
  onOpen,
}: Props) {
  const devices = useDevices(bridge);

  useEffect(() => {
    devices.run(rosUuid, networkUuid);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rosUuid, networkUuid]);

  return (
    <ExtPage
      icon={<HardDrive size={16} />}
      crumbs={[{ label: connectionName, onClick: onBack }, { label: networkName }]}
    >
      {devices.error && <p className="text-red-400">{devices.error}</p>}
      {devices.loading && !devices.data && <p className="text-muted">Loading…</p>}
      {devices.data && devices.data.items.length === 0 && (
        <p className="text-muted">No devices on this network.</p>
      )}
      <ul className="flex flex-col gap-2">
        {devices.data?.items.map((d) => (
          <li key={d.uuid} className="rounded border border-border bg-panel px-3 py-2">
            <button className="w-full text-left" onClick={() => onOpen(d.uuid)}>
              <span className="font-medium">{d.name}</span>
              {!d.enable && <span className="ml-2 text-sm text-muted">(disabled)</span>}
            </button>
          </li>
        ))}
      </ul>
    </ExtPage>
  );
}
