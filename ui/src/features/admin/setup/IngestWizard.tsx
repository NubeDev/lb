// The Ingest wizard (setup scope) — walks a user from nothing to "an external program is pushing data
// in", in three steps: (1) create the series to write into, (2) mint an API key the program will
// authenticate with, (3) copy a runnable Python example prefilled with both. It is PURE ORCHESTRATION
// over pieces that already exist — the ingest `CreateSeriesWizard` (schema builder), the api-keys
// `useApiKeys` hook (one-time secret), the shared `CopyButton`, and the `pythonSnippet` generator that
// mirrors `clients/python/example.py`. No new backend, no duplicated editor. That reuse is the whole
// point. One responsibility per file (FILE-LAYOUT).

import { useEffect, useState } from "react";
import { Check, Database, KeyRound, Terminal } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { CopyButton } from "@/components/ui/copy-button";
import { gatewayUrl } from "@/lib/ipc/http";
import { CAP, hasCap } from "@/lib/session";
import { saveSchema } from "@/lib/ingest/schema.api";
import { newField } from "@/lib/ingest/schema.types";
import { listSeries } from "@/lib/ingest/ingest.api";
import { CreateSeriesWizard } from "@/features/ingest/CreateSeriesWizard";
import { useApiKeys } from "../useApiKeys";
import { StepFlow, StepShell, type FlowStep } from "../wizard/StepFlow";
import { PIP_INSTALL, pythonSnippet } from "./pythonSnippet";

interface Props {
  ws: string;
  caps: string[] | undefined;
  onDone?: () => void;
}

export function IngestWizard({ ws, caps, onDone }: Props) {
  const canWriteIngest = hasCap(caps, CAP.ingestWrite);
  const canMintKey = hasCap(caps, CAP.apikeyManage);

  // Accumulated across steps: the series to write into, whether it's been PERSISTED (not just named),
  // and the one-time key bearer.
  const [series, setSeries] = useState("");
  const [created, setCreated] = useState(false);
  const [apiKey, setApiKey] = useState<string | null>(null);

  const steps: FlowStep[] = [
    {
      key: "series",
      label: "Series",
      hint: "Where data lands",
      // Can't advance until the series is actually created — otherwise a typed-but-unwritten name
      // wouldn't exist when they hit the API-key/Python steps (a series exists only once written to).
      canAdvance: created,
      render: () => (
        <StepShell
          icon={Database}
          title="Create the series to write into"
          blurb="A series is the named stream your program pushes Samples to. Give it a name and a typed schema, then create it."
        >
          <SeriesStep
            value={series}
            created={created}
            onPick={(name) => {
              setSeries(name);
              setCreated(false); // a name change invalidates the previous create
            }}
            onCreated={(name) => {
              setSeries(name);
              setCreated(true);
            }}
            canWrite={canWriteIngest}
          />
        </StepShell>
      ),
    },
    {
      key: "key",
      label: "API key",
      hint: "How it authenticates",
      render: () => (
        <StepShell
          icon={KeyRound}
          title="Mint an API key"
          blurb="Your program authenticates as a machine principal with a long-lived, revocable key. The secret is shown once."
        >
          <ApiKeyStep apiKey={apiKey} onMinted={setApiKey} canMint={canMintKey} series={series} />
        </StepShell>
      ),
    },
    {
      key: "code",
      label: "Python",
      hint: "Copy & run",
      render: () => (
        <StepShell
          icon={Terminal}
          title="Push data with Python"
          blurb="Install the client and run this — it writes one Sample and reads it back, against this node."
        >
          <CodeStep ws={ws} series={series} apiKey={apiKey} />
        </StepShell>
      ),
    },
  ];

  return <StepFlow steps={steps} finishLabel="Done" onFinish={onDone} />;
}

