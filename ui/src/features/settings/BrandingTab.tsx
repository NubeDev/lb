// The Settings → Branding tab (workspace-branding scope) — the admin-owned workspace identity
// editor. Edits the strings (site name, abbreviation, tagline) and uploads/clears the image marks
// (logo / icon / favicon) that paint the shell chrome for EVERY member of the workspace. Strings
// ride the new `ui_branding` axis on the prefs record through `prefs.set_default` (admin); images
// are embedded as data-URIs in the SAME blob (atomic on one prefs read, no S4 gate-3 ownership
// issue, capped small per the scope). One component per file (FILE-LAYOUT).
//
// Member view: a non-admin (no `mcp:prefs.set_default:call`) sees the resolved brand read-only —
// they can see WHAT the workspace brand is but cannot change it (the gateway re-checks the cap
// server-side regardless). Branding is workspace identity, not personal taste.

import { useCallback, useEffect, useRef, useState } from "react";
import { Upload } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { CAP, hasCap } from "@/lib/session";
import {
  BRAND_IMAGE_ACCEPT,
  BRANDING_PLACEHOLDERS,
  DEFAULT_BRANDING,
  MAX_BRAND_IMAGE_BYTES,
  readBrandImage,
  readResolvedBranding,
  persistWorkspaceDefaultBranding,
  type Branding,
} from "@/lib/branding";
import { Field, FieldGroup } from "./Field";

interface Props {
  ws: string;
  caps: string[] | undefined;
}

const KIB = 1024;

export function BrandingTab({ caps }: Props) {
  const canSetDefault = hasCap(caps, CAP.prefsSetDefault);

  const [brand, setBrand] = useState<Branding>({ ...DEFAULT_BRANDING });
  const [loading, setLoading] = useState(true);
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [error, setError] = useState<string | null>(null);
  const [imageError, setImageError] = useState<string | null>(null);

  // Load the resolved workspace brand. For an admin this is what they're editing (the workspace
  // default — the only link that carries a brand). For a member this is what they see read-only.
  const load = useCallback(() => {
    setLoading(true);
    void readResolvedBranding()
      .then((resolved) => {
        setBrand(resolved);
        setLoading(false);
      })
      .catch(() => {
        // Keep DEFAULT_BRANDING on a load failure — the shell still renders coherently.
        setLoading(false);
      });
  }, []);
  useEffect(load, [load]);

  const updateString = <K extends keyof Branding>(key: K, value: string) => {
    setStatus("idle");
    setImageError(null);
    setBrand((prev) => ({ ...prev, [key]: value }));
  };

  const uploadImage = async (key: "logoDataUri" | "iconDataUri" | "faviconDataUri", file: File | null) => {
    if (!file) return;
    setImageError(null);
    try {
      const dataUri = await readBrandImage(file);
      setBrand((prev) => ({ ...prev, [key]: dataUri }));
      setStatus("idle");
    } catch (e) {
      setImageError(e instanceof Error ? e.message : "upload failed");
    }
  };

  const clearImage = (key: "logoDataUri" | "iconDataUri" | "faviconDataUri") => {
    setImageError(null);
    setBrand((prev) => {
      const next = { ...prev };
      delete next[key];
      return next;
    });
    setStatus("idle");
  };

  const save = async () => {
    setStatus("saving");
    setError(null);
    try {
      await persistWorkspaceDefaultBranding(brand);
      setStatus("saved");
    } catch (e) {
      setStatus("error");
      setError(e instanceof Error ? e.message : "save failed");
    }
  };

  const resetAll = () => {
    // Clear back to the compiled default. The admin could also leave the brand unset (resolve
    // falls back), but writing DEFAULT_BRANDING explicitly is the honest "reset" — matches the
    // theme layer's `resetTheme` discipline.
    setBrand({ ...DEFAULT_BRANDING });
    setStatus("idle");
    setImageError(null);
  };

  if (loading) {
    return <div className="mx-auto max-w-3xl px-4 py-4 text-sm text-muted">Loading…</div>;
  }

  return (
    <div className="mx-auto max-w-3xl px-4 py-4">
      {!canSetDefault && (
        <p className="mb-3 rounded-md border border-border bg-panel/40 px-3 py-2 text-[11px] text-muted">
          You can view the workspace brand. Editing it requires an administrator
          (<code className="text-fg">mcp:prefs.set_default:call</code>).
        </p>
      )}

      <FieldGroup title="Names">
        <Field
          label="Site name"
          htmlFor="brand-site-name"
          help="Shows in the sidebar header and as the browser tab title."
        >
          <Input
            id="brand-site-name"
            value={brand.siteName}
            maxLength={80}
            disabled={!canSetDefault}
            onChange={(e) => updateString("siteName", e.target.value)}
            placeholder={BRANDING_PLACEHOLDERS.siteName}
          />
        </Field>
        <Field
          label="Mark abbreviation"
          htmlFor="brand-site-abbr"
          help="The 1–4 letter sigil shown in the sidebar tile when no icon or logo image is set."
        >
          <Input
            id="brand-site-abbr"
            value={brand.siteAbbr}
            maxLength={4}
            disabled={!canSetDefault}
            onChange={(e) => updateString("siteAbbr", e.target.value)}
            placeholder={BRANDING_PLACEHOLDERS.siteAbbr}
          />
        </Field>
        <Field
          label="Tagline"
          htmlFor="brand-tagline"
          help="The subtitle under the name. Leave empty to hide the line."
        >
          <Input
            id="brand-tagline"
            value={brand.tagline}
            maxLength={120}
            disabled={!canSetDefault}
            onChange={(e) => updateString("tagline", e.target.value)}
            placeholder={BRANDING_PLACEHOLDERS.tagline}
          />
        </Field>
      </FieldGroup>

      <FieldGroup title="Marks">
        <p className="mb-2 text-[11px] leading-snug text-muted">
          Upload a logo (full mark, e.g. the "Acme" wordmark), an icon (small sigil, e.g. the Google
          "G"), and the browser-tab favicon. Up to {Math.round(MAX_BRAND_IMAGE_BYTES / KIB)} KiB each;
          PNG, JPEG, WebP, SVG, GIF, or ICO.
        </p>

        <BrandImageField
          label="Logo"
          help="The full mark. Replaces the tile + name in the sidebar header."
          dataUri={brand.logoDataUri}
          disabled={!canSetDefault}
          onUpload={(f) => uploadImage("logoDataUri", f)}
          onClear={() => clearImage("logoDataUri")}
        />
        <BrandImageField
          label="Icon"
          help="The small sigil — replaces the abbreviation in the tile when no logo is set."
          dataUri={brand.iconDataUri}
          disabled={!canSetDefault}
          onUpload={(f) => uploadImage("iconDataUri", f)}
          onClear={() => clearImage("iconDataUri")}
        />
        <BrandImageField
          label="Favicon"
          help="The browser-tab icon. Recommended ICO or PNG; 32×32 or larger."
          dataUri={brand.faviconDataUri}
          disabled={!canSetDefault}
          onUpload={(f) => uploadImage("faviconDataUri", f)}
          onClear={() => clearImage("faviconDataUri")}
        />
      </FieldGroup>

      {imageError && (
        <p role="alert" className="mb-3 text-xs text-red-500">
          {imageError}
        </p>
      )}

      {canSetDefault && (
        <div className="sticky bottom-0 flex items-center gap-3 border-t border-border bg-bg/95 py-3 backdrop-blur">
          <Button onClick={save} disabled={status === "saving"} aria-label="save workspace brand">
            {status === "saving" ? "Saving…" : "Save workspace brand"}
          </Button>
          <Button variant="ghost" size="sm" onClick={resetAll} disabled={status === "saving"}>
            Reset to default
          </Button>
          {status === "saved" && <span className="text-xs text-accent">Saved.</span>}
          {status === "error" && (
            <span role="alert" className="text-xs text-red-500">
              {error}
            </span>
          )}
          <span className="ml-auto text-[11px] text-muted">
            Every member of this workspace sees this brand.
          </span>
        </div>
      )}
    </div>
  );
}

