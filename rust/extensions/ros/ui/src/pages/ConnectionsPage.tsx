import { useEffect, useState } from "react";
import { Plus, Radio, Trash2 } from "lucide-react";
import type { PageBridge } from "@nube/ext-ui-sdk";
import { ExtPage } from "@nube/ext-ui-sdk";
import { useConnections } from "@/app/useConnections";
import { useCreateConnection } from "@/app/useCreateConnection";
import { useDeleteConnection } from "@/app/useDeleteConnection";
import { usePingConnection } from "@/app/usePingConnection";
import { ConnectionForm, baseUrlOf, type ConnectionFormValues } from "@/components/ConnectionForm";

interface Props {
  bridge: PageBridge;
  isAdmin: boolean;
  onOpen: (uuid: string) => void;
}

export function ConnectionsPage({ bridge, isAdmin, onOpen }: Props) {
  const list = useConnections(bridge);
  const create = useCreateConnection(bridge);
  const del = useDeleteConnection(bridge);
  const ping = usePingConnection(bridge);
  const [creating, setCreating] = useState(false);
  const [pingResults, setPingResults] = useState<Record<string, string>>({});

  useEffect(() => {
    list.run();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleCreate = async (values: ConnectionFormValues) => {
    await create.run({
      uuid: crypto.randomUUID(),
      name: values.name,
      base_url: baseUrlOf(values),
      token: values.token,
    });
    setCreating(false);
    list.run();
  };

  const handlePing = async (uuid: string) => {
    const result = await ping.run(uuid);
    setPingResults((m) => ({ ...m, [uuid]: result.ok ? "green" : result.error ?? "unreachable" }));
  };

  const handleDelete = async (uuid: string) => {
    await del.run(uuid);
    list.run();
  };

  return (
    <ExtPage
      title="ROS Connections"
      icon={<Radio size={16} />}
      actions={
        isAdmin && (
          <button
            className="flex items-center gap-1 rounded bg-accent px-3 py-1 text-white"
            onClick={() => setCreating(true)}
          >
            <Plus size={14} /> New connection
          </button>
        )
      }
    >
      {creating && (
        <div className="mb-4 rounded border border-border bg-panel p-4">
          <ConnectionForm submitLabel="Create" onSubmit={handleCreate} onCancel={() => setCreating(false)} />
        </div>
      )}
      {list.error && <p className="text-red-400">{list.error}</p>}
      {list.loading && !list.data && <p className="text-muted">Loading…</p>}
      {list.data && list.data.items.length === 0 && (
        <p className="text-muted">No ROS connections yet.</p>
      )}
      <ul className="flex flex-col gap-2">
        {list.data?.items.map((c) => (
          <li
            key={c.uuid}
            className="flex items-center justify-between rounded border border-border bg-panel px-3 py-2"
          >
            <button className="flex-1 text-left" onClick={() => onOpen(c.uuid)}>
              <span className="font-medium">{c.name}</span>
              <span className="ml-2 text-sm text-muted">{c.base_url}</span>
              {!c.enable && <span className="ml-2 text-sm text-muted">(disabled)</span>}
            </button>
            {pingResults[c.uuid] && (
              <span className="mr-2 text-sm text-muted">{pingResults[c.uuid]}</span>
            )}
            <button
              className="rounded border border-border px-2 py-1 text-sm"
              onClick={() => handlePing(c.uuid)}
              disabled={ping.loading}
            >
              Ping
            </button>
            {isAdmin && (
              <button
                className="ml-2 rounded border border-border p-1 text-red-400"
                aria-label={`delete ${c.name}`}
                onClick={() => handleDelete(c.uuid)}
              >
                <Trash2 size={14} />
              </button>
            )}
          </li>
        ))}
      </ul>
    </ExtPage>
  );
}