// ── Step 1: series ── actually PERSISTS the series (a series exists only once written to — there is
//    no create-series verb; `saveSchema` writes the schema as the series' first real sample). Two
//    paths, both reused: a quick "Create series" that writes a minimal one-field schema for a typed
//    name, or the ingest `CreateSeriesWizard` modal for a full typed schema. Either way the series is
//    real before we move on.
function SeriesStep({
  value,
  created,
  onPick,
  onCreated,
  canWrite,
}: {
  value: string;
  created: boolean;
  onPick: (name: string) => void;
  onCreated: (name: string) => void;
  canWrite: boolean;
}) {
  const [modal, setModal] = useState(false);
  const [existing, setExisting] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const openModal = async () => {
    try {
      setExisting(await listSeries());
    } catch {
      setExisting([]);
    }
    setModal(true);
  };

  // Quick-create: materialize the series with a minimal schema so it exists without opening the modal.
  const quickCreate = async () => {
    const name = value.trim();
    if (!name) return;
    setBusy(true);
    setError(null);
    try {
      await saveSchema({ series: name, fields: [newField("number")] });
      onCreated(name);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      <label className="block space-y-1.5">
        <span className="text-xs font-medium text-muted">Series name</span>
        <Input
          value={value}
          placeholder="e.g. demo.cpu_temp"
          onChange={(e) => onPick(e.target.value)}
          aria-label="Series name"
          disabled={created}
        />
      </label>

      {created ? (
        <div
          role="status"
          className="flex items-center gap-2 rounded-md border border-accent/25 bg-accent/10 px-3 py-2 text-xs text-accent"
        >
          <Check size={14} />
          Series <span className="font-medium">{value.trim()}</span> created — ready to receive samples.
        </div>
      ) : canWrite ? (
        <div className="flex flex-wrap gap-2">
          <Button
            size="sm"
            disabled={!value.trim() || busy}
            onClick={() => void quickCreate()}
            aria-label="Create series"
          >
            <Database size={13} /> {busy ? "Creating…" : "Create series"}
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={!value.trim()}
            onClick={() => void openModal()}
            aria-label="Build a typed schema"
          >
            Build a typed schema…
          </Button>
        </div>
      ) : (
        <p className="text-xs text-muted">
          Creating a series needs ingest-write access. Ask an admin, or name a series that already
          exists to write into.
        </p>
      )}

      {error && (
        <p role="alert" className="text-xs text-red-500">
          {error}
        </p>
      )}

      {modal && (
        <CreateSeriesWizard
          existing={existing}
          initialName={value.trim()}
          onCancel={() => setModal(false)}
          onCreate={async (schema) => {
            await saveSchema(schema);
            onCreated(schema.series);
            setModal(false);
          }}
        />
      )}
    </div>
  );
}

// ── Step 2: API key ── reuses useApiKeys (real mint + one-time secret).
function ApiKeyStep({
  apiKey,
  onMinted,
  canMint,
  series,
}: {
  apiKey: string | null;
  onMinted: (key: string) => void;
  canMint: boolean;
  series: string;
}) {
  const { newSecret, create, clearSecret } = useApiKeys();
  const [busy, setBusy] = useState(false);

  // Surface the freshly-minted secret to the wizard's own state, then clear the hook's copy.
  useEffect(() => {
    if (newSecret && newSecret !== apiKey) {
      onMinted(newSecret);
      clearSecret();
    }
  }, [newSecret, apiKey, onMinted, clearSecret]);

  const mint = async () => {
    setBusy(true);
    try {
      await create({ label: `ingest: ${series || "series"}`, kind: "api", role: "apikey-write" });
    } finally {
      setBusy(false);
    }
  };

  if (apiKey) {
    return (
      <div role="alert" className="space-y-2 rounded-md border border-accent/25 bg-accent/10 px-3 py-2.5">
        <p className="text-xs font-medium text-accent">Copy the key now — you won&apos;t see it again.</p>
        <code className="block break-all rounded-md bg-bg px-2 py-1 font-mono text-xs">{apiKey}</code>
        <CopyButton text={apiKey} label="Copy key" variant="outline" />
      </div>
    );
  }

  if (!canMint) {
    return (
      <p className="text-xs text-muted">
        You don&apos;t have permission to mint keys. Ask an admin for one with{" "}
        <span className="font-mono">apikey-write</span> access, then paste it into the code below.
      </p>
    );
  }

  return (
    <Button size="sm" disabled={busy} onClick={() => void mint()} aria-label="Mint an API key">
      <KeyRound size={13} /> {busy ? "Minting…" : "Mint a write key"}
    </Button>
  );
}

// ── Step 3: Python example ── the copy-paste snippet, prefilled.
function CodeStep({ ws, series, apiKey }: { ws: string; series: string; apiKey: string | null }) {
  const url = gatewayUrl() || "http://127.0.0.1:8080";
  const code = pythonSnippet({ url, ws, series: series.trim() || "demo.cpu_temp", apiKey: apiKey ?? undefined });

  return (
    <div className="space-y-3">
      <CodeBlock label="Install" code={PIP_INSTALL} />
      <CodeBlock label="example.py" code={code} />
      {!apiKey && (
        <p className="text-xs text-muted">
          No key minted here — replace{" "}
          <span className="font-mono">lbk_REPLACE_WITH_YOUR_API_KEY</span> with your key.
        </p>
      )}
    </div>
  );
}

function CodeBlock({ label, code }: { label: string; code: string }) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-medium text-muted">{label}</span>
        <CopyButton text={code} label="Copy" />
      </div>
      <pre className="overflow-x-auto rounded-md border border-border bg-bg px-3 py-2.5 font-mono text-xs leading-relaxed text-fg">
        <code>{code}</code>
      </pre>
    </div>
  );
}
