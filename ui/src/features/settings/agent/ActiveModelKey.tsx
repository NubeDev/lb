// Set / rotate the model key (token) for the ACTIVE pick — INCLUDING a built-in — without cloning it
// (agent-catalog test-and-secrets scope, open-question #5: the active `agent.config` is workspace-
// scoped and can own a sealed secret path). This is the answer to "how do I add a token to the
// in-house model?": the built-in is read-only, but the workspace's *selection* of it can carry a
// sealed key.
//
// Write-only, names-only: the value is sealed through `secret.set` (Private) inside `setActiveKey`;
// only the resulting PATH lands on `agent.config`. The value is never read back — once set, the field
// shows "key is set ✓ · rotate" and stays empty.

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

interface Props {
  /** Whether a sealed key path is already stored on the active `agent.config` (drives the copy). */
  hasKey: boolean;
  /** Seal `value` for the active pick (via `secret.set` → path onto `agent.config`). */
  onSet: (value: string) => Promise<void>;
}

export function ActiveModelKey({ hasKey, onSet }: Props) {
  const [open, setOpen] = useState(false);
  const [value, setValue] = useState("");
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [error, setError] = useState<string | null>(null);

  const save = async () => {
    if (!value) return;
    setStatus("saving");
    setError(null);
    try {
      await onSet(value);
      setValue("");
      setStatus("saved");
      setOpen(false);
    } catch (e) {
      setStatus("error");
      setError(e instanceof Error ? e.message : "could not set the key");
    }
  };

  if (!open) {
    return (
      <div className="flex items-center gap-2">
        <span className="text-[11px] text-muted">
          {hasKey || status === "saved" ? "model key: set ✓" : "model key: using node env / none"}
        </span>
        <Button
          size="sm"
          variant="outline"
          onClick={() => setOpen(true)}
          aria-label="set model key"
        >
          {hasKey || status === "saved" ? "Rotate key" : "Set model key"}
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-1.5" aria-label="model key editor">
      <p className="text-[11px] leading-snug text-muted">
        Sealed in the workspace store on save (never read back). Applies to the active selection — a
        built-in stays read-only, but your workspace's pick of it can carry this key.
      </p>
      <div className="flex items-center gap-2">
        <Input
          aria-label="active model key"
          type="password"
          value={value}
          placeholder={hasKey ? "enter a new value to rotate" : "paste the API key / token to seal it"}
          onChange={(e) => setValue(e.target.value)}
        />
        <Button
          size="sm"
          onClick={() => void save()}
          disabled={!value || status === "saving"}
          aria-label="save model key"
        >
          {status === "saving" ? "Sealing…" : "Save"}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            setOpen(false);
            setValue("");
            setError(null);
          }}
          aria-label="cancel model key"
        >
          Cancel
        </Button>
      </div>
      {status === "error" && (
        <span role="alert" className="text-[11px] text-red-500">
          {error}
        </span>
      )}
    </div>
  );
}
