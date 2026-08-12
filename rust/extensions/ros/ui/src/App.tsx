import { useEffect } from "react";
import type { PageBridge, PageCtx } from "@nube/ext-ui-sdk";
import { ConnectionsPage } from "@/pages/ConnectionsPage";
import { NetworksPage } from "@/pages/NetworksPage";
import { DevicesPage } from "@/pages/DevicesPage";
import { PointsPage } from "@/pages/PointsPage";
import { useConnection } from "@/app/useConnection";
import { useNetwork } from "@/app/useNetwork";
import { useDevice } from "@/app/useDevice";
import { isNotFound } from "@/app/types";

interface Props {
  ctx: PageCtx;
  bridge: PageBridge;
}

/** Routes on `ctx.route` — the host owns the URL (ext-nav-contribution scope), this ext never pushes
 *  history itself. Grammar: "" | "<rosUuid>" | "<rosUuid>/<networkUuid>" |
 *  "<rosUuid>/<networkUuid>/<deviceUuid>" — one drill level per path segment. */
export function App({ ctx, bridge }: Props) {
  const [rosUuid, networkUuid, deviceUuid] = (ctx.route ?? "").split("/").filter(Boolean);
  const isAdmin = ctx.isAdmin ?? false;

  const connection = useConnection(bridge);
  const network = useNetwork(bridge);
  const device = useDevice(bridge);

  useEffect(() => {
    if (rosUuid) connection.run(rosUuid);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rosUuid]);
  useEffect(() => {
    if (rosUuid && networkUuid) network.run(rosUuid, networkUuid);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rosUuid, networkUuid]);
  useEffect(() => {
    if (rosUuid && networkUuid && deviceUuid) device.run(rosUuid, networkUuid, deviceUuid);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rosUuid, networkUuid, deviceUuid]);

  const navigate = (path: string) => ctx.onNavigate?.(path);

  if (!rosUuid) {
    return <ConnectionsPage bridge={bridge} isAdmin={isAdmin} onOpen={(uuid) => navigate(uuid)} />;
  }

  const connectionName =
    connection.data && !isNotFound(connection.data) ? connection.data.name : rosUuid;

  if (!networkUuid) {
    return (
      <NetworksPage
        bridge={bridge}
        rosUuid={rosUuid}
        connectionName={connectionName}
        onBack={() => navigate("")}
        onOpen={(nUuid) => navigate(`${rosUuid}/${nUuid}`)}
      />
    );
  }

  const networkName = network.data && !isNotFound(network.data) ? network.data.name : networkUuid;

  if (!deviceUuid) {
    return (
      <DevicesPage
        bridge={bridge}
        rosUuid={rosUuid}
        networkUuid={networkUuid}
        connectionName={connectionName}
        networkName={networkName}
        onBack={() => navigate("")}
        onOpen={(dUuid) => navigate(`${rosUuid}/${networkUuid}/${dUuid}`)}
      />
    );
  }

  const deviceName = device.data && !isNotFound(device.data) ? device.data.name : deviceUuid;

  return (
    <PointsPage
      bridge={bridge}
      rosUuid={rosUuid}
      deviceUuid={deviceUuid}
      networkName={networkName}
      deviceName={deviceName}
      isAdmin={isAdmin}
      onBack={() => navigate(rosUuid)}
    />
  );
}
