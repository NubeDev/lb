// The upload control for the extensions console (admin-console + lifecycle-management scopes): pick a
// signed artifact JSON file, parse it to an `Artifact`, and hand it to `onUpload` (which publishes it
// over `ext.publish` — the host verify-before-stores). The UI never mints or signs an artifact; it
// only transports a publisher-produced one. A malformed file surfaces a local error and uploads
// nothing. One responsibility (file → Artifact → callback); the table + lifecycle live in the view.

import { useRef, useState } from "react";
import { Upload } from "lucide-react";

import type { Artifact } from "@/lib/ext/ext.api";

interface Props {
  /** Publish the parsed artifact (the view wires this to `useExtensions().upload`). */
  onUpload: (artifact: Artifact) => void;
}

/** Minimal structural check that a parsed object is an artifact (the host re-verifies the signature;
 *  this only guards against an obviously-wrong file so we don't POST garbage). */
function isArtifact(v: unknown): v is Artifact {
  const a = v as Record<string, unknown>;
  return (
    !!a &&
    typeof a.ext_id === "string" &&
    typeof a.version === "string" &&
    typeof a.manifest_toml === "string" &&
    Array.isArray(a.wasm) &&
    typeof a.publisher_key_id === "string" &&
    Array.isArray(a.signature)
  );
}

export function UploadArtifact({ onUpload }: Props) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [error, setError] = useState<string | null>(null);

  // Read via `FileReader` (portable across browsers; `Blob.text()` is not reliably present in every
  // runtime). Returns the file's text, or rejects on a read error.
  function readText(file: File): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = () => reject(reader.error ?? new Error("read failed"));
      reader.readAsText(file);
    });
  }

  async function onFile(file: File) {
    setError(null);
    try {
      const parsed: unknown = JSON.parse(await readText(file));
      if (!isArtifact(parsed)) {
        setError("Not a valid signed artifact (expected the publisher's artifact JSON).");
        return;
      }
      onUpload(parsed);
    } catch {
      setError("Could not read the artifact file (invalid JSON).");
    }
  }

  return (
    <div className="flex items-center gap-2">
      <input
        ref={inputRef}
        type="file"
        accept="application/json,.json"
        aria-label="artifact file"
        className="hidden"
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) void onFile(f);
          e.target.value = ""; // allow re-selecting the same file
        }}
      />
      <button
        type="button"
        aria-label="upload artifact"
        className="flex items-center gap-1 rounded bg-accent/15 px-3 py-1 text-xs text-accent"
        onClick={() => inputRef.current?.click()}
      >
        <Upload size={14} /> Upload signed artifact
      </button>
      {error && (
        <span role="alert" className="text-xs text-red-400">
          {error}
        </span>
      )}
    </div>
  );
}
