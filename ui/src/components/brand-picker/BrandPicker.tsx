// BrandPicker — a `<BrandPicker value onChange />` dropdown of the workspace's brand profiles
// (reports scope). Loads `listBrands()` (the host seeds one default, so it's never empty) and lets the
// caller pick the `brandId` a report renders with; a {@link BrandSwatch} beside the select previews the
// chosen palette. Reusable: the report editor is the first consumer, a future brand-settings surface
// the next. One responsibility: select a brand id from the workspace's profiles.

import { useEffect, useState } from "react";

import { Select } from "@/components/ui/select";
import { listBrands, type Brand } from "@/lib/brand";
import { BrandSwatch } from "./swatch";

interface Props {
  /** The selected brand id. */
  value: string;
  onChange: (brandId: string) => void;
  label?: string;
}

export function BrandPicker({ value, onChange, label = "brand profile" }: Props) {
  const [brands, setBrands] = useState<Brand[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | undefined>();

  useEffect(() => {
    let live = true;
    listBrands()
      .then((b) => {
        if (!live) return;
        setBrands(b);
        setLoaded(true);
        // Never leave the picker unset when the host has seeded a default and no value is chosen.
        if (!value && b.length > 0) onChange(b[0].id);
      })
      .catch((e) => live && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const selected = brands.find((b) => b.id === value);

  return (
    <div className="flex items-center gap-2">
      <Select
        aria-label={label}
        className="h-8 w-48"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={!loaded && !error}
      >
        {/* Empty workspace: the export path falls back to a neutral default brand — offer it plainly. */}
        {brands.length === 0 && <option value="">{loaded || error ? "Default brand" : "Loading…"}</option>}
        {brands.map((b) => (
          <option key={b.id} value={b.id}>
            {b.name}
          </option>
        ))}
      </Select>
      {selected && <BrandSwatch colors={selected.colors} />}
    </div>
  );
}
