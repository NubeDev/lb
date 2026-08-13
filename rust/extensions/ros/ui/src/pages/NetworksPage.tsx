import { useEffect } from "react";
import { Network } from "lucide-react";
import type { PageBridge } from "@nube/ext-ui-sdk";
import { ExtPage } from "@nube/ext-ui-sdk";
import { useNetworks } from "@/app/useNetworks";

interface Props {
  bridge: PageBridge;
  rosUuid: string;
  connectionName: string;
  onBack: () => void;
  onOpen: (networkUuid: string) => void;
}

export function NetworksPage({ bridge, rosUuid, connectionName, onBack, onOpen }: Props) {
  const networks = useNetworks(bridge);

  useEffect(() => {
    networks.run(rosUuid);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rosUuid]);

  return (
    <ExtPage
      icon={<Network size={16} />}
      crumbs={[{ label: "Connections", onClick: onBack }, { label: connectionName }]}
    >
      {networks.error && <p className="text-red-400">{networks.error}</p>}
      {networks.loading && !networks.data && <p className="text-muted">Loading…</p>}
      {networks.data && networks.data.items.length === 0 && (
        <p className="text-muted">No networks on this appliance.</p>
      )}
      <ul className="flex flex-col gap-2">
        {networks.data?.items.map((n) => (
          <li key={n.uuid} className="rounded border border-border bg-panel px-3 py-2">
            <button className="w-full text-left" onClick={() => onOpen(n.uuid)}>
              <span className="font-medium">{n.name}</span>
              {!n.enable && <span className="ml-2 text-sm text-muted">(disabled)</span>}
            </button>
          </li>
        ))}
      </ul>
    </ExtPage>
  );
}