/** A single brand image slot — preview the current image (or a placeholder), upload a new one, or
 *  clear it. The shadcn `<Input type="file">` is hidden and triggered via a ref through the Upload
 *  button (the project's ui-standards discipline: never a raw `<input>`; mirrors
 *  `UploadArtifact.tsx`). The input is reset after each pick so the same file can be re-picked. */
function BrandImageField({
  label,
  help,
  dataUri,
  disabled,
  onUpload,
  onClear,
}: {
  label: string;
  help: string;
  dataUri?: string;
  disabled: boolean;
  onUpload: (file: File | null) => void;
  onClear: () => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  return (
    <Field label={label} help={help}>
      <div className="flex items-center gap-3">
        <div
          className="flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden rounded-md border border-border bg-panel/60"
          aria-hidden="true"
        >
          {dataUri ? (
            <img src={dataUri} alt="" className="h-full w-full object-contain" />
          ) : (
            <span className="text-[10px] uppercase tracking-wide text-muted">none</span>
          )}
        </div>
        <Input
          ref={inputRef}
          type="file"
          accept={BRAND_IMAGE_ACCEPT}
          disabled={disabled}
          className="hidden"
          aria-label={`${label} upload`}
          onChange={(e) => {
            const f = e.target.files?.[0] ?? null;
            onUpload(f);
            e.target.value = "";
          }}
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={disabled}
          aria-label={`upload ${label.toLowerCase()}`}
          onClick={() => inputRef.current?.click()}
        >
          <Upload size={14} />
          <span>Upload</span>
        </Button>
        {dataUri && !disabled && (
          <Button variant="ghost" size="sm" onClick={onClear} aria-label={`clear ${label.toLowerCase()}`}>
            Clear
          </Button>
        )}
      </div>
    </Field>
  );
}
