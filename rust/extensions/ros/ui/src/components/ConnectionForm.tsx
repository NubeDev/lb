import { useState } from "react";

// Field shape matches the real Grafana "Rubix OS Data Source" config screen (Protocol/Host/Port/
// Token) — Port only shown when Protocol isn't https (https implies 443).

export interface ConnectionFormValues {
  name: string;
  protocol: "http" | "https";
  host: string;
  port: string;
  token: string;
}

const EMPTY: ConnectionFormValues = { name: "", protocol: "https", host: "", port: "", token: "" };

export function baseUrlOf(v: ConnectionFormValues): string {
  const port = v.protocol === "https" ? "" : v.port ? `:${v.port}` : "";
  return `${v.protocol}://${v.host}${port}`;
}

interface Props {
  initial?: Partial<ConnectionFormValues>;
  tokenConfigured?: boolean;
  submitLabel: string;
  onSubmit: (values: ConnectionFormValues) => void;
  onCancel: () => void;
}

export function ConnectionForm({ initial, tokenConfigured, submitLabel, onSubmit, onCancel }: Props) {
  const [values, setValues] = useState<ConnectionFormValues>({ ...EMPTY, ...initial });
  const [resetToken, setResetToken] = useState(!tokenConfigured);

  const set = <K extends keyof ConnectionFormValues>(key: K, value: ConnectionFormValues[K]) =>
    setValues((v) => ({ ...v, [key]: value }));

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit({ ...values, token: resetToken ? values.token : "" });
      }}
    >
      <label className="flex flex-col gap-1 text-sm">
        Name
        <input
          className="rounded border border-border bg-panel px-2 py-1"
          value={values.name}
          onChange={(e) => set("name", e.target.value)}
          required
        />
      </label>
      <div className="flex gap-3">
        <label className="flex flex-1 flex-col gap-1 text-sm">
          Protocol
          <select
            className="rounded border border-border bg-panel px-2 py-1"
            value={values.protocol}
            onChange={(e) => set("protocol", e.target.value as "http" | "https")}
          >
            <option value="https">https</option>
            <option value="http">http</option>
          </select>
        </label>
        <label className="flex flex-[2] flex-col gap-1 text-sm">
          Host
          <input
            className="rounded border border-border bg-panel px-2 py-1"
            value={values.host}
            onChange={(e) => set("host", e.target.value)}
            placeholder="ros-appliance.example.com"
            required
          />
        </label>
      </div>
      {values.protocol !== "https" && (
        <label className="flex flex-col gap-1 text-sm">
          Port
          <input
            className="rounded border border-border bg-panel px-2 py-1"
            value={values.port}
            onChange={(e) => set("port", e.target.value)}
            placeholder="1660"
          />
        </label>
      )}
      <label className="flex flex-col gap-1 text-sm">
        Token
        {resetToken ? (
          <input
            className="rounded border border-border bg-panel px-2 py-1"
            type="password"
            value={values.token}
            onChange={(e) => set("token", e.target.value)}
            required
          />
        ) : (
          <div className="flex items-center gap-2">
            <span className="flex-1 rounded border border-border bg-panel px-2 py-1 text-muted">
              configured
            </span>
            <button type="button" className="rounded border border-border px-2 py-1" onClick={() => setResetToken(true)}>
              Reset
            </button>
          </div>
        )}
      </label>
      <div className="flex justify-end gap-2 pt-2">
        <button type="button" className="rounded border border-border px-3 py-1" onClick={onCancel}>
          Cancel
        </button>
        <button type="submit" className="rounded bg-accent px-3 py-1 text-white">
          {submitLabel}
        </button>
      </div>
    </form>
  );
}
